use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use async_imap::extensions::idle::IdleResponse;
use async_imap::types::{Fetch, Flag, Mailbox, NameAttribute};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

use crate::folder_name;
use crate::message::Flags;
use crate::net::{self, Refused, timed};

pub type Session = async_imap::Session<TlsStream<TcpStream>>;

pub const INBOX: &str = "INBOX";

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

    let mut client = async_imap::Client::new(tls);
    timed("greeting", client.read_response())
        .await?
        .context("reading the server greeting")?
        .context("server closed the connection")?;
    timed("LOGIN", client.login(username, password))
        .await?
        .map_err(|(err, _)| match err {
            async_imap::error::Error::No(info) | async_imap::error::Error::Bad(info) => {
                let failed = format!("login as {username} failed: {}", server_words(&info));
                if info.contains("[UNAVAILABLE]") {
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
        bail!("IDLE: the server closed the connection");
    }
    let session = timed("DONE", handle.done()).await?.context("DONE")?;
    Ok((session, matches!(response, IdleResponse::NewData(_))))
}

fn server_words(info: &str) -> &str {
    info.split_once("info: Some(\"")
        .and_then(|(_, rest)| rest.strip_suffix("\")"))
        .unwrap_or(info)
}

pub async fn list_folders(session: &mut Session) -> Result<Vec<Folder>> {
    let mut folders = Vec::new();
    let mut names = timed("LIST", session.list(Some(""), Some("*")))
        .await?
        .context("LIST")?;
    while let Some(name) = timed("LIST", names.next()).await? {
        let name = name.context("LIST")?;
        folders.push(Folder {
            name: name.name().to_string(),
            delimiter: name.delimiter().map(str::to_string),
            selectable: !name
                .attributes()
                .iter()
                .any(|a| matches!(a, NameAttribute::NoSelect)),
            special: name.attributes().iter().find_map(special_of),
        });
    }
    Ok(folders)
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
            Refused(format!("{command}: {}", server_words(&info))).into()
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

pub async fn store(session: &mut Session, uids: &[u32], query: &str) -> Result<()> {
    let mut fetches = timed("UID STORE", session.uid_store(uid_set(uids), query))
        .await?
        .map_err(|err| refused_or(err, "UID STORE"))?;
    while let Some(fetch) = timed("UID STORE", fetches.next()).await? {
        fetch.map_err(|err| refused_or(err, "UID STORE"))?;
    }
    Ok(())
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
    let expunged: Vec<_> = if capabilities.has_str("UIDPLUS") {
        let stream = timed("UID EXPUNGE", session.uid_expunge(uid_set(uids)))
            .await?
            .map_err(|err| refused_or(err, "UID EXPUNGE"))?;
        timed("UID EXPUNGE", stream.collect()).await?
    } else {
        let stream = timed("EXPUNGE", session.expunge())
            .await?
            .map_err(|err| refused_or(err, "EXPUNGE"))?;
        timed("EXPUNGE", stream.collect()).await?
    };
    for removed in expunged {
        removed.map_err(|err| refused_or(err, "EXPUNGE"))?;
    }
    Ok(())
}

pub async fn fetch_flags(session: &mut Session, uids: &str) -> Result<BTreeMap<u32, Flags>> {
    let mut map = BTreeMap::new();
    let mut fetches = timed("UID FETCH FLAGS", session.uid_fetch(uids, "(FLAGS)"))
        .await?
        .context("UID FETCH FLAGS")?;
    while let Some(fetch) = timed("UID FETCH FLAGS", fetches.next()).await? {
        let fetch = fetch.context("UID FETCH FLAGS")?;
        if let Some(uid) = fetch.uid {
            map.insert(uid, flags_of(&fetch));
        }
    }
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
    let mut fetches = timed(
        "UID FETCH",
        session.uid_fetch(set, "(UID FLAGS INTERNALDATE RFC822.SIZE BODY.PEEK[])"),
    )
    .await?
    .context("UID FETCH")?;
    while let Some(fetch) = timed("UID FETCH", fetches.next()).await? {
        let fetch = fetch.context("UID FETCH")?;
        let (Some(uid), Some(body)) = (fetch.uid, fetch.body()) else {
            continue;
        };
        messages.push(RawMessage {
            uid,
            flags: flags_of(&fetch),
            received: fetch.internal_date().map_or(0, |d| d.timestamp()),
            size: fetch.size.map_or(body.len() as u64, u64::from),
            body: body.to_vec(),
        });
    }
    Ok(messages)
}

fn flags_of(fetch: &Fetch) -> Flags {
    let mut flags = Flags::default();
    for flag in fetch.flags() {
        match flag {
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
