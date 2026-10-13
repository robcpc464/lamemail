use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::{fs, io};

use anyhow::{Context, Result, bail};
use jiff::Timestamp;
use tokio::time::Instant;

use crate::compose::{File, Outgoing};
use crate::flag::Change;
use crate::i18n::{fill, t};
use crate::imap::{self, CredentialError, Folder, RawMessage, Special};
use crate::message::Flags;
use crate::net::Refused;
use crate::settings::{Account, Smtp};
use crate::{keyring, smtp, store};

const UIDVALIDITY_FILE: &str = ".uidvalidity";
const UIDNEXT_FILE: &str = ".uidnext";
const SPECIAL_FILE: &str = ".special";
const READ_ONLY_FILE: &str = ".readonly";
const DELIMITER: &str = "/";
const SUBMITTED_DIR: &str = ".smtp";
const DEV_REFUSED_DOMAIN: &str = ".invalid";
const DEV_POLL: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub enum Access {
    Password(String),
    Dev(PathBuf),
}

pub struct Mailbox {
    pub uid_validity: u32,
    pub exists: u32,
    pub keywords: bool,
}

pub struct Next {
    pub uid_validity: u32,
    pub uid: u32,
}

pub struct Server {
    link: Link,
    smtp: Option<Smtp>,
    username: String,
    access: Access,
}

enum Link {
    Imap(Box<imap::Session>),
    Dev {
        root: PathBuf,
        selected: BTreeMap<u32, DevMessage>,
    },
}

pub struct DevMessage {
    path: PathBuf,
    flags: Flags,
}

pub fn access(dev: Option<&Path>, id: &str) -> Result<Access> {
    if let Some(dev) = dev {
        return Ok(Access::Dev(dev.join(id)));
    }
    let password =
        keyring::lookup(id)?.ok_or_else(|| CredentialError(fill(t().no_password, &[&id, &id])))?;
    Ok(Access::Password(password))
}

fn smtp_login<'a>(smtp: &'a Smtp, username: &'a str, password: &'a str) -> smtp::Login<'a> {
    smtp::Login {
        host: &smtp.host,
        port: smtp.port,
        username,
        password,
    }
}

pub async fn check_smtp(smtp: &Smtp, username: &str, access: &Access) -> Result<bool> {
    match access {
        Access::Password(password) => {
            smtp::check(&smtp_login(smtp, username, password)).await?;
            Ok(true)
        }
        Access::Dev(_) => Ok(false),
    }
}

impl Server {
    pub async fn connect(account: &Account, access: Access) -> Result<Server> {
        let link = match &access {
            Access::Password(password) => Link::Imap(Box::new(
                imap::connect(&account.host, account.port, &account.username, password).await?,
            )),
            Access::Dev(root) => {
                if !root.is_dir() {
                    bail!("{}", fill(t().dev_no_account, &[&root.display()]));
                }
                Link::Dev {
                    root: root.clone(),
                    selected: BTreeMap::new(),
                }
            }
        };
        Ok(Server {
            link,
            smtp: account.smtp.clone(),
            username: account.username.clone(),
            access,
        })
    }

    pub fn copies_to_sent(&self) -> bool {
        self.smtp.as_ref().is_some_and(|smtp| smtp.copy_to_sent)
    }

    pub async fn submit(&self, outgoing: &Outgoing, files: &[File]) -> Result<()> {
        let smtp = self.smtp.as_ref().context("no SMTP server set")?;
        let message = outgoing.raw(files)?;
        match &self.access {
            Access::Password(password) => {
                let envelope = smtp::Envelope {
                    from: outgoing.sender(),
                    recipients: &outgoing.recipients(),
                    message: &message,
                };
                smtp::send(&smtp_login(smtp, &self.username, password), &envelope).await
            }
            Access::Dev(root) => {
                if let Some(refused) = outgoing
                    .recipients()
                    .into_iter()
                    .find(|address| address.ends_with(DEV_REFUSED_DOMAIN))
                {
                    bail!(Refused(format!("{refused}: RCPT TO: 550 no such domain")));
                }
                let dir = root.join(SUBMITTED_DIR);
                fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
                store::write_bytes_atomic(
                    &dir.join(format!("{}.eml", outgoing.message_id.replace('/', "_"))),
                    &message,
                )
            }
        }
    }

