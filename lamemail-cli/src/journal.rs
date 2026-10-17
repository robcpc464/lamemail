use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

use crate::compose::Outgoing;
use crate::flag::Change;
use crate::i18n::{fill, t};
use crate::imap::Folder;
use crate::list::Located;
use crate::message::{Flags, MailFile};
use crate::net::Refused;
use crate::send::{self, Sent};
use crate::server::{self, Server};
use crate::settings::Settings;
use crate::store::FolderState;
use crate::{draft, folder_name, output, settings, show, store, sync};

const JOURNAL_FILE: &str = ".journal.json";
const JOURNAL_LOCK: &str = ".journal.lock";

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Journal {
    next: u64,
    entries: Vec<Entry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    outbox: Vec<Outbound>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: u64,
    pub path: String,
    pub uidvalidity: u32,
    #[serde(flatten)]
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum Action {
    Flag { change: Change },
    Move { folder: String },
    Discard,
}

impl Action {
    fn replaces(&self, other: &Action) -> bool {
        match (self, other) {
            (Action::Flag { change: mine }, Action::Flag { change: theirs }) => {
                mine.same_flag(*theirs)
            }
            (Action::Move { .. }, Action::Move { .. }) => true,
            (Action::Discard, _) => true,
            _ => false,
        }
    }
}

impl Entry {
    fn account(&self) -> &str {
        self.path.split('/').next().unwrap_or_default()
    }

    fn uid(&self) -> Option<u32> {
        store::uid_from_name(self.path.rsplit('/').next()?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Purpose {
    Send,
    Draft,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outbound {
    #[serde(rename = "action")]
    pub purpose: Purpose,
    pub id: u64,
    pub account: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub forward: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sent: bool,
    #[serde(flatten)]
    pub mail: Outgoing,
}

impl Outbound {
    fn is_draft(&self, account: &str, message_id: &str) -> bool {
        self.purpose == Purpose::Draft
            && self.account == account
            && self.mail.message_id == message_id
    }
}

#[derive(Default, Serialize)]
pub struct Pushed {
    pub pushed: usize,
    pub dropped: Vec<Dropped>,
    pub sent: Vec<Sent>,
}

#[derive(Debug, Serialize)]
pub struct Dropped {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    pub reason: String,
}

impl Dropped {
    pub fn text(&self) -> String {
        let what = self.path.as_deref().or(self.message_id.as_deref());
        fill(t().push_dropped, &[&what.unwrap_or_default(), &self.reason])
    }
}

#[derive(Serialize)]
#[serde(untagged)]
enum PushLine {
    Done {
        account: String,
        #[serde(flatten)]
        pushed: Pushed,
    },
    Failed {
        account: String,
        error: String,
    },
}

#[derive(Default)]
pub struct Overlay {
    changes: HashMap<String, Vec<Change>>,
    moves: HashMap<String, String>,
    discarded: HashSet<String>,
    leaving: HashSet<String>,
}

impl Overlay {
    pub fn apply(&self, path: &str, flags: &mut Flags) {
        for change in self.changes.get(path).into_iter().flatten() {
            change.apply(flags);
        }
    }

    pub fn relocate(&self, located: &mut Located, settings: &Settings) {
        if let Some(folder) = self.moves.get(&located.path) {
            located.kind = settings.kind_of(&located.account, folder);
            located.style = settings.style(located.kind);
            located.folder = folder.clone();
        }
    }

    pub fn folder_of(&self, path: &str) -> Option<&str> {
        self.moves.get(path).map(String::as_str)
    }

    pub fn discards(&self, path: &str) -> bool {
        self.discarded.contains(path)
    }

    pub fn hides(&self, path: &str) -> bool {
        self.discards(path) || self.leaving.contains(path)
    }
}

pub fn overlay(dir: &Path, settings: &Settings) -> Result<Overlay> {
    let mut overlay = Overlay::default();
    for entry in read(dir)?.entries {
        let account = settings.accounts.get(entry.account());
        match entry.action {
            Action::Flag { change } => overlay.changes.entry(entry.path).or_default().push(change),
            Action::Move { folder } => {
                if !account.is_some_and(|account| account.synced_kind(&folder).is_some()) {
                    overlay.leaving.insert(entry.path.clone());
                }
                overlay.moves.insert(entry.path, folder);
            }
            Action::Discard => {
                overlay.discarded.insert(entry.path);
            }
        }
    }
    Ok(overlay)
}

pub fn record_outbound(dir: &Path, mut outbound: Outbound) -> Result<Outbound> {
    edit(dir, |journal| {
        journal
            .outbox
            .retain(|waiting| !waiting.is_draft(&outbound.account, &outbound.mail.message_id));
        outbound.id = journal.next;
        journal.next += 1;
        journal.outbox.push(outbound.clone());
        outbound
    })
}

pub fn discard_draft(
    dir: &Path,
    account: &str,
    message_id: &str,
    copies: &[String],
    uidvalidity: u32,
) -> Result<()> {
    edit(dir, |journal| {
        journal
            .outbox
            .retain(|waiting| !waiting.is_draft(account, message_id));
        for path in copies {
            journal.entries.retain(|entry| &entry.path != path);
            journal.entries.push(Entry {
                id: journal.next,
                path: path.clone(),
                uidvalidity,
                action: Action::Discard,
            });
            journal.next += 1;
        }
    })
}

pub fn waiting_draft(dir: &Path, account: &str, message_id: &str) -> Result<Option<Outgoing>> {
    Ok(read(dir)?
        .outbox
        .into_iter()
        .find(|waiting| waiting.is_draft(account, message_id))
        .map(|waiting| waiting.mail))
}

pub fn mark_sent(dir: &Path, id: u64) -> Result<()> {
    edit(dir, |journal| {
        if let Some(outbound) = journal.outbox.iter_mut().find(|outbound| outbound.id == id) {
            outbound.sent = true;
        }
    })
}

pub fn forget_mail(dir: &Path, id: u64) -> Result<bool> {
    edit(dir, |journal| {
        let before = journal.outbox.len();
        journal.outbox.retain(|outbound| outbound.id != id);
        journal.outbox.len() < before
    })
}

impl Journal {
    fn put(&mut self, path: &str, uidvalidity: u32, action: Action) {
        let discarded = |entry: &Entry| entry.path == path && entry.action == Action::Discard;
        if self.entries.iter().any(discarded) {
            return;
        }
        self.withdraw(path, &action);
        self.entries.push(Entry {
            id: self.next,
            path: path.to_string(),
            uidvalidity,
            action,
        });
        self.next += 1;
    }

    fn withdraw(&mut self, path: &str, action: &Action) {
        self.entries
            .retain(|entry| entry.path != path || !action.replaces(&entry.action));
    }
}

pub fn record(dir: &Path, path: &str, uidvalidity: u32, action: Action) -> Result<()> {
    edit(dir, |journal| journal.put(path, uidvalidity, action))
}

pub fn note_move(
    dir: &Path,
    path: &str,
    uidvalidity: u32,
    verdict: Option<Change>,
    folder: Option<String>,
) -> Result<()> {
    edit(dir, |journal| {
        match verdict {
            Some(change) => journal.put(path, uidvalidity, Action::Flag { change }),
            None => journal.withdraw(
                path,
                &Action::Flag {
                    change: Change::Junk,
                },
            ),
        }
        match folder {
            Some(folder) => journal.put(path, uidvalidity, Action::Move { folder }),
            None => journal.withdraw(
                path,
                &Action::Move {
                    folder: String::new(),
                },
            ),
        }
    })
}

pub fn forget_account(dir: &Path, id: &str) -> Result<()> {
    edit(dir, |journal| {
        journal.entries.retain(|entry| entry.account() != id);
        journal.outbox.retain(|outbound| outbound.account != id);
    })
}

fn read(dir: &Path) -> Result<Journal> {
    Ok(store::read_optional(&dir.join(JOURNAL_FILE))?.unwrap_or_default())
}

fn edit<T>(dir: &Path, change: impl FnOnce(&mut Journal) -> T) -> Result<T> {
    let _lock = store::lock(&dir.join(JOURNAL_LOCK))?;
    let before = read(dir)?;
    let mut journal = before.clone();
    let result = change(&mut journal);
    if journal != before {
        store::write_atomic(&dir.join(JOURNAL_FILE), &journal)?;
    }
    Ok(result)
}

pub async fn push_account(server: &mut Server, dir: &Path, id: &str) -> Result<Pushed> {
    let journal = read(dir)?;
    let outbox: Vec<Outbound> = journal
        .outbox
        .into_iter()
        .filter(|outbound| outbound.account == id)
        .collect();
    let mut files = send::fetch_files(server, &outbox).await?;
    let (drafts, mails): (Vec<Outbound>, Vec<Outbound>) = outbox
        .into_iter()
        .partition(|outbound| outbound.purpose == Purpose::Draft);
    let mut pushed = Pushed::default();
    push_entries(server, dir, id, journal.entries, &mut pushed).await?;
    if !drafts.is_empty() {
        draft::push(server, dir, id, drafts, &mut files, &mut pushed).await?;
    }
    if !mails.is_empty() {
        send::push(server, dir, id, mails, files, &mut pushed).await?;
        let marks = read(dir)?.entries;
        push_entries(server, dir, id, marks, &mut pushed).await.ok();
    }
    Ok(pushed)
}

async fn push_entries(
    server: &mut Server,
    dir: &Path,
    id: &str,
    entries: Vec<Entry>,
    pushed: &mut Pushed,
) -> Result<()> {
    let mut by_folder: BTreeMap<PathBuf, Vec<Entry>> = BTreeMap::new();
    for entry in entries {
        if entry.account() == id {
            let folder = dir.join(&entry.path).parent().map(Path::to_path_buf);
            by_folder
                .entry(folder.unwrap_or_default())
                .or_default()
                .push(entry);
        }
    }
    let mut done = BTreeSet::new();
    for (folder_dir, entries) in by_folder {
        let folder = FolderPush {
            dir,
            id,
            folder_dir: &folder_dir,
        }
        .push(server, &entries)
        .await?;
        pushed.pushed += folder.pushed.len();
        done.extend(folder.pushed);
        for (entry, reason) in folder.dropped {
            done.insert(entry.id);
            pushed.dropped.push(Dropped {
                path: Some(entry.path.clone()),
                message_id: None,
                reason,
            });
        }
    }
    if !done.is_empty() {
        edit(dir, |journal| {
            journal.entries.retain(|entry| !done.contains(&entry.id))
        })?;
    }
    Ok(())
}

struct FolderPush<'a> {
    dir: &'a Path,
    id: &'a str,
    folder_dir: &'a Path,
}

#[derive(Default)]
struct Outcome<'e> {
    pushed: Vec<u64>,
    dropped: Vec<(&'e Entry, String)>,
}

impl Outcome<'_> {
    fn drop_all<'e>(entries: &'e [Entry], reason: &str) -> Outcome<'e> {
        Outcome {
            pushed: Vec::new(),
            dropped: entries
                .iter()
                .map(|entry| (entry, reason.to_string()))
                .collect(),
        }
    }
}

impl FolderPush<'_> {
    async fn push<'e>(&self, server: &mut Server, entries: &'e [Entry]) -> Result<Outcome<'e>> {
        let Some(state) = store::read_state(self.folder_dir)? else {
            return Ok(Outcome::drop_all(entries, t().folder_changed));
        };
        let mailbox = match server.select(&state.imap_name).await {
            Ok(mailbox) => mailbox,
            Err(err) if err.is::<Refused>() => {
                return Ok(Outcome::drop_all(entries, t().folder_changed));
            }
            Err(err) => return Err(err),
        };
        if mailbox.uid_validity != state.uidvalidity {
            return Ok(Outcome::drop_all(entries, t().folder_changed));
        }
        let mut outcome = Outcome::default();
        let mut valid = Vec::new();
        for entry in entries {
            match entry.uid() {
                Some(uid) if entry.uidvalidity == state.uidvalidity => valid.push((entry, uid)),
                Some(_) => outcome
                    .dropped
                    .push((entry, t().folder_changed.to_string())),
                None => outcome.dropped.push((entry, t().message_gone.to_string())),
            }
        }
        let changes = Change::ALL
            .into_iter()
            .filter(|change| mailbox.keywords || !change.is_keyword());
        for change in changes {
            let flagging = |entry: &Entry| entry.action == Action::Flag { change };
            let uids: Vec<u32> = valid
                .iter()
                .filter(|(entry, _)| flagging(entry))
                .map(|(_, uid)| *uid)
                .collect();
            if uids.is_empty() {
                continue;
            }
            match server.store(&uids, change).await {
                Ok(()) => {}
                Err(err) if err.is::<Refused>() => {
                    let reason = format!("{err:#}");
                    let (refused, kept): (Vec<_>, Vec<_>) =
                        valid.into_iter().partition(|(entry, _)| flagging(entry));
                    valid = kept;
                    outcome.dropped.extend(
                        refused
                            .into_iter()
                            .map(|(entry, _)| (entry, reason.clone())),
                    );
                }
                Err(err) => return Err(err),
            }
        }
        let uids: BTreeSet<u32> = valid.iter().map(|(_, uid)| *uid).collect();
        let flags = server
            .fetch_flags_of(&uids.into_iter().collect::<Vec<_>>())
            .await?;
        let mut written = BTreeSet::new();
        let mut moves: BTreeMap<&str, Vec<(&Entry, u32)>> = BTreeMap::new();
        let mut discards = Vec::new();
        for (entry, uid) in valid {
            let Some(flags) = flags.get(&uid) else {
                if entry.action == Action::Discard {
                    remove_file(&self.dir.join(&entry.path))?;
                    outcome.pushed.push(entry.id);
                } else {
                    outcome.dropped.push((entry, t().message_gone.to_string()));
                }
                continue;
            };
            match &entry.action {
                Action::Flag { .. } => {
                    outcome.pushed.push(entry.id);
                    if written.insert(uid) {
                        write_flags(&self.dir.join(&entry.path), flags)?;
                    }
                }
                Action::Move { folder } => moves.entry(folder).or_default().push((entry, uid)),
                Action::Discard => discards.push((entry, uid)),
            }
        }
        self.push_discards(server, discards, &mut outcome).await?;
        self.push_moves(server, moves, &mut outcome).await?;
        Ok(outcome)
    }

    async fn push_discards<'e>(
        &self,
        server: &mut Server,
        items: Vec<(&'e Entry, u32)>,
        outcome: &mut Outcome<'e>,
    ) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }
        let uids: Vec<u32> = items.iter().map(|(_, uid)| *uid).collect();
        match server.expunge(&uids).await {
            Ok(()) => {}
            Err(err) if err.is::<Refused>() => {
                let reason = format!("{err:#}");
                outcome
                    .dropped
                    .extend(items.iter().map(|(entry, _)| (*entry, reason.clone())));
                return Ok(());
            }
            Err(err) => return Err(err),
        }
        for (entry, _) in items {
            remove_file(&self.dir.join(&entry.path))?;
            outcome.pushed.push(entry.id);
        }
        Ok(())
    }

    async fn push_moves<'e>(
        &self,
        server: &mut Server,
        moves: BTreeMap<&str, Vec<(&'e Entry, u32)>>,
        outcome: &mut Outcome<'e>,
    ) -> Result<()> {
        let account_dir = self.dir.join(self.id);
        let source = store::relative(self.folder_dir, &account_dir);
        let mut arriving = Vec::new();
        for (target, items) in moves {
            if target == source {
                outcome
                    .pushed
                    .extend(items.iter().map(|(entry, _)| entry.id));
                continue;
            }
            let target_dir = account_dir.join(target);
            let uids: Vec<u32> = items.iter().map(|(_, uid)| *uid).collect();
            let moved = async {
                let found = self.target(server, target, &target_dir).await?;
                server.move_to(&uids, &found.imap_name).await?;
                Ok::<_, anyhow::Error>(found)
            }
            .await;
            let found = match moved {
                Ok(found) => found,
                Err(err) if err.is::<Refused>() => {
                    let reason = format!("{err:#}");
                    outcome
                        .dropped
                        .extend(items.iter().map(|(entry, _)| (*entry, reason.clone())));
                    continue;
                }
                Err(err) => return Err(err),
            };
            for (entry, _) in &items {
                remove_file(&self.dir.join(&entry.path))?;
                outcome.pushed.push(entry.id);
            }
            if let Some(first) = found.first_new {
                arriving.push((target_dir, found.imap_name, first));
            }
        }
        for (target_dir, imap_name, first) in arriving {
            sync::fetch_new(server, &target_dir, self.id, &imap_name, first).await?;
        }
        Ok(())
    }

    async fn target(&self, server: &mut Server, target: &str, target_dir: &Path) -> Result<Target> {
        let settings = settings::load(self.dir)?;
        let mirrored = settings.account(self.id)?.synced_kind(target).is_some();
        if mirrored && let Some(state) = store::read_state(target_dir)? {
            let next = server.next_uid(&state.imap_name).await?;
            return Ok(Target {
                first_new: (next.uid_validity == state.uidvalidity).then_some(next.uid),
                imap_name: state.imap_name,
            });
        }
        let listed = server.list_folders().await?;
        let found = listed
            .iter()
            .find(|folder| folder.selectable && folder.is_at(target));
        let imap_name = match found {
            Some(folder) => folder.name.clone(),
            None if !mirrored => bail!(Refused(t().push_no_target.to_string())),
            None => {
                let name = year_name(&listed, target)
                    .ok_or_else(|| Refused(fill(t().push_cannot_create, &[&target])))?;
                server.create(&name).await?;
                if let Err(err) = server.subscribe(&name).await
                    && !err.is::<Refused>()
                {
                    return Err(err);
                }
                name
            }
        };
        if !mirrored {
            return Ok(Target {
                imap_name,
                first_new: None,
            });
        }
        let next = server.next_uid(&imap_name).await?;
        fs::create_dir_all(target_dir)
            .with_context(|| format!("creating {}", target_dir.display()))?;
        store::write_state(
            target_dir,
            &FolderState {
                imap_name: imap_name.clone(),
                uidvalidity: next.uid_validity,
                last_sync: None,
            },
        )?;
        Ok(Target {
            imap_name,
            first_new: Some(next.uid),
        })
    }
}

