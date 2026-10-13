use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::ops::AddAssign;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use jiff::Zoned;
use serde::Serialize;

use crate::i18n::{fill, t};
use crate::imap::RawMessage;
use crate::kind::{self, Kinds};
use crate::message::{self, MailFile, Meta};
use crate::send::Sent;
use crate::server::{Access, Server};
use crate::settings::{self, Account};
use crate::store::{self, FolderState};
use crate::{imap, journal};

const FETCH_CHUNK: usize = 50;
const PROGRESS_EVERY: usize = 500;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Counts {
    pub new: usize,
    pub removed: usize,
    pub updated: usize,
}

impl AddAssign for Counts {
    fn add_assign(&mut self, other: Counts) {
        self.new += other.new;
        self.removed += other.removed;
        self.updated += other.updated;
    }
}

#[derive(Debug, Default)]
pub struct Synced {
    pub counts: Counts,
    pub added: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    All,
    Inbox,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub account: String,
    pub folder: String,
    pub fetched: usize,
    pub of: usize,
    pub folder_number: usize,
    pub folders: usize,
}

#[derive(Clone)]
pub enum Report {
    #[cfg(test)]
    Quiet,
    Text,
    Progress(Arc<dyn Fn(Progress) + Send + Sync>),
}

impl Report {
    fn text(&self) -> bool {
        matches!(self, Report::Text)
    }
}

struct Step<'a> {
    id: &'a str,
    name: &'a str,
    folder: String,
    number: usize,
    count: usize,
    report: &'a Report,
}

impl Step<'_> {
    fn fetched(&self, fetched: usize, of: usize) {
        if let Report::Progress(report) = self.report {
            report(Progress {
                account: self.id.to_string(),
                folder: self.folder.clone(),
                fetched,
                of,
                folder_number: self.number,
                folders: self.count,
            });
        }
    }
}

pub async fn sync_account(
    dir: &Path,
    id: &str,
    account: &Account,
    access: Access,
    report: &Report,
) -> Result<Synced> {
    let mut server = Server::connect(account, access).await?;
    let result = sync_folders(&mut server, dir, id, Scope::All, report).await;
    server.logout().await;
    result
}

pub async fn sync_folders(
    server: &mut Server,
    dir: &Path,
    id: &str,
    scope: Scope,
    report: &Report,
) -> Result<Synced> {
    let pushed = journal::push_account(server, dir, id).await?;
    if report.text() && pushed.pushed > 0 {
        println!("{}", fill(t().push_done, &[&id, &pushed.pushed]));
        for line in pushed.sent.iter().flat_map(Sent::lines) {
            println!("{line}");
        }
    }
    for dropped in &pushed.dropped {
        eprintln!("{}", dropped.text());
    }
    let account_dir = dir.join(id);
    let folders = server.list_folders().await?;
    if !folders.iter().any(imap::Folder::is_inbox) {
        bail!("{}", t().no_inbox_listed);
    }
    let account = assign_folders(dir, id, &folders, report)?;

    let mut by_path: BTreeMap<PathBuf, Vec<String>> = BTreeMap::new();
    let mut targets: Vec<(imap::Folder, PathBuf)> = Vec::new();
    for folder in folders {
        let rel = match folder.local_path() {
            Ok(rel) => rel,
            Err(err) => {
                eprintln!("{id}: {err:#}");
                continue;
            }
        };
        by_path
            .entry(rel.clone())
            .or_default()
            .push(folder.name.clone());
        if folder.selectable && account.synced_kind(&rel.to_string_lossy()).is_some() {
            targets.push((folder, rel));
        }
    }
    targets.sort_by(|(a, a_rel), (b, b_rel)| (a.is_inbox(), a_rel).cmp(&(b.is_inbox(), b_rel)));

    let mut synced = Synced::default();
    if scope == Scope::All {
        let kept: BTreeSet<&str> = targets.iter().map(|(f, _)| f.name.as_str()).collect();
        for (folder_dir, state) in store::local_folders(&account_dir)? {
            if !kept.contains(state.imap_name.as_str()) {
                let removed = store::remove_folder(&folder_dir)?;
                synced.counts += Counts {
                    removed,
                    ..Counts::default()
                };
                if report.text() {
                    println!(
                        "{}",
                        fill(t().folder_removed, &[&store::relative(&folder_dir, dir)])
                    );
                }
            }
        }
    } else {
        targets.retain(|(folder, _)| folder.is_inbox());
    }

    let count = targets.len();
    for (number, (folder, rel)) in targets.iter().enumerate() {
        let colliding = &by_path[rel];
        if colliding.len() > 1 {
            eprintln!(
                "{}",
                fill(
                    t().folder_collision,
                    &[&format!("{id}/{}", rel.display()), &colliding.join(", ")]
                )
            );
            continue;
        }
        let folder_dir = account_dir.join(rel);
        let name = store::relative(&folder_dir, dir);
        let step = Step {
            id,
            name: &name,
            folder: rel.to_string_lossy().into_owned(),
            number: number + 1,
            count,
            report,
        };
        let counts = sync_folder(server, folder, &folder_dir, &step, &mut synced.added)
            .await
            .with_context(|| name.clone())?;
        synced.counts += counts;
    }
    Ok(synced)
}

