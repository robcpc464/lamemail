use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use jiff::Timestamp;
use serde::Serialize;

use crate::compose::{self, Choice, File, LocalFile, Original, Outgoing};
use crate::draft::Continued;
use crate::flag::Change;
use crate::i18n::{fill, t};
use crate::imap::Folder;
use crate::journal::{self, Action, Dropped, Outbound, Purpose, Pushed};
use crate::kind::{Kind, Kinds};
use crate::message::{self, Addr, Attachment, MailFile};
use crate::net::Refused;
use crate::server::Server;
use crate::settings::{self, Account, Settings};
use crate::show::{self, Identities, Message};
use crate::{attachment, draft, list, output, store, sync};

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SentCopy {
    Saved,
    Off,
    Failed,
}

#[derive(Serialize)]
pub struct Sent {
    pub account: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub message_id: String,
    pub from: Addr,
    pub to: Vec<Addr>,
    pub cc: Vec<Addr>,
    pub bcc: Vec<Addr>,
    pub subject: String,
    pub copy: SentCopy,
    pub folder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub struct Reply {
    message: Message,
    me: Identities,
}

pub struct NewMail {
    id: String,
    me: Identities,
}

pub type Files = BTreeMap<u64, Result<Vec<File>, String>>;

pub struct Answer {
    pub all: bool,
    pub forward: bool,
}

pub struct Request<'a> {
    pub continues: Option<&'a str>,
    pub attachments: &'a [Pick],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pick {
    Draft(u32),
    Message(PathBuf, u32),
}

impl FromStr for Pick {
    type Err = String;

    fn from_str(text: &str) -> Result<Pick, String> {
        if let Ok(index) = text.parse() {
            return Ok(Pick::Draft(index));
        }
        text.rsplit_once(':')
            .filter(|(path, _)| !path.is_empty())
            .and_then(|(path, index)| Some(Pick::Message(path.into(), index.parse().ok()?)))
            .ok_or_else(|| format!("expected <index> or <path>:<index>, not {text}"))
    }
}

pub fn run_reply(
    dir: &Path,
    path: &Path,
    choice: &Choice,
    answer: &Answer,
    request: &Request,
    json: bool,
) -> Result<()> {
    if let Some(id) = draft_alone(dir, path, request)? {
        return run_new(dir, &id, choice, request, json);
    }
    let reply = Reply::prepare(dir, path, choice.draft)?;
    let noted = reply.note(dir, choice, answer, request, &read_text()?)?;
    report(&noted, json)
}

fn draft_alone(dir: &Path, path: &Path, request: &Request) -> Result<Option<String>> {
    if request.continues.is_none() || dir.join(path).is_file() {
        return Ok(None);
    }
    Ok(Some(
        list::locate_one(dir, &settings::load(dir)?, path)?.account,
    ))
}

pub fn run_new(dir: &Path, id: &str, choice: &Choice, request: &Request, json: bool) -> Result<()> {
    let mail = NewMail::prepare(dir, id, choice.draft)?;
    let noted = mail.note(dir, choice, request, &read_text()?)?;
    report(&noted, json)
}

pub fn read_text() -> Result<String> {
    let mut text = String::new();
    io::stdin()
        .read_to_string(&mut text)
        .context("reading the text from stdin")?;
    Ok(text)
}

fn report(noted: &Outbound, json: bool) -> Result<()> {
    output::stdout(|out| {
        if json {
            output::json_line(out, noted)
        } else {
            let people = everyone(&noted.mail.to, &noted.mail.cc, &noted.mail.bcc);
            let template = match (noted.purpose, people.is_empty()) {
                (Purpose::Send, _) => t().send_noted,
                (Purpose::Draft, false) => t().draft_noted,
                (Purpose::Draft, true) => t().draft_noted_alone,
            };
            writeln!(out, "{}", fill(template, &[&show::people(&people)]))
        }
    })
}

fn everyone(to: &[Addr], cc: &[Addr], bcc: &[Addr]) -> Vec<Addr> {
    to.iter().chain(cc).chain(bcc).cloned().collect()
}

impl Reply {
    pub fn prepare(dir: &Path, path: &Path, draft: bool) -> Result<Reply> {
        let settings = settings::load(dir)?;
        let (message, me) = show::open(dir, &settings, path)?;
        ensure_ready(dir, &settings, &message.account, draft)?;
        Ok(Reply { message, me })
    }