struct Target {
    imap_name: String,
    first_new: Option<u32>,
}

fn year_name(listed: &[Folder], target: &str) -> Option<String> {
    let (parent, year) = target
        .rsplit_once('/')
        .filter(|(_, year)| folder_name::is_year(year))?;
    let parent = listed.iter().find(|folder| folder.is_at(parent))?;
    Some(format!(
        "{}{}{year}",
        parent.name,
        parent.delimiter.as_deref()?
    ))
}

fn remove_file(file: &Path) -> Result<()> {
    if file.exists() {
        fs::remove_file(file).with_context(|| format!("removing {}", file.display()))?;
    }
    Ok(())
}

fn write_flags(file: &Path, flags: &Flags) -> Result<()> {
    let Some(mut mail) = store::read_optional::<MailFile>(file)? else {
        return Ok(());
    };
    if &mail.flags != flags {
        mail.flags = flags.clone();
        store::write_atomic(file, &mail)?;
    }
    Ok(())
}

pub async fn run_push(dir: &Path, dev: Option<&Path>, json: bool) -> Result<()> {
    let settings = settings::load(dir)?;
    let _lock = store::wait_for_lock(dir)?;
    let journal = read(dir)?;
    let ids: BTreeSet<String> = journal
        .entries
        .iter()
        .map(|entry| entry.account().to_string())
        .chain(journal.outbox.into_iter().map(|outbound| outbound.account))
        .collect();
    if ids.is_empty() && !json {
        println!("{}", t().journal_empty);
    }
    let mut tasks = JoinSet::new();
    for id in ids {
        let dir = dir.to_path_buf();
        let prepared = settings
            .account(&id)
            .cloned()
            .and_then(|account| Ok((account, server::access(dev, &id)?)));
        tasks.spawn(async move {
            let result = async {
                let (account, access) = prepared?;
                let mut server = Server::connect(&account, access).await?;
                let pushed = push_account(&mut server, &dir, &id).await;
                server.logout().await;
                pushed
            }
            .await;
            (id, result)
        });
    }
    let mut failed = false;
    while let Some(joined) = tasks.join_next().await {
        let (account, result) = joined.context("push task panicked")?;
        let line = match result {
            Ok(pushed) => PushLine::Done { account, pushed },
            Err(err) => {
                failed = true;
                PushLine::Failed {
                    account,
                    error: format!("{err:#}"),
                }
            }
        };
        output::stdout(|out| {
            if json {
                output::json_line(out, &line)
            } else {
                writeln!(out, "{}", push_text(&line))
            }
        })?;
    }
    if failed {
        bail!("{}", t().push_incomplete);
    }
    Ok(())
}

