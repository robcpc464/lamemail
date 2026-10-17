use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use async_imap::extensions::idle::IdleResponse;
use async_imap::imap_proto::{AttributeValue, MailboxDatum, Response, Status};
use async_imap::types::{Flag, Mailbox, NameAttribute};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::folder_name;
use crate::message::Flags;
use crate::net::{self, CLOSED, KeptOpen, Refused, timed};

pub trait Wire: AsyncRead + AsyncWrite + Unpin + fmt::Debug + Send + Sync {}

impl<S: AsyncRead + AsyncWrite + Unpin + fmt::Debug + Send + Sync> Wire for S {}

pub type Session = async_imap::Session<Box<dyn Wire>>;

pub const INBOX: &str = "INBOX";
const INTERNAL_DATE: &str = "%d-%b-%Y %H:%M:%S %z";
const UNAVAILABLE: &str = "[UNAVAILABLE]";

#[derive(Debug)]
pub struct CredentialError(pub String);

impl fmt::Display for CredentialError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for CredentialError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Special {
    Sent,
    Drafts,
    Junk,
    Trash,
    Archive,
    All,
}

#[derive(Debug, Clone)]
pub struct Folder {
    pub name: String,
    pub delimiter: Option<String>,
    pub selectable: bool,
    pub special: Option<Special>,
}

impl Folder {
    pub fn is_inbox(&self) -> bool {
        self.name.eq_ignore_ascii_case(INBOX)
    }

    pub fn local_path(&self) -> Result<PathBuf> {
        folder_name::local_path(&self.name, self.delimiter.as_deref())
    }

    pub fn shown(&self) -> Result<String> {
        folder_name::shown(&self.name, self.delimiter.as_deref())
    }

    pub fn is_at(&self, local: &str) -> bool {
        self.local_path()
            .is_ok_and(|path| path.to_string_lossy() == local)
    }
}

pub struct RawMessage {
    pub uid: u32,
    pub flags: Flags,
    pub received: i64,
    pub size: u64,
    pub body: Vec<u8>,
}

pub async fn connect(host: &str, port: u16, username: &str, password: &str) -> Result<Session> {
    let tls = net::tls(host, net::tcp(host, port).await?).await?;
    login(Box::new(KeptOpen(tls)), username, password).await
}

async fn login(wire: Box<dyn Wire>, username: &str, password: &str) -> Result<Session> {
    let mut client = async_imap::Client::new(wire);
    timed("greeting", client.read_response())
        .await?
        .context("reading the server greeting")?
        .context(CLOSED)?;
    timed("LOGIN", client.login(username, password))
        .await?
        .map_err(|(err, _)| match err {
            async_imap::error::Error::No(info) | async_imap::error::Error::Bad(info) => {
                let failed = format!("login as {username} failed: {}", server_words(&info));
                if info.contains(UNAVAILABLE) {
                    anyhow!(failed)
                } else {
                    CredentialError(failed).into()
                }
            }
            err => anyhow!("login as {username} failed: {err}"),
        })
}

pub async fn idle(mut session: Session, name: &str, limit: Duration) -> Result<(Session, bool)> {
    examine(&mut session, name).await?;
    let mut handle = session.idle();
    timed("IDLE", handle.init()).await?.context("IDLE")?;
    let (waiting, _stop) = handle.wait_with_timeout(limit);
    let response = waiting.await.context("IDLE")?;
    if response == IdleResponse::ManualInterrupt {
        bail!("IDLE: {CLOSED}");
    }
    let session = timed("DONE", handle.done()).await?.context("DONE")?;
    Ok((session, matches!(response, IdleResponse::NewData(_))))
}

fn server_words(info: &str) -> &str {
    info.split_once("info: Some(\"")
        .and_then(|(_, rest)| rest.strip_suffix("\")"))
        .unwrap_or(info)
}