    pub fn note(
        self,
        dir: &Path,
        choice: &Choice,
        answer: &Answer,
        request: &Request,
        text: &str,
    ) -> Result<Outbound> {
        let message = &self.message;
        let continued = continued(dir, &message.account, request)?;
        let originals = originals(
            dir,
            &message.account,
            request.attachments,
            continued.as_ref(),
        )?;
        let files = local_files(&choice.files)?;
        let mut mail = compose::reply(
            message,
            &self.me,
            choice,
            answer.all,
            answer.forward,
            text,
            Timestamp::now(),
        )?;
        mail.originals = originals;
        mail.files = files;
        let path = Some(message.path.clone());
        record(
            dir,
            choice,
            &message.account,
            path,
            answer.forward,
            request,
            mail,
        )
    }
}

fn continued(dir: &Path, account: &str, request: &Request) -> Result<Option<Continued>> {
    request
        .continues
        .map(|message_id| draft::continued(dir, account, message_id))
        .transpose()
}

fn originals(
    dir: &Path,
    account: &str,
    picks: &[Pick],
    continued: Option<&Continued>,
) -> Result<Vec<Original>> {
    let mut drafted = Vec::new();
    let mut messages: Vec<(&Path, Vec<u32>)> = Vec::new();
    for pick in picks {
        match pick {
            Pick::Draft(index) => drafted.push(*index),
            Pick::Message(path, index) => match messages.iter_mut().find(|(at, _)| at == path) {
                Some((_, indices)) => indices.push(*index),
                None => messages.push((path, vec![*index])),
            },
        }
    }
    let mut originals = match continued {
        _ if drafted.is_empty() => Vec::new(),
        Some(continued) => continued.originals(dir, &drafted)?,
        None => bail!("{}", t().attachment_no_draft),
    };
    if messages.is_empty() {
        return Ok(originals);
    }
    let settings = settings::load(dir)?;
    for (path, indices) in messages {
        let located = list::locate_one(dir, &settings, path)?;
        if located.account != account {
            bail!(
                "{}",
                fill(t().attachment_other_account, &[&located.path, &account])
            );
        }
        let file: MailFile = store::read(&located.file)?;
        originals.push(original(
            dir,
            &located.path,
            file.uid,
            &file.attachments,
            &indices,
        )?);
    }
    Ok(originals)
}

pub fn original(
    dir: &Path,
    path: &str,
    uid: u32,
    attachments: &[Attachment],
    indices: &[u32],
) -> Result<Original> {
    let wanted: BTreeSet<u32> = indices.iter().copied().collect();
    let known = |index: &u32| attachments.iter().any(|a| a.index == *index);
    if let Some(index) = wanted.iter().find(|index| !known(index)) {
        bail!("{}", fill(t().attachment_none, &[&path, index]));
    }
    let state = store::state_of(&dir.join(path))?;
    Ok(Original {
        folder: state.imap_name,
        uidvalidity: state.uidvalidity,
        uid,
        attachments: attachments
            .iter()
            .filter(|attachment| wanted.contains(&attachment.index))
            .cloned()
            .collect(),
    })
}

fn record(
    dir: &Path,
    choice: &Choice,
    account: &str,
    path: Option<String>,
    forward: bool,
    request: &Request,
    mut mail: Outgoing,
) -> Result<Outbound> {
    if let Some(message_id) = request.continues {
        draft::check_message_id(message_id)?;
        mail.message_id = message_id.to_string();
    }
    journal::record_outbound(
        dir,
        Outbound {
            purpose: if choice.draft {
                Purpose::Draft
            } else {
                Purpose::Send
            },
            id: 0,
            account: account.to_string(),
            path,
            forward,
            sent: false,
            mail,
        },
    )
}

impl NewMail {
    pub fn prepare(dir: &Path, id: &str, draft: bool) -> Result<NewMail> {
        let settings = settings::load(dir)?;
        ensure_ready(dir, &settings, id, draft)?;
        Ok(NewMail {
            id: id.to_string(),
            me: show::identities(dir, &settings, id)?,
        })
    }

    pub fn note(
        self,
        dir: &Path,
        choice: &Choice,
        request: &Request,
        text: &str,
    ) -> Result<Outbound> {
        let continued = continued(dir, &self.id, request)?;
        let originals = originals(dir, &self.id, request.attachments, continued.as_ref())?;
        let files = local_files(&choice.files)?;
        let mut mail = compose::new_mail(&self.me, &self.id, choice, text, Timestamp::now())?;
        mail.originals = originals;
        mail.files = files;
        if let Some(continued) = continued {
            mail.in_reply_to = continued.in_reply_to;
            mail.references = continued.references;
        }
        record(dir, choice, &self.id, None, false, request, mail)
    }
}