fn assign_folders(
    dir: &Path,
    id: &str,
    listed: &[imap::Folder],
    report: &Report,
) -> Result<Account> {
    let mut settings = settings::load(dir)?;
    let account = settings.account_mut(id)?;
    if !kind::assign(&mut account.folders, listed) {
        return Ok(account.clone());
    }
    let account = account.clone();
    settings::save(dir, &settings)?;
    if report.text()
        && let Some(note) = Kinds::of(&account).note(id)
    {
        println!("{note}");
    }
    Ok(account)
}

pub fn write_message(
    folder_dir: &Path,
    id: &str,
    imap_name: &str,
    raw: RawMessage,
) -> Result<(PathBuf, MailFile)> {
    let received = message::local_zoned(raw.received);
    let meta = Meta {
        account: id,
        folder: imap_name,
        uid: raw.uid,
        flags: raw.flags,
        received: raw.received,
        size: raw.size,
    };
    let mail = message::convert(&raw.body, meta);
    let path = folder_dir.join(store::file_name(&received, &mail.subject, raw.uid));
    store::write_atomic(&path, &mail)?;
    Ok((path, mail))
}

pub async fn fetch_new(
    server: &mut Server,
    folder_dir: &Path,
    id: &str,
    imap_name: &str,
    first: u32,
) -> Result<()> {
    server.examine(imap_name).await?;
    for raw in server.fetch_from(first).await? {
        write_message(folder_dir, id, imap_name, raw)?;
    }
    Ok(())
}