    pub async fn list_folders(&mut self) -> Result<Vec<Folder>> {
        match &mut self.link {
            Link::Imap(session) => imap::list_folders(session).await,
            Link::Dev { root, .. } => dev_folders(root),
        }
    }

    pub async fn examine(&mut self, name: &str) -> Result<Mailbox> {
        self.open(name, false).await
    }

    pub async fn select(&mut self, name: &str) -> Result<Mailbox> {
        self.open(name, true).await
    }

    async fn open(&mut self, name: &str, writable: bool) -> Result<Mailbox> {
        match &mut self.link {
            Link::Imap(session) => {
                let mailbox = if writable {
                    imap::select(session, name).await?
                } else {
                    imap::examine(session, name).await?
                };
                Ok(Mailbox {
                    uid_validity: mailbox.uid_validity.context("server sent no UIDVALIDITY")?,
                    exists: mailbox.exists,
                    keywords: imap::allows_keywords(&mailbox),
                })
            }
            Link::Dev { root, selected } => {
                let folder = root.join(name);
                let uid_validity = dev_validity(&folder)?;
                *selected = dev_messages(&folder)?;
                Ok(Mailbox {
                    uid_validity,
                    exists: selected.len().try_into()?,
                    keywords: true,
                })
            }
        }
    }

    pub async fn fetch_flags(&mut self) -> Result<BTreeMap<u32, Flags>> {
        match &mut self.link {
            Link::Imap(session) => imap::fetch_flags(session, "1:*").await,
            Link::Dev { selected, .. } => Ok(selected
                .iter()
                .map(|(uid, message)| (*uid, message.flags.clone()))
                .collect()),
        }
    }

    pub async fn fetch_flags_of(&mut self, uids: &[u32]) -> Result<BTreeMap<u32, Flags>> {
        match &mut self.link {
            Link::Imap(session) => imap::fetch_flags(session, &imap::uid_set(uids)).await,
            Link::Dev { selected, .. } => Ok(uids
                .iter()
                .filter_map(|uid| {
                    selected
                        .get(uid)
                        .map(|message| (*uid, message.flags.clone()))
                })
                .collect()),
        }
    }

    pub async fn fetch_messages(&mut self, uids: &[u32]) -> Result<Vec<RawMessage>> {
        match &mut self.link {
            Link::Imap(session) => imap::fetch_messages(session, uids).await,
            Link::Dev { selected, .. } => uids
                .iter()
                .filter_map(|uid| selected.get_key_value(uid))
                .map(|(uid, message)| read_dev_message(*uid, message))
                .collect(),
        }
    }

    pub async fn fetch_message(
        &mut self,
        folder: &str,
        uidvalidity: u32,
        uid: u32,
    ) -> Result<Vec<u8>> {
        if self.examine(folder).await?.uid_validity != uidvalidity {
            bail!(Refused(t().folder_changed.to_string()));
        }
        let message = self.fetch_messages(&[uid]).await?.into_iter().next();
        Ok(message
            .ok_or_else(|| Refused(t().message_gone.to_string()))?
            .body)
    }

    pub async fn store(&mut self, uids: &[u32], change: Change) -> Result<()> {
        match &mut self.link {
            Link::Imap(session) => {
                for query in change.queries() {
                    imap::store(session, uids, query).await?;
                }
                Ok(())
            }
            Link::Dev { selected, .. } => {
                for uid in uids {
                    let Some(message) = selected.get_mut(uid) else {
                        continue;
                    };
                    if message.path.with_file_name(READ_ONLY_FILE).exists() {
                        bail!(Refused("UID STORE: the folder is read-only".to_string()));
                    }
                    change.apply(&mut message.flags);
                    let renamed = message
                        .path
                        .with_file_name(dev_file_name(*uid, &message.flags));
                    fs::rename(&message.path, &renamed)
                        .with_context(|| format!("renaming {}", message.path.display()))?;
                    message.path = renamed;
                }
                Ok(())
            }
        }
    }