fn local_files(paths: &[PathBuf]) -> Result<Vec<LocalFile>> {
    paths
        .iter()
        .map(|path| {
            let path = std::path::absolute(path)
                .with_context(|| fill(t().send_no_file, &[&path.display()]))?;
            let readable = fs::File::open(&path).and_then(|file| file.metadata());
            let metadata = match readable {
                Ok(metadata) if metadata.is_file() => metadata,
                _ => bail!("{}", fill(t().send_no_file, &[&path.display()])),
            };
            Ok(LocalFile {
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                content_type: compose::content_type(&path).to_string(),
                size: metadata.len(),
                path,
            })
        })
        .collect()
}

fn ensure_ready(dir: &Path, settings: &Settings, id: &str, draft: bool) -> Result<()> {
    let account = settings.account(id)?;
    if draft {
        return draft::ensure_folder(dir, settings, id);
    }
    account
        .smtp
        .as_ref()
        .with_context(|| fill(t().no_smtp, &[&id, &id]))?;
    Ok(())
}

pub async fn fetch_files(server: &mut Server, outbox: &[Outbound]) -> Result<Files> {
    let mut files = Files::new();
    let copies = server.copies_to_sent();
    for outbound in outbox.iter().filter(|outbound| !outbound.sent || copies) {
        let fetched = fetch_originals(server, &outbound.mail.originals).await?;
        let local: Result<Vec<File>, String> = outbound.mail.files.iter().map(read_file).collect();
        let attached = fetched.and_then(|mut attached| {
            attached.extend(local?);
            Ok(attached)
        });
        files.insert(outbound.id, attached);
    }
    Ok(files)
}

fn read_file(local: &LocalFile) -> Result<File, String> {
    let content =
        fs::read(&local.path).map_err(|err| format!("{}: {err}", local.path.display()))?;
    Ok(File {
        name: local.name.clone(),
        content_type: local.content_type.clone(),
        content,
    })
}

async fn fetch_originals(
    server: &mut Server,
    originals: &[Original],
) -> Result<Result<Vec<File>, String>> {
    let mut fetched = Vec::new();
    for original in originals {
        match attachments_of(server, original).await? {
            Ok(files) => fetched.extend(files),
            Err(reason) => return Ok(Err(reason)),
        }
    }
    Ok(Ok(fetched))
}

async fn attachments_of(
    server: &mut Server,
    original: &Original,
) -> Result<Result<Vec<File>, String>> {
    let fetched = server
        .fetch_message(&original.folder, original.uidvalidity, original.uid)
        .await;
    let raw = match fetched {
        Ok(raw) => raw,
        Err(err) if err.is::<Refused>() => return Ok(Err(format!("{err:#}"))),
        Err(err) => return Err(err),
    };
    let source = format!("{} {}", original.folder, original.uid);
    Ok(original
        .attachments
        .iter()
        .map(|attachment| {
            let content = message::attachment_content(&raw, attachment.index)
                .ok_or_else(|| fill(t().attachment_none, &[&source, &attachment.index]))?;
            Ok(File {
                name: attachment::file_name(attachment),
                content_type: attachment.content_type.clone(),
                content,
            })
        })
        .collect())
}