async fn sync_folder(
    server: &mut Server,
    folder: &imap::Folder,
    folder_dir: &Path,
    step: &Step<'_>,
    written: &mut Vec<PathBuf>,
) -> Result<Counts> {
    let (id, name, progress) = (step.id, step.name, step.report.text());
    let mailbox = server.examine(&folder.name).await?;
    fs::create_dir_all(folder_dir).with_context(|| format!("creating {}", folder_dir.display()))?;

    let fresh = FolderState {
        imap_name: folder.name.clone(),
        uidvalidity: mailbox.uid_validity,
        last_sync: None,
    };
    let mut removed = 0;
    let mut state = match store::read_state(folder_dir)? {
        Some(state) if state.uidvalidity == mailbox.uid_validity => state,
        Some(_) => {
            if progress {
                println!("{}", fill(t().uidvalidity_changed, &[&name]));
            }
            removed = store::clear_messages(folder_dir)?;
            fresh
        }
        None => fresh,
    };
    store::write_state(folder_dir, &state)?;

    let remote = if mailbox.exists == 0 {
        BTreeMap::new()
    } else {
        server.fetch_flags().await?
    };
    let local = store::index(folder_dir)?;

    let mut updated = 0;
    let mut broken = BTreeSet::new();
    for (uid, path) in &local {
        let Some(flags) = remote.get(uid) else {
            fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
            removed += 1;
            continue;
        };
        match store::read_flags(path) {
            Ok(current) if &current == flags => {}
            Ok(_) => {
                let mut mail: MailFile = store::read(path)?;
                mail.flags = flags.clone();
                store::write_atomic(path, &mail)?;
                updated += 1;
            }
            Err(err) => {
                eprintln!(
                    "{}",
                    fill(t().file_refetched, &[&path.display(), &format!("{err:#}")])
                );
                fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
                broken.insert(*uid);
            }
        }
    }

    let missing: Vec<u32> = remote
        .keys()
        .filter(|uid| !local.contains_key(uid) || broken.contains(uid))
        .copied()
        .collect();
    let total = missing.len();
    let mut added = 0;
    let mut next_report = PROGRESS_EVERY;
    step.fetched(added, total);
    for chunk in missing.chunks(FETCH_CHUNK) {
        for raw in server.fetch_messages(chunk).await? {
            let (path, _) = write_message(folder_dir, id, &folder.name, raw)?;
            written.push(path);
            added += 1;
        }
        step.fetched(added, total);
        if progress && added >= next_report && added < total {
            println!("{}", fill(t().folder_progress, &[&name, &added, &total]));
            next_report += PROGRESS_EVERY;
        }
    }

    state.last_sync = Some(message::format_rfc3339(&Zoned::now()));
    store::write_state(folder_dir, &state)?;
    if progress {
        println!(
            "{}",
            fill(t().folder_summary, &[&name, &added, &removed, &updated])
        );
    }
    Ok(Counts {
        new: added,
        removed,
        updated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::Kind;

    const PLAIN: &str = include_str!("../tests/fixtures/plain.eml");

    fn configured(dir: &Path, account: &Account) {
        settings::init(dir).unwrap();
        let mut settings = settings::load(dir).unwrap();
        settings
            .accounts
            .insert("private".to_string(), account.clone());
        settings::save(dir, &settings).unwrap();
    }

    #[tokio::test]
    async fn progress_is_reported_per_folder_and_fetched_chunk() {
        let base =
            std::env::temp_dir().join(format!("lamemail-sync-progress-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let server = base.join("server/private");
        for folder in ["INBOX", "Sent"] {
            fs::create_dir_all(server.join(folder)).unwrap();
            fs::write(server.join(folder).join(".uidvalidity"), "1").unwrap();
        }
        fs::write(server.join("Sent/.special"), "sent").unwrap();
        fs::write(server.join("INBOX/1.eml"), PLAIN).unwrap();
        fs::write(server.join("INBOX/2.eml"), PLAIN).unwrap();
        let account = Account::new(
            "imap.example.com".to_string(),
            993,
            "me@example.com".to_string(),
        );
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let report = Report::Progress(Arc::new({
            let seen = seen.clone();
            move |progress| seen.lock().unwrap().push(progress)
        }));
        let dir = base.join("mirror");
        configured(&dir, &account);
        sync_account(&dir, "private", &account, Access::Dev(server), &report)
            .await
            .unwrap();
        let seen = seen.lock().unwrap();
        let steps: Vec<(&str, usize, usize, usize, usize)> = seen
            .iter()
            .map(|p| {
                (
                    p.folder.as_str(),
                    p.fetched,
                    p.of,
                    p.folder_number,
                    p.folders,
                )
            })
            .collect();
        assert_eq!(
            steps,
            [
                ("sent", 0, 0, 1, 2),
                ("inbox", 0, 2, 2, 2),
                ("inbox", 2, 2, 2, 2)
            ]
        );
        assert!(seen.iter().all(|progress| progress.account == "private"));
        fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test]
    async fn a_server_that_lists_no_inbox_changes_nothing() {
        let base = std::env::temp_dir().join(format!("lamemail-sync-inbox-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let server = base.join("server/private");
        for folder in ["INBOX", "Sent", "Notes"] {
            fs::create_dir_all(server.join(folder)).unwrap();
            fs::write(server.join(folder).join(".uidvalidity"), "1").unwrap();
            fs::write(server.join(folder).join("1.eml"), PLAIN).unwrap();
        }
        fs::write(server.join("Sent/.special"), "sent").unwrap();
        let account = Account::new(
            "imap.example.com".to_string(),
            993,
            "me@example.com".to_string(),
        );
        let dir = base.join("mirror");
        configured(&dir, &account);
        let sync = |scope| {
            let (dir, server) = (dir.clone(), server.clone());
            async move {
                let access = Access::Dev(server);
                let account = settings::load(&dir).unwrap().account("private")?.clone();
                let mut server = Server::connect(&account, access).await?;
                sync_folders(&mut server, &dir, "private", scope, &Report::Quiet).await
            }
        };
        sync(Scope::All).await.unwrap();
        {
            let mut settings = settings::load(&dir).unwrap();
            let folders = &mut settings.account_mut("private").unwrap().folders;
            kind::reassign(folders, "notes", Some(Kind::Archive), None, false).unwrap();
            settings::save(&dir, &settings).unwrap();
        }
        assert_eq!(sync(Scope::All).await.unwrap().counts.new, 1);
        let assigned = || {
            settings::load(&dir).unwrap().accounts["private"]
                .folders
                .clone()
        };
        let files = || {
            ["inbox", "sent", "notes"].map(|folder| {
                store::index(&dir.join("private").join(folder))
                    .unwrap()
                    .len()
            })
        };
        let before = assigned();
        assert_eq!(before["notes"].kind, Some(Kind::Archive));

        for emptied in [vec!["INBOX"], vec!["Sent", "Notes"]] {
            for folder in emptied {
                fs::rename(server.join(folder), base.join(folder)).unwrap();
            }
            for scope in [Scope::Inbox, Scope::All] {
                let err = sync(scope).await.unwrap_err();
                assert_eq!(format!("{err:#}"), t().no_inbox_listed);
            }
            assert_eq!(assigned(), before);
            assert_eq!(files(), [1, 1, 1]);
        }
        fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test]
    async fn mirrors_a_development_server() {
        let base = std::env::temp_dir().join(format!("lamemail-sync-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let server = base.join("server/private");
        let inbox = server.join("INBOX");
        let drafts = server.join("Entw&APw-rfe");
        for folder in [&inbox, &drafts] {
            fs::create_dir_all(folder).unwrap();
            fs::write(folder.join(".uidvalidity"), "7").unwrap();
        }
        fs::write(drafts.join(".special"), "drafts").unwrap();
        fs::create_dir_all(server.join("[Gmail]")).unwrap();
        fs::write(inbox.join("1,S.eml"), PLAIN).unwrap();
        fs::write(inbox.join("2.eml"), PLAIN).unwrap();
        fs::write(drafts.join("3,D.eml"), PLAIN).unwrap();
        let dir = base.join("mirror");
        let account = Account::new(
            "imap.example.com".to_string(),
            993,
            "me@example.com".to_string(),
        );
        configured(&dir, &account);
        let sync = || async {
            sync_account(
                &dir,
                "private",
                &account,
                Access::Dev(server.clone()),
                &Report::Quiet,
            )
            .await
            .unwrap()
            .counts
        };
        let counts = |new, removed, updated| Counts {
            new,
            removed,
            updated,
        };
        let local = |folder: &str| store::index(&dir.join("private").join(folder)).unwrap();

        assert_eq!(sync().await, counts(3, 0, 0));
        let files = local("inbox");
        assert_eq!(files.keys().copied().collect::<Vec<_>>(), vec![1, 2]);
        assert!(store::read_flags(&files[&1]).unwrap().seen);
        assert_eq!(
            store::read::<MailFile>(&files[&2]).unwrap().subject,
            "Offer for project"
        );
        assert!(store::read_flags(&local("entwürfe")[&3]).unwrap().draft);
        assert!(!dir.join("private/[gmail]").exists());

        fs::rename(inbox.join("2.eml"), inbox.join("2,SF.eml")).unwrap();
        fs::remove_file(inbox.join("1,S.eml")).unwrap();
        fs::remove_dir_all(&drafts).unwrap();
        assert_eq!(sync().await, counts(0, 2, 1));
        let files = local("inbox");
        assert_eq!(files.keys().copied().collect::<Vec<_>>(), vec![2]);
        let flags = store::read_flags(&files[&2]).unwrap();
        assert!(flags.seen && flags.flagged);
        assert!(!dir.join("private/entwürfe").exists());

        let mut stale: MailFile = store::read(&files[&2]).unwrap();
        stale.subject = "stale".to_string();
        store::write_atomic(&files[&2], &stale).unwrap();
        fs::write(inbox.join(".uidvalidity"), "8").unwrap();
        assert_eq!(sync().await, counts(1, 1, 0));
        let files = local("inbox");
        assert_eq!(
            store::read::<MailFile>(&files[&2]).unwrap().subject,
            "Offer for project"
        );
        let state = store::read_state(&dir.join("private/inbox"))
            .unwrap()
            .unwrap();
        assert_eq!(state.uidvalidity, 8);

        let notes = server.join("Notes");
        fs::create_dir_all(&notes).unwrap();
        fs::write(notes.join(".uidvalidity"), "1").unwrap();
        fs::write(notes.join("1.eml"), PLAIN).unwrap();
        assert_eq!(sync().await, counts(0, 0, 0));
        assert!(!dir.join("private/notes").exists());
        {
            let mut settings = settings::load(&dir).unwrap();
            let folders = &mut settings.account_mut("private").unwrap().folders;
            kind::reassign(folders, "notes", Some(Kind::Archive), None, false).unwrap();
            settings::save(&dir, &settings).unwrap();
        }
        assert_eq!(sync().await, counts(1, 0, 0));
        assert_eq!(local("notes").len(), 1);
        fs::remove_dir_all(&base).unwrap();
    }
}
