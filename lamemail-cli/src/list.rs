use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::i18n::{fill, t};
use crate::kind::{self, Kind};
use crate::message::{self, Addr, Flags, MailFile};
use crate::settings::Settings;
use crate::style::{Color, Style};
use crate::{journal, output, settings, store};

const PREVIEW_CHARS: usize = 160;
const FILE_TIME_CHARS: usize = 15;
const ACCOUNT_WIDTH: usize = 10;
const FROM_WIDTH: usize = 24;

#[derive(Debug, Serialize)]
pub struct Entry {
    pub account: String,
    pub color: Color,
    pub folder: String,
    pub style: Style,
    pub kind: Option<Kind>,
    pub uid: u32,
    pub path: String,
    pub received: String,
    pub date: Option<String>,
    pub message_id: Option<String>,
    pub from: Vec<Addr>,
    pub to: Vec<Addr>,
    pub cc: Vec<Addr>,
    pub subject: String,
    pub flags: Flags,
    pub forwarded: bool,
    pub pinned: bool,
    pub size: u64,
    pub attachments: usize,
    pub preview: String,
}

pub struct Located {
    pub account: String,
    pub color: Color,
    pub folder: String,
    pub style: Style,
    pub kind: Option<Kind>,
    pub file: PathBuf,
    pub path: String,
    pub time: String,
    pub uid: u32,
}

pub fn run(
    dir: &Path,
    json: bool,
    search: &str,
    kinds: &[Kind],
    flagged_first: bool,
) -> Result<()> {
    let entries = entries(dir, search, kinds)?;
    output::stdout(|out| stream(out, entries.into_iter(), json, flagged_first))
}

fn entries(dir: &Path, search: &str, kinds: &[Kind]) -> Result<Vec<Entry>> {
    let settings = settings::load(dir)?;
    let located = locate_all(dir, &settings)?;
    let overlay = journal::overlay(dir)?;
    let words = search_words(search);
    let found = located
        .into_iter()
        .filter_map(|mut item| {
            if overlay.hides(&item.path) {
                return None;
            }
            let mut file: MailFile = store::read_or_skip(&item.file)?;
            overlay.apply(&item.path, &mut file.flags);
            let moved = overlay.folder_of(&item.path).is_some();
            overlay.relocate(&mut item, &settings);
            matches(&file, &words).then(|| (entry(file, &item), moved))
        })
        .collect();
    let entries = one_copy(found, |(entry, moved): &(Entry, bool)| {
        let id = entry.message_id.clone()?;
        Some(((entry.account.clone(), id), kind::rank(*moved, entry.kind)))
    });
    Ok(entries
        .into_iter()
        .map(|(entry, _)| entry)
        .filter(|entry| kinds.is_empty() || entry.kind.is_some_and(|kind| kinds.contains(&kind)))
        .collect())
}

pub fn one_copy<T, K: Eq + Hash, R: Ord>(
    items: Vec<T>,
    copy: impl Fn(&T) -> Option<(K, R)>,
) -> Vec<T> {
    let copies: Vec<Option<(K, R)>> = items.iter().map(copy).collect();
    let mut best: HashMap<&K, usize> = HashMap::new();
    for (index, copy) in copies.iter().enumerate() {
        let Some((key, rank)) = copy else {
            continue;
        };
        let kept = best.entry(key).or_insert(index);
        if copies[*kept]
            .as_ref()
            .is_some_and(|(_, other)| rank < other)
        {
            *kept = index;
        }
    }
    let kept: HashSet<usize> = best.into_values().collect();
    items
        .into_iter()
        .zip(copies)
        .enumerate()
        .filter(|(index, (_, copy))| copy.is_none() || kept.contains(index))
        .map(|(_, (item, _))| item)
        .collect()
}

pub fn locate_all(dir: &Path, settings: &Settings) -> Result<Vec<Located>> {
    let mut located = Vec::new();
    for id in settings.accounts.keys() {
        located.extend(locate_account(dir, settings, id)?);
    }
    located.sort_by(|a, b| b.time.cmp(&a.time).then(b.uid.cmp(&a.uid)));
    Ok(located)
}

