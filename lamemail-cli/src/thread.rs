use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{self, Write};
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::i18n::{fill, t};
use crate::kind;
use crate::list::{self, Located};
use crate::message::{Addr, MailFile};
use crate::settings::Settings;
use crate::show::{self, Message};
use crate::{output, settings};

const SEPARATOR: &str = "────────────────────────────────────────────────────────────";

#[derive(Serialize)]
pub struct Entry {
    #[serde(flatten)]
    pub message: Message,
    pub counterparts: Vec<Addr>,
    pub me: Vec<String>,
    pub parent: Option<String>,
}

struct Mail {
    located: Located,
    file: MailFile,
    counterparts: BTreeMap<String, Addr>,
}

pub fn run(dir: &Path, path: &Path, json: bool) -> Result<()> {
    let settings = settings::load(dir)?;
    let entries = entries(dir, &settings, path)?;
    output::stdout(|out| write(out, &entries, json))
}

pub fn entries(dir: &Path, settings: &Settings, path: &Path) -> Result<Vec<Entry>> {
    let anchor = list::locate_one(dir, settings, path)?;
    let (mails, mine) = read_account(dir, settings, &anchor.account)?;
    history(mails, &mine, &anchor.path)
        .with_context(|| fill(t().show_not_message, &[&path.display(), &dir.display()]))
}

fn read_account(
    dir: &Path,
    settings: &Settings,
    id: &str,
) -> Result<(Vec<Mail>, BTreeSet<String>)> {
    let (read, mine) = show::read_account(dir, settings, id)?;
    let mut mails: Vec<Mail> = read
        .into_iter()
        .map(|(located, file)| Mail {
            counterparts: counterparts(&file, &mine),
            located,
            file,
        })
        .collect();
    chronological(&mut mails);
    Ok((mails, mine))
}

fn history(mails: Vec<Mail>, mine: &BTreeSet<String>, anchor: &str) -> Option<Vec<Entry>> {
    let wanted: BTreeSet<String> = mails
        .iter()
        .find(|mail| mail.located.path == anchor)?
        .counterparts
        .keys()
        .cloned()
        .collect();
    let mut mails = list::one_copy(mails, |mail| {
        let id = mail.file.message_id.clone()?;
        Some((
            id,
            (
                mail.located.path != anchor,
                kind::rank(false, mail.located.kind),
            ),
        ))
    });
    mails.retain(|mail| includes(&mail.counterparts, &wanted));
    let paths: Vec<String> = mails.iter().map(|mail| mail.located.path.clone()).collect();
    let parents = parents(&mails);
    let entries = mails
        .into_iter()
        .zip(parents)
        .map(|(mail, parent)| Entry {
            me: involved(&mail.file, mine),
            message: show::message(mail.file, mail.located, mine),
            counterparts: mail.counterparts.into_values().collect(),
            parent: parent.map(|parent| paths[parent].clone()),
        })
        .collect();
    Some(entries)
}

fn counterparts(file: &MailFile, mine: &BTreeSet<String>) -> BTreeMap<String, Addr> {
    file.from
        .iter()
        .chain(&file.to)
        .chain(&file.cc)
        .filter_map(|addr| {
            addr.address
                .as_deref()
                .map(|address| (address.to_lowercase(), addr.clone()))
        })
        .filter(|(address, _)| !mine.contains(address))
        .collect()
}

fn involved(file: &MailFile, mine: &BTreeSet<String>) -> Vec<String> {
    let addresses: BTreeSet<String> = [&file.from, &file.to, &file.cc]
        .into_iter()
        .flat_map(|addrs| show::addresses(addrs))
        .filter(|address| mine.contains(address))
        .collect();
    addresses.into_iter().collect()
}

fn includes(set: &BTreeMap<String, Addr>, wanted: &BTreeSet<String>) -> bool {
    if wanted.is_empty() {
        set.is_empty()
    } else {
        wanted.iter().all(|address| set.contains_key(address))
    }
}

fn chronological(mails: &mut [Mail]) {
    mails.sort_by(|a, b| {
        a.located
            .time
            .cmp(&b.located.time)
            .then(a.located.uid.cmp(&b.located.uid))
    });
}

fn parents(mails: &[Mail]) -> Vec<Option<usize>> {
    let ids: HashMap<&str, usize> = mails
        .iter()
        .enumerate()
        .filter_map(|(index, mail)| mail.file.message_id.as_deref().map(|id| (id, index)))
        .collect();
    mails
        .iter()
        .enumerate()
        .map(|(index, mail)| {
            mail.file
                .in_reply_to
                .iter()
                .chain(mail.file.references.iter().rev())
                .filter_map(|id| ids.get(id.as_str()).copied())
                .find(|&parent| parent < index)
        })
        .collect()
}