    pub async fn next_uid(&mut self, name: &str) -> Result<Next> {
        match &mut self.link {
            Link::Imap(session) => {
                let mailbox = imap::status(session, name).await?;
                Ok(Next {
                    uid_validity: mailbox.uid_validity.context("server sent no UIDVALIDITY")?,
                    uid: mailbox.uid_next.context("server sent no UIDNEXT")?,
                })
            }
            Link::Dev { root, .. } => {
                let folder = root.join(name);
                Ok(Next {
                    uid_validity: dev_validity(&folder)?,
                    uid: dev_next_uid(&folder)?,
                })
            }
        }
    }

    pub async fn create(&mut self, name: &str) -> Result<()> {
        match &mut self.link {
            Link::Imap(session) => imap::create(session, name).await,
            Link::Dev { root, .. } => {
                let folder = root.join(name);
                fs::create_dir_all(&folder)
                    .with_context(|| format!("creating {}", folder.display()))?;
                store::write_bytes_atomic(&folder.join(UIDVALIDITY_FILE), b"1\n")
            }
        }
    }

    pub async fn subscribe(&mut self, name: &str) -> Result<()> {
        match &mut self.link {
            Link::Imap(session) => imap::subscribe(session, name).await,
            Link::Dev { .. } => Ok(()),
        }
    }

    pub async fn move_to(&mut self, uids: &[u32], name: &str) -> Result<()> {
        match &mut self.link {
            Link::Imap(session) => imap::move_uids(session, uids, name).await,
            Link::Dev { root, selected } => {
                let target = root.join(name);
                dev_validity(&target)?;
                let mut next = dev_next_uid(&target)?;
                for uid in uids {
                    let Some(message) = selected.remove(uid) else {
                        continue;
                    };
                    let moved = target.join(dev_file_name(next, &message.flags));
                    fs::rename(&message.path, &moved)
                        .with_context(|| format!("moving {}", message.path.display()))?;
                    next += 1;
                }
                dev_keep_next_uid(&target, next)
            }
        }
    }

    pub async fn fetch_from(&mut self, first: u32) -> Result<Vec<RawMessage>> {
        match &mut self.link {
            Link::Imap(session) => imap::fetch_from(session, first).await,
            Link::Dev { selected, .. } => selected
                .range(first..)
                .map(|(uid, message)| read_dev_message(*uid, message))
                .collect(),
        }
    }

    pub async fn append(&mut self, name: &str, message: &[u8]) -> Result<()> {
        self.append_flagged(name, false, message).await
    }

    pub async fn append_draft(&mut self, name: &str, message: &[u8]) -> Result<()> {
        self.append_flagged(name, true, message).await
    }

    async fn append_flagged(&mut self, name: &str, draft: bool, message: &[u8]) -> Result<()> {
        match &mut self.link {
            Link::Imap(session) => {
                let flags = if draft {
                    "(\\Seen \\Draft)"
                } else {
                    "(\\Seen)"
                };
                imap::append(session, name, flags, message).await
            }
            Link::Dev { root, .. } => {
                let folder = root.join(name);
                dev_validity(&folder)?;
                let uid = dev_next_uid(&folder)?;
                let flags = Flags {
                    seen: true,
                    draft,
                    ..Flags::default()
                };
                store::write_bytes_atomic(&folder.join(dev_file_name(uid, &flags)), message)?;
                dev_keep_next_uid(&folder, uid + 1)
            }
        }
    }

    pub async fn expunge(&mut self, uids: &[u32]) -> Result<()> {
        match &mut self.link {
            Link::Imap(session) => imap::expunge(session, uids).await,
            Link::Dev { selected, .. } => {
                for uid in uids {
                    if let Some(message) = selected.remove(uid) {
                        fs::remove_file(&message.path)
                            .with_context(|| format!("removing {}", message.path.display()))?;
                    }
                }
                Ok(())
            }
        }
    }

    pub async fn wait_for_change(mut self, name: &str, limit: Duration) -> Result<(Server, bool)> {
        let changed = match self.link {
            Link::Imap(session) => {
                let (session, changed) = imap::idle(*session, name, limit).await?;
                self.link = Link::Imap(Box::new(session));
                changed
            }
            Link::Dev { root, selected } => {
                let changed = dev_poll(&root.join(name), limit).await?;
                self.link = Link::Dev { root, selected };
                changed
            }
        };
        Ok((self, changed))
    }