pub fn locate_account(dir: &Path, settings: &Settings, id: &str) -> Result<Vec<Located>> {
    let Some(account) = settings.accounts.get(id) else {
        return Ok(Vec::new());
    };
    let account_dir = dir.join(id);
    let color = settings.color(id);
    let mut located = Vec::new();
    for (folder_dir, _) in store::local_folders(&account_dir)? {
        let folder = store::relative(&folder_dir, &account_dir);
        let Some(kind) = account.synced_kind(&folder) else {
            continue;
        };
        let style = settings.style(Some(kind));
        for (uid, file) in store::index(&folder_dir)? {
            located.push(Located {
                account: id.to_string(),
                color,
                folder: folder.clone(),
                style,
                kind: Some(kind),
                path: store::relative(&file, dir),
                time: time_prefix(&file),
                file,
                uid,
            });
        }
    }
    Ok(located)
}

pub fn locate_one(dir: &Path, settings: &Settings, path: &Path) -> Result<Located> {
    let base = std::path::absolute(dir).with_context(|| format!("resolving {}", dir.display()))?;
    let file = base.join(path);
    let parts = parts_below(&base, &file).unwrap_or_default();
    match parts.as_slice() {
        [account, folder @ .., name]
            if !folder.is_empty()
                && let Some(uid) = store::uid_from_name(name) =>
        {
            let folder = folder.join("/");
            let kind = settings.kind_of(account, &folder);
            Ok(Located {
                account: account.to_string(),
                color: settings.color(account),
                style: settings.style(kind),
                kind,
                folder,
                path: parts.join("/"),
                time: time_prefix(&file),
                file,
                uid,
            })
        }
        _ => bail!(
            "{}",
            fill(t().show_not_message, &[&path.display(), &base.display()])
        ),
    }
}

fn parts_below<'a>(base: &Path, file: &'a Path) -> Option<Vec<&'a str>> {
    file.strip_prefix(base)
        .ok()?
        .components()
        .map(|part| match part {
            Component::Normal(name) => name.to_str(),
            _ => None,
        })
        .collect()
}

fn stream(
    out: &mut dyn Write,
    entries: impl Iterator<Item = Entry>,
    json: bool,
    flagged_first: bool,
) -> io::Result<()> {
    let mut rest = Vec::new();
    for mut entry in entries {
        entry.pinned = flagged_first && entry.flags.flagged;
        if flagged_first && !entry.pinned {
            rest.push(entry);
        } else {
            write(out, &entry, json)?;
        }
    }
    for entry in rest {
        write(out, &entry, json)?;
    }
    Ok(())
}

fn write(out: &mut dyn Write, entry: &Entry, json: bool) -> io::Result<()> {
    if json {
        output::json_line(out, entry)
    } else {
        writeln!(out, "{}", line(entry))
    }
}

fn search_words(search: &str) -> Vec<String> {
    search.split_whitespace().map(str::to_lowercase).collect()
}

pub fn matches(file: &MailFile, words: &[String]) -> bool {
    if words.is_empty() {
        return true;
    }
    let people = [&file.from, &file.to, &file.cc, &file.bcc]
        .into_iter()
        .flatten()
        .flat_map(|addr| [&addr.name, &addr.address])
        .flatten();
    let text = people
        .chain([&file.subject, &file.body])
        .map(|part| part.to_lowercase())
        .collect::<Vec<_>>()
        .join("\n");
    words.iter().all(|word| text.contains(word.as_str()))
}

fn entry(file: MailFile, located: &Located) -> Entry {
    Entry {
        account: located.account.clone(),
        color: located.color,
        folder: located.folder.clone(),
        style: located.style,
        kind: located.kind,
        uid: file.uid,
        path: located.path.clone(),
        received: file.received,
        date: file.date,
        message_id: file.message_id,
        from: file.from,
        to: file.to,
        cc: file.cc,
        subject: file.subject,
        forwarded: file.flags.has(message::FORWARDED),
        flags: file.flags,
        pinned: false,
        size: file.size,
        attachments: file.attachments.len(),
        preview: preview(&file.body),
    }
}

fn preview(body: &str) -> String {
    let mut out = String::new();
    for word in body.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
        if out.chars().count() >= PREVIEW_CHARS {
            break;
        }
    }
    out.chars().take(PREVIEW_CHARS).collect()
}

fn time_prefix(path: &Path) -> String {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.chars().take(FILE_TIME_CHARS).collect()
}