fn push_text(line: &PushLine) -> String {
    match line {
        PushLine::Done { account, pushed } => {
            let mut lines = vec![fill(t().push_done, &[account, &pushed.pushed])];
            lines.extend(pushed.sent.iter().flat_map(Sent::lines));
            lines.extend(pushed.dropped.iter().map(Dropped::text));
            lines.join("\n")
        }
        PushLine::Failed { account, error } => fill(t().sync_failed, &[account, error]),
    }
}

pub fn run_list(dir: &Path, json: bool) -> Result<()> {
    let journal = read(dir)?;
    output::stdout(|out| {
        if !json && journal.entries.is_empty() && journal.outbox.is_empty() {
            writeln!(out, "{}", t().journal_empty)?;
        }
        for entry in &journal.entries {
            if json {
                output::json_line(out, entry)?;
            } else {
                let what = match &entry.action {
                    Action::Flag { change } => change.as_str().to_string(),
                    Action::Move { folder } => format!("→ {folder}"),
                    Action::Discard => "discard".to_string(),
                };
                writeln!(out, "{} {what}", entry.path)?;
            }
        }
        for outbound in &journal.outbox {
            if json {
                output::json_line(out, outbound)?;
            } else {
                let label = match outbound.purpose {
                    Purpose::Send => String::new(),
                    Purpose::Draft => format!(" ({})", t().draft_label),
                };
                let people = match outbound.mail.to.as_slice() {
                    [] => String::new(),
                    to => format!(" → {}", show::people(to)),
                };
                writeln!(
                    out,
                    "{}{label}{people}: {}",
                    outbound.account, outbound.mail.subject
                )?;
            }
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::flag::{self, Change};
    use crate::moving;
    use crate::testing::Fixture;

    #[test]
    fn a_mail_noted_with_one_original_still_reads() {
        let entry: Outbound = serde_json::from_value(serde_json::json!({
            "action": "send", "id": 8, "account": "private", "message_id": "m@example.com",
            "date": "2026-09-22T16:13:20+02:00", "from": {"name": null, "address": "me@example.com"},
            "to": [], "cc": [], "bcc": [], "subject": "", "body": "",
            "original": {"folder": "INBOX", "uidvalidity": 1, "uid": 2, "attachments": []}
        }))
        .unwrap();
        assert_eq!(entry.mail.originals.len(), 1);
        let line = serde_json::to_value(&entry).unwrap();
        assert!(line.get("original").is_none());
        assert_eq!(line["originals"][0]["uid"], 2);
    }

    fn changes(dir: &Path) -> Vec<(String, Change)> {
        read(dir)
            .unwrap()
            .entries
            .into_iter()
            .filter_map(|entry| match entry.action {
                Action::Flag { change } => Some((entry.path, change)),
                Action::Move { .. } | Action::Discard => None,
            })
            .collect()
    }

    #[test]
    fn the_journal_keeps_the_last_wish_per_message_and_flag() {
        let fixture = Fixture::new("journal-record");
        let dir = &fixture.dir;
        for change in [Change::Seen, Change::Flagged, Change::Unseen] {
            record(dir, "private/inbox/a-1.json", 1, Action::Flag { change }).unwrap();
        }
        record(
            dir,
            "private/inbox/b-2.json",
            1,
            Action::Flag {
                change: Change::Seen,
            },
        )
        .unwrap();
        assert_eq!(
            changes(dir),
            [
                ("private/inbox/a-1.json".to_string(), Change::Flagged),
                ("private/inbox/a-1.json".to_string(), Change::Unseen),
                ("private/inbox/b-2.json".to_string(), Change::Seen),
            ]
        );
        let ids: Vec<u64> = read(dir)
            .unwrap()
            .entries
            .iter()
            .map(|entry| entry.id)
            .collect();
        assert_eq!(ids, [1, 2, 3]);

        let mut flags = Flags {
            seen: true,
            ..Flags::default()
        };
        overlay(dir, &Settings::default())
            .unwrap()
            .apply("private/inbox/a-1.json", &mut flags);
        assert!(flags.flagged && !flags.seen);

        record(
            dir,
            "business/inbox/c-3.json",
            1,
            Action::Flag {
                change: Change::Seen,
            },
        )
        .unwrap();
        forget_account(dir, "private").unwrap();
        assert_eq!(
            changes(dir),
            [("business/inbox/c-3.json".to_string(), Change::Seen)]
        );
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[tokio::test]
    async fn a_refused_flag_is_dropped_and_the_rest_of_the_folder_is_pushed() {
        let fixture = Fixture::new("journal-refused");
        let dir = &fixture.dir;
        fs::write(fixture.dev.join("private/INBOX/2.eml"), "Subject: two\n\n").unwrap();
        fixture.sync().await;
        let inbox = store::index(&dir.join("private/inbox")).unwrap();
        let [one, two] = [1, 2].map(|uid| store::relative(&inbox[&uid], dir));
        flag::run(dir, Path::new(&one), Change::Flagged).unwrap();
        flag::run(dir, Path::new(&two), Change::Flagged).unwrap();
        moving::run(dir, Path::new(&two), "sent", false).unwrap();
        fs::write(fixture.dev.join("private/INBOX/.readonly"), "").unwrap();

        let pushed = fixture.push().await;
        assert_eq!(pushed.pushed, 1);
        let dropped: Vec<_> = pushed
            .dropped
            .iter()
            .map(|dropped| (dropped.path.as_deref(), dropped.reason.as_str()))
            .collect();
        let reason = "UID STORE: the folder is read-only";
        assert_eq!(
            dropped,
            [(Some(one.as_str()), reason), (Some(two.as_str()), reason)]
        );
        assert!(read(dir).unwrap().entries.is_empty());
        assert!(!store::read_flags(&inbox[&1]).unwrap().flagged);
        assert!(!inbox[&2].exists());
        assert_eq!(store::index(&dir.join("private/sent")).unwrap().len(), 1);
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[tokio::test]
    async fn a_discard_is_the_last_word_and_done_when_the_message_is_gone() {
        let fixture = Fixture::new("journal-discard");
        let dir = &fixture.dir;
        fixture.sync().await;
        let inbox = store::index(&dir.join("private/inbox")).unwrap();
        let path = store::relative(&inbox[&1], dir);
        record(dir, &path, 1, Action::Discard).unwrap();
        flag::run(dir, Path::new(&path), Change::Flagged).unwrap();
        assert!(moving::run(dir, Path::new(&path), "sent", false).is_err());
        record(
            dir,
            &path,
            1,
            Action::Move {
                folder: "sent".to_string(),
            },
        )
        .unwrap();
        let entries = read(dir).unwrap().entries;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].action, Action::Discard);
        fs::remove_file(fixture.dev.join("private/INBOX/1,S.eml")).unwrap();
        let pushed = fixture.push().await;
        assert_eq!((pushed.pushed, pushed.dropped.len()), (1, 0));
        assert!(!inbox[&1].exists());
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[tokio::test]
    async fn a_move_leaves_the_last_wish_and_mirrors_only_source_and_target() {
        let fixture = Fixture::new("journal-move");
        let (dir, server_dir) = (&fixture.dir, fixture.dev.join("private"));
        fixture.marked_folder("Trash", "trash");
        fs::write(server_dir.join("INBOX/2.eml"), "Subject: two\r\n\r\nHi\r\n").unwrap();
        fixture.sync().await;
        let files = store::index(&dir.join("private/inbox")).unwrap();
        let path = |uid: u32| store::relative(&files[&uid], dir);
        let one = path(1);
        let two = path(2);
        moving::run(dir, Path::new(&one), "trash", false).unwrap();
        moving::run(dir, Path::new(&one), "inbox", false).unwrap();
        moving::run(dir, Path::new(&one), "trash", false).unwrap();
        flag::run(dir, Path::new(&one), Change::Flagged).unwrap();
        moving::run(dir, Path::new(&two), "trash", false).unwrap();
        moving::run(dir, Path::new(&two), "inbox", false).unwrap();
        let overlay = overlay(dir, &fixture.settings()).unwrap();
        assert_eq!(overlay.folder_of(&one), Some("trash"));
        assert_eq!(overlay.folder_of(&two), None);
        assert_eq!(read(dir).unwrap().entries.len(), 2);

        let pushed = fixture.push().await;
        assert_eq!((pushed.pushed, pushed.dropped.len()), (2, 0));
        assert!(server_dir.join("Trash/1,SF.eml").is_file());
        assert!(server_dir.join("INBOX/2.eml").is_file());
        assert!(!dir.join(&one).exists() && dir.join(&two).exists());
        let trash = store::index(&dir.join("private/trash")).unwrap();
        assert!(store::read_flags(&trash[&1]).unwrap().flagged);
        assert!(read(dir).unwrap().entries.is_empty());
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[tokio::test]
    async fn moving_into_junk_and_out_again_marks_the_mail() {
        let fixture = Fixture::new("journal-junk");
        let (dir, server_dir) = (&fixture.dir, fixture.dev.join("private"));
        fixture.marked_folder("Junk", "junk");
        fixture.sync().await;
        let inbox = store::index(&dir.join("private/inbox")).unwrap();
        let path = store::relative(&inbox[&1], dir);

        moving::run(dir, Path::new(&path), "junk", false).unwrap();
        assert_eq!(changes(dir), [(path.clone(), Change::Junk)]);
        moving::run(dir, Path::new(&path), "inbox", false).unwrap();
        assert!(changes(dir).is_empty());
        moving::run(dir, Path::new(&path), "junk", false).unwrap();
        let pushed = fixture.push().await;
        assert_eq!((pushed.pushed, pushed.dropped.len()), (2, 0));
        assert!(server_dir.join("Junk/1,S,$Junk.eml").is_file());

        let junk = store::index(&dir.join("private/junk")).unwrap();
        let path = store::relative(&junk[&1], dir);
        assert!(store::read_flags(&dir.join(&path)).unwrap().has("$Junk"));
        moving::run(dir, Path::new(&path), "inbox", false).unwrap();
        fixture.push().await;
        assert!(server_dir.join("INBOX/1,S,$NotJunk.eml").is_file());
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[tokio::test]
    async fn a_push_sends_the_changes_bundled_then_mirrors_exactly_those_messages() {
        let fixture = Fixture::new("journal-push");
        let (dir, inbox) = (&fixture.dir, fixture.dev.join("private/INBOX"));
        fs::write(inbox.join("2.eml"), "Subject: two\r\n\r\nHi\r\n").unwrap();
        fixture.sync().await;
        let files = store::index(&dir.join("private/inbox")).unwrap();
        let path = |uid: u32| store::relative(&files[&uid], dir);
        for (uid, change) in [
            (1, Change::Flagged),
            (2, Change::Flagged),
            (1, Change::Unseen),
        ] {
            flag::run(dir, Path::new(&path(uid)), change).unwrap();
        }
        assert!(!store::read_flags(&files[&1]).unwrap().flagged);

        let pushed = fixture.push().await;
        assert_eq!((pushed.pushed, pushed.dropped.len()), (3, 0));
        assert!(inbox.join("1,F.eml").is_file() && inbox.join("2,F.eml").is_file());
        let flags = store::read_flags(&files[&1]).unwrap();
        assert!(flags.flagged && !flags.seen);
        assert!(changes(dir).is_empty());

        flag::run(dir, Path::new(&path(2)), Change::Seen).unwrap();
        fs::remove_file(inbox.join("2,F.eml")).unwrap();
        flag::run(dir, Path::new(&path(1)), Change::Seen).unwrap();
        fs::write(inbox.join(".uidvalidity"), "1").unwrap();
        let pushed = fixture.push().await;
        assert_eq!(pushed.pushed, 1);
        assert_eq!(pushed.dropped.len(), 1);
        assert_eq!(pushed.dropped[0].path, Some(path(2)));
        assert_eq!(pushed.dropped[0].reason, t().message_gone);
        assert!(inbox.join("1,SF.eml").is_file());

        flag::run(dir, Path::new(&path(1)), Change::Unflagged).unwrap();
        fs::write(inbox.join(".uidvalidity"), "2").unwrap();
        let pushed = fixture.push().await;
        assert_eq!(pushed.dropped[0].reason, t().folder_changed);
        assert!(changes(dir).is_empty());

        fs::write(inbox.join(".uidvalidity"), "1").unwrap();
        fixture.sync().await;
        let files = store::index(&dir.join("private/inbox")).unwrap();
        flag::run(
            dir,
            Path::new(&store::relative(&files[&1], dir)),
            Change::Seen,
        )
        .unwrap();
        fs::remove_file(inbox.join(".uidvalidity")).unwrap();
        let pushed = fixture.push().await;
        assert_eq!(pushed.dropped[0].reason, t().folder_changed);
        fs::write(inbox.join(".uidvalidity"), "1").unwrap();

        flag::run(
            dir,
            Path::new(&store::relative(&files[&1], dir)),
            Change::Unseen,
        )
        .unwrap();
        fixture.sync().await;
        assert!(inbox.join("1,F.eml").is_file());
        assert!(changes(dir).is_empty());
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[test]
    fn a_folder_of_a_year_is_named_below_its_parent_with_the_parent_s_delimiter() {
        let folder = |name: &str, delimiter: Option<&str>| Folder {
            name: name.to_string(),
            delimiter: delimiter.map(str::to_string),
            selectable: true,
            special: None,
        };
        let listed = [
            folder("INBOX", Some(".")),
            folder("INBOX.Archive", Some(".")),
            folder("Flat", None),
        ];
        assert_eq!(
            year_name(&listed, "inbox/archive/2026").as_deref(),
            Some("INBOX.Archive.2026")
        );
        assert_eq!(year_name(&listed, "inbox/archive/rechnungen"), None);
        assert_eq!(year_name(&listed, "flat/2026"), None);
        assert_eq!(year_name(&listed, "archive/2026"), None);
        assert_eq!(year_name(&listed, "inbox"), None);
    }
}