    pub async fn logout(self) {
        if let Link::Imap(mut session) = self.link {
            let _ = session.logout().await;
        }
    }
}

async fn dev_poll(folder: &Path, limit: Duration) -> Result<bool> {
    let before = dev_listing(folder)?;
    let deadline = Instant::now() + limit;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(false);
        }
        tokio::time::sleep(DEV_POLL.min(left)).await;
        if dev_listing(folder)? != before {
            return Ok(true);
        }
    }
}

fn dev_listing(folder: &Path) -> Result<(String, Vec<OsString>)> {
    let validity = folder.join(UIDVALIDITY_FILE);
    let validity =
        fs::read_to_string(&validity).with_context(|| format!("reading {}", validity.display()))?;
    let mut names = fs::read_dir(folder)
        .with_context(|| format!("reading {}", folder.display()))?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<io::Result<Vec<_>>>()
        .with_context(|| format!("reading {}", folder.display()))?;
    names.sort();
    Ok((validity, names))
}

fn dev_validity(folder: &Path) -> Result<u32> {
    let file = folder.join(UIDVALIDITY_FILE);
    if !file.is_file() {
        bail!(Refused(format!("{}: no such folder", folder.display())));
    }
    let text = fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
    text.trim()
        .parse()
        .with_context(|| format!("parsing {}", file.display()))
}

fn dev_next_uid(folder: &Path) -> Result<u32> {
    let listed = dev_messages(folder)?
        .last_key_value()
        .map_or(1, |(uid, _)| uid + 1);
    let kept = fs::read_to_string(folder.join(UIDNEXT_FILE))
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(1);
    Ok(listed.max(kept))
}

fn dev_keep_next_uid(folder: &Path, next: u32) -> Result<()> {
    store::write_bytes_atomic(&folder.join(UIDNEXT_FILE), next.to_string().as_bytes())
}

fn dev_folders(root: &Path) -> Result<Vec<Folder>> {
    let mut folders = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() || entry.file_name().to_string_lossy().starts_with('.')
            {
                continue;
            }
            let path = entry.path();
            let name = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join(DELIMITER);
            folders.push(Folder {
                name,
                delimiter: Some(DELIMITER.to_string()),
                selectable: path.join(UIDVALIDITY_FILE).is_file(),
                special: dev_special(&path),
            });
            pending.push(path);
        }
    }
    folders.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(folders)
}

fn dev_special(folder: &Path) -> Option<Special> {
    let marking = fs::read_to_string(folder.join(SPECIAL_FILE)).ok()?;
    serde_json::from_value(serde_json::Value::String(marking.trim().to_string())).ok()
}

fn dev_messages(folder: &Path) -> Result<BTreeMap<u32, DevMessage>> {
    let mut messages = BTreeMap::new();
    for entry in fs::read_dir(folder).with_context(|| format!("reading {}", folder.display()))? {
        let path = entry?.path();
        let Some(stem) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".eml"))
        else {
            continue;
        };
        let mut parts = stem.split(',');
        let Some(Ok(uid)) = parts.next().map(str::parse) else {
            continue;
        };
        let letters = parts.next().unwrap_or_default();
        match messages.entry(uid) {
            Entry::Vacant(slot) => {
                slot.insert(DevMessage {
                    flags: dev_flags(letters, parts),
                    path,
                });
            }
            Entry::Occupied(taken) => bail!(
                "{} and {} have the same UID",
                taken.get().path.display(),
                path.display()
            ),
        }
    }
    Ok(messages)
}

fn dev_flags<'a>(letters: &str, keywords: impl Iterator<Item = &'a str>) -> Flags {
    Flags {
        seen: letters.contains('S'),
        answered: letters.contains('A'),
        flagged: letters.contains('F'),
        draft: letters.contains('D'),
        keywords: keywords.map(str::to_string).collect(),
    }
}