fn line(entry: &Entry) -> String {
    let mark = if entry.flags.seen { ' ' } else { '*' };
    let when = message::minute(&entry.received);
    let from = entry
        .from
        .first()
        .and_then(|a| a.name.clone().or_else(|| a.address.clone()))
        .unwrap_or_default();
    let from: String = from.chars().take(FROM_WIDTH).collect();
    format!(
        "{mark} {when}  {:<ACCOUNT_WIDTH$}  {from:<FROM_WIDTH$}  {}",
        entry.account, entry.subject
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use super::*;
    use crate::kind::Assigned;
    use crate::message::{Meta, convert};
    use crate::settings::Account;
    use crate::style;
    use crate::testing::{assigned, write_message};

    fn sample() -> MailFile {
        convert(
            include_bytes!("../tests/fixtures/multipart-attachment.eml"),
            Meta {
                account: "private",
                folder: "INBOX",
                uid: 4711,
                flags: Flags::default(),
                received: 1_757_939_525,
                size: 1234,
            },
        )
    }

    fn located(path: &str) -> Located {
        locate_one(Path::new("/sync"), &Settings::default(), Path::new(path)).unwrap()
    }

    fn private_with(folders: BTreeMap<String, Assigned>) -> Settings {
        let mut account = Account::new("imap.example.com".to_string(), 993, "me@x".to_string());
        account.folders = folders;
        Settings {
            accounts: [("private".to_string(), account)].into(),
            ..Settings::default()
        }
    }

    #[test]
    fn only_folders_with_a_kind_and_sync_are_found() {
        let dir = std::env::temp_dir().join(format!("lamemail-list-found-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for path in [
            "private/inbox/2026-01-01_1000-a-1.json",
            "private/spam/2026-01-01_1000-b-2.json",
            "private/notes/2026-01-01_1000-c-3.json",
            "private/new/2026-01-01_1000-d-4.json",
        ] {
            write_message(&dir, path, "paul@x", &["me@x"], None);
        }
        let mut folders = assigned(&[("inbox", Kind::Inbox), ("spam", Kind::Junk)]);
        folders.get_mut("spam").unwrap().sync = false;
        folders.insert(
            "notes".to_string(),
            Assigned {
                kind: None,
                sync: false,
                default: false,
            },
        );
        let settings = private_with(folders);
        let found: Vec<(String, Option<Kind>, style::Icon)> =
            locate_account(&dir, &settings, "private")
                .unwrap()
                .into_iter()
                .map(|located| (located.folder, located.kind, located.style.icon))
                .collect();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            found,
            [(
                "inbox".to_string(),
                Some(Kind::Inbox),
                style::Icon::Incoming
            )]
        );
    }

    #[test]
    fn kinds_keep_the_mail_of_their_folders_after_copies_are_one() {
        let dir = std::env::temp_dir().join(format!("lamemail-list-kinds-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for path in [
            "private/inbox/2026-01-01_1000-a-1.json",
            "private/all/2026-01-01_1000-a-1.json",
            "private/all/2026-01-01_0900-b-2.json",
            "private/sent/2026-01-01_0800-c-3.json",
        ] {
            write_message(&dir, path, "paul@x", &["me@x"], None);
        }
        let settings = private_with(assigned(&[
            ("inbox", Kind::Inbox),
            ("all", Kind::Archive),
            ("sent", Kind::Sent),
        ]));
        settings::save(&dir, &settings).unwrap();
        let listed = |kinds: &[Kind]| -> Vec<(u32, String)> {
            entries(&dir, "", kinds)
                .unwrap()
                .into_iter()
                .map(|entry| (entry.uid, entry.folder))
                .collect()
        };
        let every = listed(&[]);
        let archived = listed(&[Kind::Archive]);
        let received_and_sent = listed(&[Kind::Inbox, Kind::Sent]);
        fs::remove_dir_all(&dir).unwrap();
        let line = |uid, folder: &str| (uid, folder.to_string());
        assert_eq!(every, [line(1, "inbox"), line(2, "all"), line(3, "sent")]);
        assert_eq!(archived, [line(2, "all")]);
        assert_eq!(received_and_sent, [line(1, "inbox"), line(3, "sent")]);
    }

    #[test]
    fn locates_account_folder_and_file() {
        let item = located("business/inbox/archive/2025/2026-01-01_1000-x-7.json");
        assert_eq!(item.account, "business");
        assert_eq!(item.folder, "inbox/archive/2025");
        assert_eq!(
            item.path,
            "business/inbox/archive/2025/2026-01-01_1000-x-7.json"
        );
        assert_eq!(
            item.file,
            PathBuf::from("/sync/business/inbox/archive/2025/2026-01-01_1000-x-7.json")
        );
        assert_eq!(item.time, "2026-01-01_1000");
        assert_eq!(item.uid, 7);
        assert_eq!(item.kind, None);
        assert_eq!(item.style, style::default_for(None));
        assert_eq!(
            located("/sync/private/inbox/x-1.json").path,
            "private/inbox/x-1.json"
        );
        assert_eq!(
            located("./private/inbox/x-1.json").path,
            "private/inbox/x-1.json"
        );
    }

    #[test]
    fn rejects_paths_that_are_not_messages() {
        for path in [
            "/elsewhere/private/inbox/x-1.json",
            "private/../private/inbox/x-1.json",
            "private/inbox",
            "private/x-1.json",
            "private/inbox/.folder.json",
            "private/inbox/notes.txt",
        ] {
            assert!(
                locate_one(Path::new("/sync"), &Settings::default(), Path::new(path)).is_err(),
                "{path}"
            );
        }
    }

    #[test]
    fn preview_collapses_whitespace_and_cuts_at_chars() {
        assert_eq!(
            preview("Hi,\n\nplease  find\tthe offer.\n"),
            "Hi, please find the offer."
        );
        assert_eq!(preview(""), "");
        let long = preview(&"wörd ".repeat(100));
        assert_eq!(long.chars().count(), PREVIEW_CHARS);
    }

    #[test]
    fn entry_keeps_the_documented_fields() {
        let mut item = located("private/inbox/x-4711.json");
        item.kind = Some(Kind::Inbox);
        item.style = style::default_for(item.kind);
        let entry = entry(sample(), &item);
        let json = serde_json::to_value(&entry).unwrap();
        for key in [
            "account",
            "color",
            "folder",
            "style",
            "kind",
            "uid",
            "path",
            "received",
            "date",
            "message_id",
            "from",
            "to",
            "cc",
            "subject",
            "flags",
            "pinned",
            "size",
            "attachments",
            "preview",
        ] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
        assert_eq!(json["color"], "blue");
        assert_eq!(json["attachments"], 1);
        assert_eq!(json["preview"], "Text version.");
        assert_eq!(json["flags"]["seen"], false);
        assert_eq!(json["folder"], "inbox");
        assert_eq!(json["style"]["icon"], "incoming");
        assert_eq!(json["style"]["dense"], false);
        assert_eq!(json["kind"], "inbox");
        assert_eq!(json["message_id"], "attach@example.com");
    }

    #[test]
    fn search_needs_every_word_in_people_subject_or_text() {
        let file = sample();
        for search in [
            "",
            "  ",
            "angebot",
            "FÜR projekt",
            "jane@example",
            "doe",
            "me@",
            "text version",
        ] {
            assert!(matches(&file, &search_words(search)), "{search}");
        }
        for search in ["angebot offer", "inbox", "version.pdf"] {
            assert!(!matches(&file, &search_words(search)), "{search}");
        }
    }

    #[test]
    fn flagged_first_puts_flagged_mail_ahead_and_pins_it() {
        let listed = |flagged_first| {
            let entries = [false, true, false, true]
                .into_iter()
                .zip(1..)
                .map(|(flagged, uid)| {
                    let mut entry =
                        entry(sample(), &located(&format!("private/inbox/x-{uid}.json")));
                    entry.uid = uid;
                    entry.flags.flagged = flagged;
                    entry
                });
            let mut out = Vec::new();
            stream(&mut out, entries, true, flagged_first).unwrap();
            String::from_utf8(out)
                .unwrap()
                .lines()
                .map(|line| {
                    let json: serde_json::Value = serde_json::from_str(line).unwrap();
                    format!("{} {}", json["uid"], json["pinned"])
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            listed(true),
            ["2 true", "4 true", "1 false", "3 false"].map(String::from)
        );
        assert_eq!(
            listed(false),
            ["1 false", "2 false", "3 false", "4 false"].map(String::from)
        );
    }

    #[test]
    fn human_line_marks_unread_and_shortens() {
        let line = line(&entry(sample(), &located("private/inbox/x-4711.json")));
        assert!(line.starts_with("* 2025-09-15 "), "{line}");
        assert!(line.contains("private"), "{line}");
        assert!(line.ends_with("Angebot für Projekt"), "{line}");
    }

    #[test]
    fn one_copy_of_a_message_stays_the_best_ranked_and_first() {
        let items = [
            ("a", Some("x"), 3),
            ("b", Some("x"), 1),
            ("c", None, 9),
            ("d", Some("x"), 1),
            ("e", Some("y"), 5),
        ];
        let kept = one_copy(items.to_vec(), |(_, id, rank)| id.map(|id| (id, *rank)));
        let names: Vec<&str> = kept.iter().map(|(name, _, _)| *name).collect();
        assert_eq!(names, ["b", "c", "e"]);
    }
}