pub async fn push(
    server: &mut Server,
    dir: &Path,
    id: &str,
    outbox: Vec<Outbound>,
    mut files: Files,
    pushed: &mut Pushed,
) -> Result<()> {
    let account = settings::load(dir)?.account(id)?.clone();
    let listed = server.list_folders().await?;
    let sent_folder = sent_folder(&listed, &account);
    let next = match &sent_folder {
        Some((folder, _)) => match server.next_uid(&folder.name).await {
            Ok(next) => Some(next),
            Err(err) if err.is::<Refused>() => None,
            Err(err) => return Err(err),
        },
        None => None,
    };
    let mut delivered = false;
    for outbound in outbox {
        let attached = files.remove(&outbound.id).unwrap_or_else(|| Ok(Vec::new()));
        if !outbound.sent {
            let refused = match &attached {
                Err(reason) => Some(fill(t().send_missing, &[reason])),
                Ok(attached) => match server.submit(&outbound.mail, attached).await {
                    Ok(()) => None,
                    Err(err) if err.is::<Refused>() => {
                        Some(fill(t().send_refused, &[&format!("{err:#}")]))
                    }
                    Err(err) => return Err(err),
                },
            };
            if let Some(reason) = refused {
                journal::forget_mail(dir, outbound.id)?;
                pushed.dropped.push(Dropped {
                    path: outbound.path,
                    message_id: Some(outbound.mail.message_id),
                    reason,
                });
                continue;
            }
            journal::mark_sent(dir, outbound.id)?;
            if let Some(path) = &outbound.path {
                mark_original(dir, path, outbound.forward)?;
            }
        }
        draft::remove(server, dir, id, &outbound.mail.message_id).await?;
        let copy = match (server.copies_to_sent(), &attached) {
            (false, _) => Copy::Off,
            (true, Err(reason)) => Copy::Failed(reason.clone()),
            (true, Ok(attached)) => {
                save_copy(server, sent_folder.as_ref(), &outbound.mail, attached).await?
            }
        };
        journal::forget_mail(dir, outbound.id)?;
        delivered = true;
        pushed.pushed += 1;
        pushed.sent.push(Sent::of(outbound, copy));
    }
    if let (true, Some((folder, local)), Some(next)) = (delivered, sent_folder, next) {
        let folder_dir = dir.join(id).join(local);
        let state = store::read_state(&folder_dir)?;
        if state.is_some_and(|state| state.uidvalidity == next.uid_validity) {
            sync::fetch_new(server, &folder_dir, id, &folder.name, next.uid).await?;
        }
    }
    Ok(())
}

fn mark_original(dir: &Path, path: &str, forward: bool) -> Result<()> {
    let file = dir.join(path);
    if !file.exists() {
        return Ok(());
    }
    let change = if forward {
        Change::Forwarded
    } else {
        Change::Answered
    };
    let state = store::state_of(&file)?;
    journal::record(dir, path, state.uidvalidity, Action::Flag { change })
}

enum Copy {
    Saved(String),
    Off,
    Failed(String),
}

async fn save_copy(
    server: &mut Server,
    sent_folder: Option<&(&Folder, String)>,
    mail: &Outgoing,
    files: &[File],
) -> Result<Copy> {
    let Some((folder, local)) = sent_folder else {
        return Ok(Copy::Failed(t().no_sent_folder.to_string()));
    };
    match server.append(&folder.name, &mail.copy(files)?).await {
        Ok(()) => Ok(Copy::Saved(local.clone())),
        Err(err) if err.is::<Refused>() => Ok(Copy::Failed(format!("{err:#}"))),
        Err(err) => Err(err),
    }
}

impl Sent {
    fn of(outbound: Outbound, copy: Copy) -> Sent {
        let (copy, folder, error) = match copy {
            Copy::Saved(local) => (
                SentCopy::Saved,
                Some(format!("{}/{local}", outbound.account)),
                None,
            ),
            Copy::Off => (SentCopy::Off, None, None),
            Copy::Failed(reason) => (
                SentCopy::Failed,
                None,
                Some(fill(t().copy_failed, &[&reason])),
            ),
        };
        let mail = outbound.mail;
        Sent {
            account: outbound.account,
            path: outbound.path,
            message_id: mail.message_id,
            from: mail.from,
            to: mail.to,
            cc: mail.cc,
            bcc: mail.bcc,
            subject: mail.subject,
            copy,
            folder,
            error,
        }
    }

    pub fn lines(&self) -> Vec<String> {
        let people = everyone(&self.to, &self.cc, &self.bcc);
        let mut lines = vec![fill(t().sent_to, &[&show::people(&people)])];
        match (&self.copy, &self.folder, &self.error) {
            (SentCopy::Saved, Some(folder), _) => lines.push(fill(t().copy_saved, &[folder])),
            (SentCopy::Off, _, _) => lines.push(t().copy_off.to_string()),
            (_, _, Some(error)) => lines.push(error.clone()),
            _ => {}
        }
        lines
    }
}

