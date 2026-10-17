use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::ValueEnum;
use serde::Serialize;

use crate::journal::{self, Overlay};
use crate::kind::Kind;
use crate::settings::Settings;
use crate::style::{Color, Overrides, Style};
use crate::{output, settings, store};

#[derive(Debug, Serialize)]
struct Row {
    kind: Kind,
    style: Style,
    set: Overrides,
    accounts: Vec<String>,
    messages: usize,
    unread: usize,
}

impl Row {
    fn new(kind: Kind, settings: &Settings) -> Self {
        Self {
            kind,
            style: settings.style(Some(kind)),
            set: settings.kind_styles.get(&kind).copied().unwrap_or_default(),
            accounts: Vec::new(),
            messages: 0,
            unread: 0,
        }
    }
}

pub fn run(dir: &Path, json: bool) -> Result<()> {
    let rows = rows(dir, &settings::load(dir)?)?;
    output::stdout(|out| write(out, &rows, json))
}

fn rows(dir: &Path, settings: &Settings) -> Result<Vec<Row>> {
    let overlay = journal::overlay(dir, settings)?;
    let mut rows: BTreeMap<Kind, Row> = Kind::value_variants()
        .iter()
        .map(|kind| (*kind, Row::new(*kind, settings)))
        .collect();
    for (id, account) in &settings.accounts {
        let account_dir = dir.join(id);
        for (folder_dir, _) in store::local_folders(&account_dir)? {
            let name = store::relative(&folder_dir, &account_dir);
            let Some(row) = account
                .synced_kind(&name)
                .and_then(|kind| rows.get_mut(&kind))
            else {
                continue;
            };
            let (messages, unread) = counts(dir, &folder_dir, &overlay)?;
            if !row.accounts.contains(id) {
                row.accounts.push(id.clone());
            }
            row.messages += messages;
            row.unread += unread;
        }
    }
    Ok(rows.into_values().collect())
}

fn counts(dir: &Path, folder_dir: &Path, overlay: &Overlay) -> Result<(usize, usize)> {
    let shown: Vec<(String, PathBuf)> = store::index(folder_dir)?
        .into_values()
        .map(|path| (store::relative(&path, dir), path))
        .filter(|(relative, _)| !overlay.hides(relative))
        .collect();
    let unread = shown
        .iter()
        .filter_map(|(relative, path)| {
            let mut flags = store::read_flags(path).ok()?;
            overlay.apply(relative, &mut flags);
            (!flags.seen).then_some(())
        })
        .count();
    Ok((shown.len(), unread))
}

fn write(out: &mut dyn Write, rows: &[Row], json: bool) -> io::Result<()> {
    for row in rows {
        if json {
            output::json_line(out, row)?;
        } else {
            writeln!(
                out,
                "{}{:<8} {:<9} {:<8} {:<30} {:>7} {:>5}  {}",
                if row.set.is_empty() { ' ' } else { '*' },
                row.kind.as_str(),
                row.style.icon.as_str(),
                row.style.color.map_or("", Color::as_str),
                row.style.flags(),
                row.messages,
                row.unread,
                row.accounts.join(", ")
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::settings::Account;
    use crate::testing::{assigned, write_message};

    #[test]
    fn every_kind_is_a_row_summed_over_the_synchronized_folders_of_all_accounts() {
        let dir = std::env::temp_dir().join(format!("lamemail-folders-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for path in [
            "private/inbox/2026-01-01_1000-a-1.json",
            "private/inbox/2026-01-01_1000-b-2.json",
            "private/gesendet/2026-01-01_1000-c-3.json",
            "private/spam/2026-01-01_1000-d-4.json",
            "business/inbox/2026-01-01_1000-e-5.json",
        ] {
            write_message(&dir, path, "paul@x", &["me@x"], None);
        }
        let account = |kinds: &[(&str, Kind)]| Account {
            folders: assigned(kinds),
            ..Account::new("imap.example.com".to_string(), 993, "me@x".to_string())
        };
        let mut private = account(&[
            ("inbox", Kind::Inbox),
            ("gesendet", Kind::Sent),
            ("spam", Kind::Junk),
        ]);
        private.folders.get_mut("spam").unwrap().sync = false;
        let mut junk = Overrides::default();
        junk.set(crate::style::Property::Dim, "false").unwrap();
        let settings = Settings {
            accounts: [
                ("private".to_string(), private),
                ("business".to_string(), account(&[("inbox", Kind::Inbox)])),
            ]
            .into(),
            kind_styles: [(Kind::Junk, junk)].into(),
            ..Settings::default()
        };
        let rows = rows(&dir, &settings).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        let row = |kind| rows.iter().find(|row| row.kind == kind).unwrap();
        let summary: Vec<(&str, Vec<&str>, usize)> = rows
            .iter()
            .map(|row| {
                let accounts = row.accounts.iter().map(String::as_str).collect();
                (row.kind.as_str(), accounts, row.messages)
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("inbox", vec!["business", "private"], 3),
                ("sent", vec!["private"], 1),
                ("drafts", vec![], 0),
                ("junk", vec![], 0),
                ("trash", vec![], 0),
                ("archive", vec![], 0),
            ]
        );
        let junk = row(Kind::Junk);
        assert!(!junk.style.dim && junk.style.dense && !junk.set.is_empty());
        let sent = row(Kind::Sent);
        assert!(sent.style.color == Some(Color::Green) && sent.set.is_empty());
    }
}