fn dev_file_name(uid: u32, flags: &Flags) -> String {
    let letters: String = [
        (flags.seen, 'S'),
        (flags.answered, 'A'),
        (flags.flagged, 'F'),
        (flags.draft, 'D'),
    ]
    .into_iter()
    .filter_map(|(set, letter)| set.then_some(letter))
    .collect();
    if letters.is_empty() && flags.keywords.is_empty() {
        format!("{uid}.eml")
    } else {
        let parts = std::iter::once(letters).chain(flags.keywords.iter().cloned());
        format!("{uid},{}.eml", parts.collect::<Vec<_>>().join(","))
    }
}

fn read_dev_message(uid: u32, message: &DevMessage) -> Result<RawMessage> {
    let path = &message.path;
    let body = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let modified = fs::metadata(path)
        .and_then(|meta| meta.modified())
        .with_context(|| format!("reading the time of {}", path.display()))?;
    Ok(RawMessage {
        uid,
        flags: message.flags.clone(),
        received: Timestamp::try_from(modified)
            .with_context(|| format!("the time of {}", path.display()))?
            .as_second(),
        size: body.len() as u64,
        body,
    })
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use super::*;

    fn server_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lamemail-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[tokio::test]
    async fn a_development_server_is_a_folder_of_eml_files() {
        let root = server_dir("server");
        fs::create_dir_all(root.join("INBOX")).unwrap();
        fs::create_dir_all(root.join("[Gmail]/All Mail")).unwrap();
        fs::create_dir_all(root.join(".git/objects")).unwrap();
        fs::write(root.join("INBOX/.uidvalidity"), "7\n").unwrap();
        fs::write(root.join("[Gmail]/All Mail/.uidvalidity"), "9").unwrap();
        fs::write(root.join("[Gmail]/All Mail/.special"), "all\n").unwrap();
        fs::write(root.join("INBOX/1,SAD.eml"), "Subject: one\r\n\r\nHi\r\n").unwrap();
        fs::write(root.join("INBOX/2.eml"), "Subject: two\r\n\r\nHo\r\n").unwrap();
        fs::write(root.join("INBOX/notes.txt"), "not a message").unwrap();
        fs::File::options()
            .write(true)
            .open(root.join("INBOX/2.eml"))
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(1_700_000_000))
            .unwrap();

        let account = Account::new(
            "imap.example.com".to_string(),
            993,
            "me@example.com".to_string(),
        );
        let mut server = Server::connect(&account, Access::Dev(root.clone()))
            .await
            .unwrap();
        let folders: Vec<(String, bool, Option<Special>)> = server
            .list_folders()
            .await
            .unwrap()
            .into_iter()
            .map(|folder| (folder.name, folder.selectable, folder.special))
            .collect();
        assert_eq!(
            folders,
            vec![
                ("INBOX".to_string(), true, None),
                ("[Gmail]".to_string(), false, None),
                ("[Gmail]/All Mail".to_string(), true, Some(Special::All)),
            ]
        );

        let mailbox = server.examine("INBOX").await.unwrap();
        assert_eq!(mailbox.uid_validity, 7);
        assert_eq!(mailbox.exists, 2);
        let flags = server.fetch_flags().await.unwrap();
        assert!(flags[&1].seen && flags[&1].answered && flags[&1].draft && !flags[&1].flagged);
        assert_eq!(flags[&2], Flags::default());
        let fetched = server.fetch_messages(&[2, 5]).await.unwrap();
        assert_eq!(fetched.len(), 1);
        assert_eq!(fetched[0].uid, 2);
        assert_eq!(fetched[0].body, b"Subject: two\r\n\r\nHo\r\n");
        assert_eq!(fetched[0].received, 1_700_000_000);

        server
            .append("INBOX", b"Subject: three\r\n\r\n")
            .await
            .unwrap();
        assert_eq!(
            fs::read(root.join("INBOX/3,S.eml")).unwrap(),
            b"Subject: three\r\n\r\n"
        );
        assert!(server.append("[Gmail]", b"x").await.is_err());

        fs::write(root.join("INBOX/2,S.eml"), "Subject: two\r\n\r\nHo\r\n").unwrap();
        assert!(server.examine("INBOX").await.is_err());

        assert!(
            Server::connect(&account, Access::Dev(root.join("missing")))
                .await
                .is_err()
        );
        fs::remove_dir_all(&root).unwrap();
    }
}