fn sent_folder<'a>(listed: &'a [Folder], account: &Account) -> Option<(&'a Folder, String)> {
    let sent = Kinds::of(account).folder(Kind::Sent)?.to_string();
    let found = listed
        .iter()
        .find(|folder| folder.selectable && folder.is_at(&sent))?;
    Some((found, sent))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use mail_parser::{MessageParser, MimeHeaders};

    use super::*;
    use crate::imap::Special;
    use crate::message::MailFile;
    use crate::moving;
    use crate::settings::Smtp;
    use crate::testing::{Fixture, assigned};

    const REPLY: Answer = Answer {
        all: false,
        forward: false,
    };
    const PLAIN: Request = Request {
        continues: None,
        attachments: &[],
    };

    fn folder(name: &str, sent: bool) -> Folder {
        Folder {
            name: name.to_string(),
            delimiter: Some(".".to_string()),
            selectable: true,
            special: sent.then_some(Special::Sent),
        }
    }

    #[test]
    fn the_sent_folder_is_the_one_assigned_to_sent() {
        let listed = [
            folder("INBOX", false),
            folder("Gesendet", false),
            folder("INBOX.Sent Items", true),
        ];
        let mut account = Account::new("imap.example.com".to_string(), 993, "me".to_string());
        assert!(sent_folder(&listed, &account).is_none());
        account.folders = assigned(&[("inbox/sent items", Kind::Sent)]);
        let (found, local) = sent_folder(&listed, &account).unwrap();
        assert_eq!(
            (found.name.as_str(), local.as_str()),
            ("INBOX.Sent Items", "inbox/sent items")
        );
        let mut unselectable = folder("INBOX.Sent Items", true);
        unselectable.selectable = false;
        assert!(sent_folder(&[unselectable], &account).is_none());
    }

    fn submitted(fixture: &Fixture, message_id: &str) -> PathBuf {
        fixture.dev.join(format!("private/.smtp/{message_id}.eml"))
    }

    #[tokio::test]
    async fn a_reply_is_noted_and_the_push_sends_it_and_fetches_its_copy() {
        let mut fixture = Fixture::new("send");
        let dir = fixture.dir.clone();
        fixture.sync().await;
        let inbox = store::index(&dir.join("private/inbox")).unwrap();
        let path = store::relative(&inbox[&1], &dir);

        let err = Reply::prepare(&dir, Path::new(&path), false).err().unwrap();
        assert_eq!(
            err.to_string(),
            fill(t().no_smtp, &[&"private", &"private"])
        );

        fixture.account.smtp = Some(Smtp::new("smtp.example.com".to_string(), 465, None));
        fixture.save();
        let reply = Reply::prepare(&dir, Path::new(&path), false).unwrap();
        let noted = reply
            .note(&dir, &Choice::default(), &REPLY, &PLAIN, "Thanks!")
            .unwrap();
        assert_eq!(noted.path.as_deref(), Some(path.as_str()));
        assert!(!submitted(&fixture, &noted.mail.message_id).exists());
        let line = serde_json::to_value(&noted).unwrap();
        assert_eq!(line["action"], "send");
        assert_eq!(line["account"], "private");
        assert_eq!(line["to"][0]["address"], "jane@example.com");

        let pushed = fixture.push().await;
        assert_eq!((pushed.pushed, pushed.dropped.len()), (2, 0));
        assert!(fixture.dev.join("private/INBOX/1,SA.eml").is_file());
        assert!(store::read_flags(&dir.join(&path)).unwrap().answered);
        let sent = &pushed.sent[0];
        assert_eq!(sent.copy, SentCopy::Saved);
        assert_eq!(sent.folder.as_deref(), Some("private/sent"));
        assert_eq!(sent.from.address.as_deref(), Some("me@example.com"));
        let raw = fs::read(submitted(&fixture, &sent.message_id)).unwrap();
        assert_eq!(
            raw,
            fs::read(fixture.dev.join("private/Sent/1,S.eml")).unwrap()
        );
        let copies = store::index(&dir.join("private/sent")).unwrap();
        let mail: MailFile = store::read(&copies[&1]).unwrap();
        assert_eq!(mail.subject, "Re: Offer for project");
        assert_eq!(mail.in_reply_to.as_deref(), Some("a1b2c3@example.com"));
        assert!(mail.flags.seen);
        assert!(mail.body.starts_with("Thanks!\n"), "{}", mail.body);
        assert!(fixture.push().await.sent.is_empty());

        let reply = Reply::prepare(&dir, Path::new(&path), false).unwrap();
        let once = reply
            .note(&dir, &Choice::default(), &REPLY, &PLAIN, "Once.")
            .unwrap();
        journal::mark_sent(&dir, once.id).unwrap();
        fs::remove_dir_all(fixture.dev.join("private/Sent")).unwrap();
        let reply = Reply::prepare(&dir, Path::new(&path), false).unwrap();
        let more = reply
            .note(&dir, &Choice::default(), &REPLY, &PLAIN, "More.")
            .unwrap();
        let pushed = fixture.push().await;
        assert_eq!(pushed.pushed, 3);
        assert!(!submitted(&fixture, &once.mail.message_id).exists());
        assert!(submitted(&fixture, &more.mail.message_id).is_file());
        assert_eq!(pushed.sent[1].copy, SentCopy::Failed);
        assert_eq!(
            pushed.sent[1].error.as_deref(),
            Some(fill(t().copy_failed, &[&t().no_sent_folder]).as_str())
        );
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[tokio::test]
    async fn a_new_mail_goes_out_from_the_account_and_a_refused_one_is_dropped() {
        let mut fixture = Fixture::new("send-new");
        fixture.account.smtp = Some(Smtp::new("smtp.example.com".to_string(), 465, None));
        fixture.save();
        fixture.sync().await;
        let people = |to: &str| {
            compose::choice(
                None,
                &[format!("Jane Doe <{to}>")],
                &[],
                &["tom@example.com".into()],
                Some("Lunch".into()),
            )
            .unwrap()
        };

        let err = NewMail::prepare(&fixture.dir, "work", false).err().unwrap();
        assert_eq!(err.to_string(), fill(t().unknown_account, &[&"work"]));

        let mail = NewMail::prepare(&fixture.dir, "private", false).unwrap();
        let noted = mail
            .note(
                &fixture.dir,
                &people("jane@example.com"),
                &PLAIN,
                "Tuesday?",
            )
            .unwrap();
        assert_eq!(noted.path, None);
        let line = serde_json::to_value(&noted).unwrap();
        assert!(line.get("path").is_none(), "{line}");
        let mail = NewMail::prepare(&fixture.dir, "private", false).unwrap();
        let refused = mail
            .note(&fixture.dir, &people("jane@example.invalid"), &PLAIN, "Hi")
            .unwrap();

        let pushed = fixture.push().await;
        assert_eq!(pushed.pushed, 1);
        let sent = serde_json::to_value(&pushed.sent[0]).unwrap();
        assert!(sent.get("path").is_none(), "{sent}");
        assert_eq!(sent["bcc"][0]["address"], "tom@example.com");
        let submitted = fs::read_to_string(submitted(&fixture, &noted.mail.message_id)).unwrap();
        assert!(!submitted.contains("tom@example.com"), "{submitted}");
        let dropped = &pushed.dropped[0];
        assert_eq!(dropped.path, None);
        assert_eq!(
            dropped.message_id.as_deref(),
            Some(refused.mail.message_id.as_str())
        );
        assert!(dropped.reason.contains("550"), "{}", dropped.reason);

        let copies = store::index(&fixture.dir.join("private/sent")).unwrap();
        let mail: MailFile = store::read(&copies[&1]).unwrap();
        assert_eq!(mail.subject, "Lunch");
        assert_eq!(mail.in_reply_to, None);
        assert_eq!(mail.to[0].address.as_deref(), Some("jane@example.com"));
        assert_eq!(mail.bcc[0].address.as_deref(), Some("tom@example.com"));
        assert_eq!(mail.body.trim_end(), "Tuesday?");
        assert_eq!(copies.len(), 1);
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[tokio::test]
    async fn attachments_of_the_original_go_along_even_when_it_moves_in_the_same_push() {
        let mut fixture = Fixture::new("send-attachments");
        fixture.account.smtp = Some(Smtp::new("smtp.example.com".to_string(), 465, None));
        fixture.save();
        let server_dir = fixture.dev.join("private");
        fixture.marked_folder("Trash", "trash");
        fs::write(
            server_dir.join("INBOX/2,S.eml"),
            include_bytes!("../tests/fixtures/multipart-attachment.eml"),
        )
        .unwrap();
        fixture.sync().await;
        let dir = &fixture.dir;
        let note = |path: &str, indices: &[u32]| {
            let forward = Answer {
                forward: true,
                ..REPLY
            };
            let picks: Vec<Pick> = indices
                .iter()
                .map(|index| Pick::Message(path.into(), *index))
                .collect();
            let request = Request {
                attachments: &picks,
                ..PLAIN
            };
            let choice =
                compose::choice(None, &["tom@example.com".into()], &[], &[], None).unwrap();
            Reply::prepare(dir, Path::new(path), false).unwrap().note(
                dir,
                &choice,
                &forward,
                &request,
                "See below.",
            )
        };
        let inbox = store::index(&dir.join("private/inbox")).unwrap();
        let path = store::relative(&inbox[&2], dir);

        let err = note(&path, &[0, 1]).err().unwrap();
        assert_eq!(err.to_string(), fill(t().attachment_none, &[&path, &1]));
        let noted = note(&path, &[0, 0]).unwrap();
        let line = serde_json::to_value(&noted).unwrap();
        assert_eq!(line["originals"][0]["folder"], "INBOX");
        assert_eq!(line["originals"][0]["uid"], 2);
        assert_eq!(line["originals"][0]["attachments"][0]["name"], "offer.pdf");
        assert_eq!(
            line["originals"][0]["attachments"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(line["originals"].as_array().unwrap().len(), 1);
        moving::run(dir, Path::new(&path), "trash", false).unwrap();

        let pushed = fixture.push().await;
        assert_eq!((pushed.pushed, pushed.dropped.len()), (2, 0));
        let raw = fs::read(submitted(&fixture, &noted.mail.message_id)).unwrap();
        let parsed = MessageParser::new().parse(&raw).unwrap();
        let attachment = parsed.attachments().next().unwrap();
        assert_eq!(attachment.attachment_name(), Some("offer.pdf"));
        assert_eq!(attachment.contents(), b"%PDF-1.4 fake");
        let copies = store::index(&dir.join("private/sent")).unwrap();
        let copy: MailFile = store::read(&copies[&1]).unwrap();
        assert_eq!(copy.attachments[0].name.as_deref(), Some("offer.pdf"));

        let trash = store::index(&dir.join("private/trash")).unwrap();
        let path = store::relative(&trash[&1], dir);
        let gone = note(&path, &[0]).unwrap();
        fs::write(server_dir.join("Trash/.uidvalidity"), "2").unwrap();
        let pushed = fixture.push().await;
        assert_eq!((pushed.pushed, pushed.sent.len()), (0, 0));
        let dropped = &pushed.dropped[0];
        assert_eq!(dropped.path.as_deref(), Some(path.as_str()));
        assert_eq!(
            dropped.message_id.as_deref(),
            Some(gone.mail.message_id.as_str())
        );
        assert_eq!(
            dropped.reason,
            fill(t().send_missing, &[&t().folder_changed])
        );
        assert!(!submitted(&fixture, &gone.mail.message_id).exists());
        assert!(fixture.push().await.dropped.is_empty());
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[test]
    fn an_attachment_is_an_index_of_the_draft_or_a_path_and_index() {
        assert_eq!("3".parse(), Ok(Pick::Draft(3)));
        assert_eq!(
            "private/inbox/re:-plan-7.json:2".parse(),
            Ok(Pick::Message("private/inbox/re:-plan-7.json".into(), 2))
        );
        for wrong in [
            "",
            "x",
            ":2",
            "private/inbox/x-7.json",
            "private/inbox/x-7.json:-1",
        ] {
            assert!(wrong.parse::<Pick>().is_err(), "{wrong}");
        }
    }

    #[tokio::test]
    async fn an_index_alone_needs_a_draft_and_a_message_the_same_account() {
        let fixture = Fixture::new("send-picks");
        fixture.marked_folder("Drafts", "drafts");
        fixture.sync().await;
        let dir = &fixture.dir;
        let note = |picks: &[Pick]| {
            let mut choice =
                compose::choice(None, &["jane@example.com".into()], &[], &[], None).unwrap();
            choice.draft = true;
            let request = Request {
                attachments: picks,
                ..PLAIN
            };
            NewMail::prepare(dir, "private", true)
                .unwrap()
                .note(dir, &choice, &request, "x")
                .err()
                .unwrap()
                .to_string()
        };

        assert_eq!(note(&[Pick::Draft(0)]), t().attachment_no_draft);
        let elsewhere = "work/inbox/2026-09-01_1000-x-1.json";
        assert_eq!(
            note(&[Pick::Message(elsewhere.into(), 0)]),
            fill(t().attachment_other_account, &[&elsewhere, &"private"])
        );
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[test]
    fn a_draft_goes_on_alone_once_its_original_is_gone() {
        let fixture = Fixture::new("send-alone");
        let dir = &fixture.dir;
        let gone = Path::new("private/inbox/2026-09-15_1432-offer-1.json");
        let continuing = Request {
            continues: Some("d1@example.com"),
            ..PLAIN
        };
        assert_eq!(
            draft_alone(dir, gone, &continuing).unwrap().as_deref(),
            Some("private")
        );
        assert_eq!(draft_alone(dir, gone, &PLAIN).unwrap(), None);
        let there = dir.join(gone);
        fs::create_dir_all(there.parent().unwrap()).unwrap();
        fs::write(&there, "{}").unwrap();
        assert_eq!(draft_alone(dir, gone, &continuing).unwrap(), None);
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[tokio::test]
    async fn own_files_go_out_read_at_the_push() {
        let mut fixture = Fixture::new("send-files");
        fixture.account.smtp = Some(Smtp::new("smtp.example.com".to_string(), 465, None));
        fixture.save();
        fixture.sync().await;
        let dir = &fixture.dir;
        let plan = fixture.base.join("plan.csv");
        let note = |files: &[&Path]| {
            let mut choice =
                compose::choice(None, &["jane@example.com".into()], &[], &[], None).unwrap();
            choice.files = files.iter().map(|file| file.to_path_buf()).collect();
            NewMail::prepare(dir, "private", false)
                .unwrap()
                .note(dir, &choice, &PLAIN, "The plan.")
        };

        let err = note(&[&plan]).err().unwrap();
        assert_eq!(err.to_string(), fill(t().send_no_file, &[&plan.display()]));
        fs::write(&plan, "day,task\nmonday,import\n").unwrap();
        let noted = note(&[&plan]).unwrap();
        let line = serde_json::to_value(&noted).unwrap();
        assert_eq!(line["files"][0]["path"], plan.to_str().unwrap());
        assert_eq!(line["files"][0]["name"], "plan.csv");
        assert_eq!(line["files"][0]["type"], "text/csv");
        assert_eq!(line["files"][0]["size"], 23);
        fs::write(&plan, "day,task\ntuesday,release\n").unwrap();

        let pushed = fixture.push().await;
        assert_eq!((pushed.pushed, pushed.dropped.len()), (1, 0));
        let raw = fs::read(submitted(&fixture, &noted.mail.message_id)).unwrap();
        let parsed = MessageParser::new().parse(&raw).unwrap();
        let attachment = parsed.attachments().next().unwrap();
        assert_eq!(attachment.attachment_name(), Some("plan.csv"));
        assert_eq!(attachment.contents(), b"day,task\ntuesday,release\n");
        let copies = store::index(&dir.join("private/sent")).unwrap();
        let copy: MailFile = store::read(&copies[&1]).unwrap();
        assert_eq!(copy.attachments[0].name.as_deref(), Some("plan.csv"));

        let gone = note(&[&plan]).unwrap();
        fs::remove_file(&plan).unwrap();
        let pushed = fixture.push().await;
        assert_eq!(
            pushed.dropped[0].message_id.as_deref(),
            Some(gone.mail.message_id.as_str())
        );
        assert!(
            pushed.dropped[0]
                .reason
                .starts_with(&fill(t().send_missing, &[&plan.display()]))
        );
        assert!(!submitted(&fixture, &gone.mail.message_id).exists());
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[tokio::test]
    async fn a_forward_marks_its_original_forwarded() {
        let mut fixture = Fixture::new("send-forward");
        fixture.account.smtp = Some(Smtp::new("smtp.example.com".to_string(), 465, None));
        fixture.save();
        fixture.sync().await;
        let dir = &fixture.dir;
        let inbox = store::index(&dir.join("private/inbox")).unwrap();
        let path = store::relative(&inbox[&1], dir);
        let forward = Answer {
            forward: true,
            ..REPLY
        };
        let choice = compose::choice(None, &["tom@example.com".into()], &[], &[], None).unwrap();
        let noted = Reply::prepare(dir, Path::new(&path), false)
            .unwrap()
            .note(dir, &choice, &forward, &PLAIN, "For you.")
            .unwrap();
        assert_eq!(noted.mail.subject, "Fwd: Offer for project");
        assert!(serde_json::to_value(&noted).unwrap()["forward"] == true);

        let pushed = fixture.push().await;
        assert_eq!((pushed.pushed, pushed.dropped.len()), (2, 0));
        assert!(
            fixture
                .dev
                .join("private/INBOX/1,S,$Forwarded.eml")
                .is_file()
        );
        let flags = store::read_flags(&dir.join(&path)).unwrap();
        assert!(flags.has(crate::message::FORWARDED) && !flags.answered);
        fs::remove_dir_all(&fixture.base).unwrap();
    }
}