async fn answered(
    session: &mut Session,
    label: &'static str,
    command: &str,
    mut each: impl FnMut(&Response<'_>),
) -> Result<Option<String>> {
    let tag = timed(label, session.run_command(command))
        .await?
        .context(label)?;
    loop {
        let response = timed(label, session.read_response())
            .await?
            .context(label)?
            .with_context(|| format!("{label}: {CLOSED}"))?;
        match response.parsed() {
            Response::Done {
                tag: done,
                status,
                information,
                ..
            } if *done == tag => {
                return Ok(match status {
                    Status::Ok => None,
                    _ => Some(words(information)),
                });
            }
            Response::Data {
                status: Status::Bye,
                information,
                ..
            } => bail!("{label}: {CLOSED}: {}", words(information)),
            other => each(other),
        }
    }
}

fn words(information: &Option<Cow<'_, str>>) -> String {
    information
        .as_deref()
        .unwrap_or("no reason given")
        .to_string()
}

async fn query(
    session: &mut Session,
    label: &'static str,
    command: &str,
    each: impl FnMut(&Response<'_>),
) -> Result<()> {
    match answered(session, label, command, each).await? {
        None => Ok(()),
        Some(refusal) => bail!("{label}: {refusal}"),
    }
}

async fn change(session: &mut Session, label: &'static str, command: &str) -> Result<()> {
    match answered(session, label, command, |_| {}).await? {
        None => Ok(()),
        Some(words) => Err(refusal(label, &words)),
    }
}

fn refusal(command: &str, words: &str) -> anyhow::Error {
    let failed = format!("{command}: {words}");
    if words.contains(UNAVAILABLE) {
        anyhow!(failed)
    } else {
        Refused(failed).into()
    }
}

pub async fn list_folders(session: &mut Session) -> Result<Vec<Folder>> {
    let mut folders = Vec::new();
    query(session, "LIST", "LIST \"\" *", |response| {
        folders.extend(folder_of(response));
    })
    .await?;
    Ok(folders)
}

fn folder_of(response: &Response<'_>) -> Option<Folder> {
    let Response::MailboxData(MailboxDatum::List {
        name_attributes,
        delimiter,
        name,
    }) = response
    else {
        return None;
    };
    Some(Folder {
        name: name.to_string(),
        delimiter: delimiter.as_deref().map(str::to_string),
        selectable: !name_attributes
            .iter()
            .any(|a| matches!(a, NameAttribute::NoSelect)),
        special: name_attributes.iter().find_map(special_of),
    })
}

fn special_of(attribute: &NameAttribute) -> Option<Special> {
    match attribute {
        NameAttribute::Sent => Some(Special::Sent),
        NameAttribute::Drafts => Some(Special::Drafts),
        NameAttribute::Junk => Some(Special::Junk),
        NameAttribute::Trash => Some(Special::Trash),
        NameAttribute::Archive => Some(Special::Archive),
        NameAttribute::All => Some(Special::All),
        _ => None,
    }
}

pub async fn examine(session: &mut Session, name: &str) -> Result<Mailbox> {
    timed("EXAMINE", session.examine(name))
        .await?
        .context("EXAMINE")
}

pub fn allows_keywords(mailbox: &Mailbox) -> bool {
    mailbox.permanent_flags.contains(&Flag::MayCreate)
}

pub async fn select(session: &mut Session, name: &str) -> Result<Mailbox> {
    timed("SELECT", session.select(name))
        .await?
        .map_err(|err| refused_or(err, &format!("SELECT {name}")))
}

pub async fn status(session: &mut Session, name: &str) -> Result<Mailbox> {
    timed("STATUS", session.status(name, "(UIDNEXT UIDVALIDITY)"))
        .await?
        .map_err(|err| refused_or(err, &format!("STATUS {name}")))
}

pub async fn create(session: &mut Session, name: &str) -> Result<()> {
    timed("CREATE", session.create(name))
        .await?
        .map_err(|err| refused_or(err, &format!("CREATE {name}")))
}

pub async fn subscribe(session: &mut Session, name: &str) -> Result<()> {
    timed("SUBSCRIBE", session.subscribe(name))
        .await?
        .map_err(|err| refused_or(err, &format!("SUBSCRIBE {name}")))
}

pub async fn move_uids(session: &mut Session, uids: &[u32], name: &str) -> Result<()> {
    timed("UID MOVE", session.uid_mv(uid_set(uids), name))
        .await?
        .map_err(|err| refused_or(err, &format!("UID MOVE {name}")))
}

fn refused_or(err: async_imap::error::Error, command: &str) -> anyhow::Error {
    match err {
        async_imap::error::Error::No(info) | async_imap::error::Error::Bad(info) => {
            refusal(command, server_words(&info))
        }
        err => anyhow::Error::new(err).context(command.to_string()),
    }
}

pub fn uid_set(uids: &[u32]) -> String {
    uids.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

pub async fn store(session: &mut Session, uids: &[u32], flags: &str) -> Result<()> {
    let command = format!("UID STORE {} {flags}", uid_set(uids));
    change(session, "UID STORE", &command).await
}

pub async fn append(session: &mut Session, name: &str, flags: &str, message: &[u8]) -> Result<()> {
    timed("APPEND", session.append(name, Some(flags), None, message))
        .await?
        .map_err(|err| refused_or(err, &format!("APPEND {name}")))
}

pub async fn expunge(session: &mut Session, uids: &[u32]) -> Result<()> {
    store(session, uids, "+FLAGS.SILENT (\\Deleted)").await?;
    let capabilities = timed("CAPABILITY", session.capabilities())
        .await?
        .context("CAPABILITY")?;
    if capabilities.has_str("UIDPLUS") {
        let command = format!("UID EXPUNGE {}", uid_set(uids));
        change(session, "UID EXPUNGE", &command).await
    } else {
        change(session, "EXPUNGE", "EXPUNGE").await
    }
}

pub async fn fetch_flags(session: &mut Session, uids: &str) -> Result<BTreeMap<u32, Flags>> {
    let mut map = BTreeMap::new();
    let command = format!("UID FETCH {uids} (FLAGS)");
    query(session, "UID FETCH FLAGS", &command, |response| {
        if let Response::Fetch(_, attributes) = response
            && let Some(uid) = uid_of(attributes)
        {
            map.insert(uid, flags_of(attributes));
        }
    })
    .await?;
    Ok(map)
}

pub async fn fetch_messages(session: &mut Session, uids: &[u32]) -> Result<Vec<RawMessage>> {
    if uids.is_empty() {
        return Ok(Vec::new());
    }
    fetch_bodies(session, uid_set(uids)).await
}

pub async fn fetch_from(session: &mut Session, first: u32) -> Result<Vec<RawMessage>> {
    let mut messages = fetch_bodies(session, format!("{first}:*")).await?;
    messages.retain(|message| message.uid >= first);
    Ok(messages)
}

async fn fetch_bodies(session: &mut Session, set: String) -> Result<Vec<RawMessage>> {
    let mut messages = Vec::new();
    let command = format!("UID FETCH {set} (UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[])");
    query(session, "UID FETCH", &command, |response| {
        if let Response::Fetch(_, attributes) = response {
            messages.extend(message_of(attributes));
        }
    })
    .await?;
    Ok(messages)
}

fn message_of(attributes: &[AttributeValue<'_>]) -> Option<RawMessage> {
    let (mut received, mut size, mut body) = (0, None, None);
    for attribute in attributes {
        match attribute {
            AttributeValue::InternalDate(date) => received = seconds_of(date).unwrap_or(0),
            AttributeValue::Rfc822Size(octets) => size = Some(u64::from(*octets)),
            AttributeValue::BodySection {
                section: None,
                data: Some(data),
                ..
            } => body = Some(data),
            _ => {}
        }
    }
    let body = body?;
    Some(RawMessage {
        uid: uid_of(attributes)?,
        flags: flags_of(attributes),
        received,
        size: size.unwrap_or(body.len() as u64),
        body: body.to_vec(),
    })
}

fn seconds_of(internal_date: &str) -> Option<i64> {
    let time = jiff::fmt::strtime::parse(INTERNAL_DATE, internal_date).ok()?;
    Some(time.to_timestamp().ok()?.as_second())
}

fn uid_of(attributes: &[AttributeValue<'_>]) -> Option<u32> {
    attributes.iter().find_map(|attribute| match attribute {
        AttributeValue::Uid(uid) => Some(*uid),
        _ => None,
    })
}

fn flags_of(attributes: &[AttributeValue<'_>]) -> Flags {
    let mut flags = Flags::default();
    let names = attributes.iter().filter_map(|attribute| match attribute {
        AttributeValue::Flags(names) => Some(names),
        _ => None,
    });
    for name in names.flatten() {
        match Flag::from(name.as_ref()) {
            Flag::Seen => flags.seen = true,
            Flag::Answered => flags.answered = true,
            Flag::Flagged => flags.flagged = true,
            Flag::Draft => flags.draft = true,
            Flag::Custom(keyword) => flags.keywords.push(keyword.to_string()),
            _ => {}
        }
    }
    flags.keywords.sort();
    flags
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};

    use super::*;

    type Script = Vec<(&'static str, &'static str)>;

    const DONE: &str = "TAG OK done\r\n";
    const LIST: &str = "LIST \"\" *";
    const FLAGS: &str = "UID FETCH 1:* (FLAGS)";
    const SEEN: &str = "+FLAGS.SILENT (\\Seen)";

    async fn logged_in(script: &[(&'static str, &'static str)]) -> Session {
        let (client, server) = tokio::io::duplex(4096);
        tokio::spawn(serve(server, script.to_vec()));
        login(Box::new(KeptOpen(client)), "me", "secret")
            .await
            .unwrap()
    }

    async fn serve(stream: DuplexStream, script: Script) {
        let (reading, mut writing) = tokio::io::split(stream);
        let mut lines = BufReader::new(reading).lines();
        let mut reply = "* OK ready\r\n".to_string();
        loop {
            if writing.write_all(reply.as_bytes()).await.is_err() || reply.is_empty() {
                return;
            }
            let Ok(Some(line)) = lines.next_line().await else {
                return;
            };
            let (tag, command) = line.split_once(' ').unwrap();
            let scripted = script.iter().find(|(asked, _)| *asked == command);
            reply = match scripted {
                Some((_, reply)) if reply.contains("TAG") => reply.replace("TAG", tag),
                Some((_, cut_off)) => {
                    let _ = writing.write_all(cut_off.as_bytes()).await;
                    return;
                }
                None if command.starts_with("LOGIN ") => DONE.replace("TAG", tag),
                None => return,
            };
        }
    }

    fn words(err: &anyhow::Error) -> String {
        format!("{err:#}")
    }

    #[tokio::test]
    async fn folders_are_listed_with_their_markings() {
        let listed = "* LIST (\\HasNoChildren) \"/\" INBOX\r\n\
                      A9999 NO left over from another command\r\n\
                      * NO a warning\r\n\
                      * LIST (\\HasNoChildren \\Sent) \"/\" Sent\r\n\
                      * LIST (\\Noselect \\HasChildren) \"/\" \"[Gmail]\"\r\n\
                      TAG OK done\r\n";
        let mut session = logged_in(&[(LIST, listed)]).await;
        let folders = list_folders(&mut session).await.unwrap();
        let folders: Vec<_> = folders
            .iter()
            .map(|folder| {
                (
                    folder.name.as_str(),
                    folder.delimiter.as_deref(),
                    folder.selectable,
                    folder.special,
                )
            })
            .collect();
        assert_eq!(
            folders,
            [
                ("INBOX", Some("/"), true, None),
                ("Sent", Some("/"), true, Some(Special::Sent)),
                ("[Gmail]", Some("/"), false, None),
            ]
        );
    }

    #[tokio::test]
    async fn a_refused_list_is_an_error_and_never_an_empty_list() {
        for refusal in [
            "TAG NO Temporary failure\r\n",
            "TAG BAD Temporary failure\r\n",
        ] {
            let mut session = logged_in(&[(LIST, refusal)]).await;
            let err = list_folders(&mut session).await.unwrap_err();
            assert_eq!(words(&err), "LIST: Temporary failure");
            assert!(!err.is::<Refused>());
        }
    }

    #[tokio::test]
    async fn a_list_the_server_hangs_up_on_is_an_error() {
        for cut_off in ["", "* LIST (\\HasNoChildren) \"/\" INBOX\r\n"] {
            let mut session = logged_in(&[(LIST, cut_off)]).await;
            let err = list_folders(&mut session).await.unwrap_err();
            assert_eq!(words(&err), format!("LIST: {CLOSED}"));
        }
        let mut session = logged_in(&[(LIST, "* BYE Disconnected for inactivity.\r\n")]).await;
        let err = list_folders(&mut session).await.unwrap_err();
        assert_eq!(
            words(&err),
            format!("LIST: {CLOSED}: Disconnected for inactivity.")
        );
    }

    #[tokio::test]
    async fn a_folder_the_server_hangs_up_on_while_it_opens_is_an_error() {
        let cut_off = "* OK [UIDVALIDITY 7] UIDs valid\r\n";
        let mut session = logged_in(&[("EXAMINE \"INBOX\"", cut_off)]).await;
        let err = examine(&mut session, INBOX).await.unwrap_err();
        assert!(words(&err).ends_with(CLOSED), "{err:#}");
    }

    #[tokio::test]
    async fn flags_come_by_uid_and_a_refused_or_cut_off_answer_is_an_error() {
        let fetched = "* 1 FETCH (UID 7 FLAGS (\\Seen $label1))\r\n\
                       * 2 FETCH (FLAGS (\\Flagged \\Draft) UID 9)\r\n\
                       * 3 FETCH (FLAGS (\\Seen))\r\n\
                       TAG OK done\r\n";
        let mut session = logged_in(&[(FLAGS, fetched)]).await;
        let flags = fetch_flags(&mut session, "1:*").await.unwrap();
        assert_eq!(flags.keys().copied().collect::<Vec<_>>(), [7, 9]);
        assert!(flags[&7].seen && !flags[&7].flagged);
        assert_eq!(flags[&7].keywords, ["$label1"]);
        assert!(flags[&9].flagged && flags[&9].draft && !flags[&9].seen);

        let mut session = logged_in(&[(FLAGS, "TAG NO Internal error\r\n")]).await;
        let err = fetch_flags(&mut session, "1:*").await.unwrap_err();
        assert_eq!(words(&err), "UID FETCH FLAGS: Internal error");
        assert!(!err.is::<Refused>());

        let cut_off = "* 1 FETCH (UID 7 FLAGS (\\Seen))\r\n";
        let mut session = logged_in(&[(FLAGS, cut_off)]).await;
        let err = fetch_flags(&mut session, "1:*").await.unwrap_err();
        assert_eq!(words(&err), format!("UID FETCH FLAGS: {CLOSED}"));
    }

    #[tokio::test]
    async fn a_message_comes_with_its_time_size_flags_and_body() {
        let fetch = "UID FETCH 7,8 (UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[])";
        let fetched = "* 1 FETCH (UID 7 FLAGS (\\Answered) \
                       INTERNALDATE \" 5-Jan-2026 09:03:07 +0100\" RFC822.SIZE 9 BODY[] {5}\r\n\
                       hello)\r\n\
                       * 2 FETCH (UID 8 FLAGS () BODY[] {2}\r\n\
                       hi)\r\n\
                       * 3 FETCH (FLAGS (\\Seen))\r\n\
                       TAG OK done\r\n";
        let mut session = logged_in(&[(fetch, fetched)]).await;
        let messages = fetch_messages(&mut session, &[7, 8]).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].uid, 7);
        assert!(messages[0].flags.answered);
        assert_eq!(messages[0].received, 1_767_600_187);
        assert_eq!(messages[0].size, 9);
        assert_eq!(messages[0].body, b"hello");
        assert_eq!((messages[1].uid, messages[1].received), (8, 0));
        assert_eq!(messages[1].size, 2);
        assert_eq!(messages[1].body, b"hi");
        assert_eq!(seconds_of("17-Jul-1996 02:44:25 -0700"), Some(837_596_665));
        assert_eq!(seconds_of("yesterday"), None);

        let mut session = logged_in(&[(fetch, "TAG NO Internal error\r\n")]).await;
        assert!(fetch_messages(&mut session, &[7, 8]).await.is_err());
    }

    #[tokio::test]
    async fn a_refused_change_is_refused_and_a_cut_off_one_fails() {
        let command = "UID STORE 4,5 +FLAGS.SILENT (\\Seen)";
        let mut session = logged_in(&[(command, DONE)]).await;
        store(&mut session, &[4, 5], SEEN).await.unwrap();

        let mut session = logged_in(&[(command, "TAG NO Mailbox is read-only\r\n")]).await;
        let err = store(&mut session, &[4, 5], SEEN).await.unwrap_err();
        assert!(err.is::<Refused>());
        assert_eq!(words(&err), "UID STORE: Mailbox is read-only");

        let mut session = logged_in(&[(command, "")]).await;
        let err = store(&mut session, &[4, 5], SEEN).await.unwrap_err();
        assert!(!err.is::<Refused>());
    }

    #[tokio::test]
    async fn a_refusal_while_the_server_is_unavailable_is_no_last_word() {
        let down = "TAG NO [UNAVAILABLE] Temporary failure\r\n";
        let store_seen = "UID STORE 4 +FLAGS.SILENT (\\Seen)";
        let open = "SELECT \"INBOX\"";
        let mut session = logged_in(&[(store_seen, down), (open, down)]).await;
        let err = store(&mut session, &[4], SEEN).await.unwrap_err();
        assert!(!err.is::<Refused>());
        assert_eq!(words(&err), "UID STORE: [UNAVAILABLE] Temporary failure");
        let err = select(&mut session, INBOX).await.unwrap_err();
        assert!(!err.is::<Refused>());
        assert_eq!(words(&err), "SELECT INBOX: [UNAVAILABLE] Temporary failure");

        let gone = "TAG NO Mailbox doesn't exist\r\n";
        let mut session = logged_in(&[(open, gone)]).await;
        let err = select(&mut session, INBOX).await.unwrap_err();
        assert!(err.is::<Refused>());
    }

    #[tokio::test]
    async fn messages_are_expunged_by_uid_where_the_server_can() {
        let deleted = ("UID STORE 4 +FLAGS.SILENT (\\Deleted)", DONE);
        let with_uidplus = (
            "CAPABILITY",
            "* CAPABILITY IMAP4rev1 UIDPLUS\r\nTAG OK done\r\n",
        );
        let plain = ("CAPABILITY", "* CAPABILITY IMAP4rev1\r\nTAG OK done\r\n");
        let expunged = "* 1 EXPUNGE\r\nTAG OK done\r\n";

        let mut session = logged_in(&[deleted, with_uidplus, ("UID EXPUNGE 4", expunged)]).await;
        expunge(&mut session, &[4]).await.unwrap();

        let mut session = logged_in(&[deleted, plain, ("EXPUNGE", expunged)]).await;
        expunge(&mut session, &[4]).await.unwrap();

        let refusal = ("UID EXPUNGE 4", "TAG NO Permission denied\r\n");
        let mut session = logged_in(&[deleted, with_uidplus, refusal]).await;
        let err = expunge(&mut session, &[4]).await.unwrap_err();
        assert!(err.is::<Refused>());
        assert_eq!(words(&err), "UID EXPUNGE: Permission denied");
    }
}