fn write(out: &mut dyn Write, entries: &[Entry], json: bool) -> io::Result<()> {
    for (index, entry) in entries.iter().enumerate() {
        if json {
            output::json_line(out, entry)?;
        } else {
            if index > 0 {
                writeln!(out, "\n{SEPARATOR}\n")?;
            }
            show::write_text(out, &entry.message)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::Kind;
    use crate::settings::Account;
    use crate::testing::{assigned, write_message};
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn settings() -> Settings {
        let mut accounts = BTreeMap::new();
        let mut account = Account::new(
            "imap.example.com".to_string(),
            993,
            "private@example.com".to_string(),
        );
        account.folders = assigned(&[
            ("inbox", Kind::Inbox),
            ("sent", Kind::Sent),
            ("archive", Kind::Archive),
        ]);
        accounts.insert("private".to_string(), account);
        Settings {
            accounts,
            ..Settings::default()
        }
    }

    fn sample_account() -> (Vec<Mail>, BTreeSet<String>) {
        static RUNS: AtomicUsize = AtomicUsize::new(0);
        let run = RUNS.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("lamemail-thread-{}-{run}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let me = "Private@example.com";
        write_message(
            &dir,
            "private/inbox/2026-01-01_1000-a-1.json",
            "paul@x",
            &[me],
            None,
        );
        write_message(
            &dir,
            "private/sent/2026-01-01_1100-b-2.json",
            "alias@x",
            &["maria@x", "paul@x"],
            None,
        );
        write_message(
            &dir,
            "private/inbox/2026-01-01_1200-c-3.json",
            "paul@x",
            &[me],
            Some("m2@x"),
        );
        write_message(
            &dir,
            "private/inbox/2026-01-01_1300-d-4.json",
            "maria@x",
            &[me],
            None,
        );
        write_message(
            &dir,
            "private/inbox/2026-01-01_1400-e-5.json",
            "paul@x",
            &[me],
            Some("m1@x"),
        );
        write_message(
            &dir,
            "private/inbox/2026-01-01_1500-f-6.json",
            "alias@x",
            &["paul@x"],
            Some("m9@x"),
        );
        let read = read_account(&dir, &settings(), "private").unwrap();
        fs::remove_dir_all(&dir).unwrap();
        read
    }

    #[test]
    fn own_addresses_come_from_the_login_and_the_sent_folder() {
        let (_, mine) = sample_account();
        assert_eq!(
            mine,
            ["alias@x", "private@example.com"].map(String::from).into()
        );
    }

    #[test]
    fn history_holds_every_mail_with_all_people_of_the_anchor() {
        let (mails, mine) = sample_account();
        let entries = history(mails, &mine, "private/sent/2026-01-01_1100-b-2.json").unwrap();
        let uids: Vec<u32> = entries.iter().map(|entry| entry.message.uid).collect();
        assert_eq!(uids, vec![2]);
        let group: Vec<&str> = entries[0]
            .counterparts
            .iter()
            .filter_map(|addr| addr.address.as_deref())
            .collect();
        assert_eq!(group, vec!["maria@x", "paul@x"]);
        assert!(history(Vec::new(), &mine, "private/inbox/2026-01-01_1000-a-1.json").is_none());
    }

    #[test]
    fn replies_point_at_earlier_mails_of_the_history() {
        let (mails, mine) = sample_account();
        let entries = history(mails, &mine, "private/inbox/2026-01-01_1000-a-1.json").unwrap();
        let rows: Vec<(u32, Option<&str>, bool)> = entries
            .iter()
            .map(|entry| {
                (
                    entry.message.uid,
                    entry.parent.as_deref(),
                    entry.message.own,
                )
            })
            .collect();
        assert_eq!(entries[0].me, vec!["private@example.com"]);
        assert_eq!(entries[1].me, vec!["alias@x"]);
        assert_eq!(
            rows,
            vec![
                (1, None, false),
                (2, None, true),
                (3, Some("private/sent/2026-01-01_1100-b-2.json"), false),
                (5, Some("private/inbox/2026-01-01_1000-a-1.json"), false),
                (6, None, true),
            ]
        );
    }

    #[test]
    fn copies_appear_once_the_selected_one_or_the_one_of_the_first_kind() {
        let dir =
            std::env::temp_dir().join(format!("lamemail-thread-copies-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let paths = [
            "private/archive/2026-01-01_1000-a-1.json",
            "private/inbox/2026-01-01_1000-a-1.json",
        ];
        let later = "private/archive/2026-01-02_1000-b-2.json";
        for path in paths.into_iter().chain([later]) {
            write_message(&dir, path, "paul@x", &["private@example.com"], None);
        }
        let shown = |anchor: &str| {
            let (mails, mine) = read_account(&dir, &settings(), "private").unwrap();
            history(mails, &mine, anchor)
                .unwrap()
                .into_iter()
                .map(|entry| entry.message.path)
                .collect::<Vec<_>>()
        };
        for anchor in paths {
            assert_eq!(shown(anchor), [anchor, later]);
        }
        assert_eq!(shown(later), [paths[1], later]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_mail_to_oneself_matches_only_such_mails() {
        let mut set = BTreeMap::new();
        assert!(includes(&set, &BTreeSet::new()));
        set.insert(
            "paul@x".to_string(),
            Addr {
                name: None,
                address: Some("paul@x".to_string()),
            },
        );
        assert!(!includes(&set, &BTreeSet::new()));
        assert!(includes(&set, &["paul@x".to_string()].into()));
        assert!(!includes(&set, &["maria@x".to_string()].into()));
    }
}
