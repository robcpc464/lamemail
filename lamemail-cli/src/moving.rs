use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::flag::Change;
use crate::i18n::{fill, t};
use crate::journal::{self, Action};
use crate::kind::{self, Assigned, Kind, Kinds};
use crate::list::{self, Located};
use crate::settings::{self, Account, Settings};
use crate::style::Style;
use crate::{folder_name, output, store};

const TARGETS: [Kind; 4] = [Kind::Inbox, Kind::Archive, Kind::Trash, Kind::Junk];
const YEAR_FOLDER: Assigned = Assigned {
    kind: Some(Kind::Archive),
    sync: true,
    default: false,
};

#[derive(Debug, Serialize)]
struct Moved {
    path: String,
    from: String,
    folder: String,
    style: Style,
    kind: Option<Kind>,
}

struct Placed {
    located: Located,
    uidvalidity: u32,
    shown: String,
}

impl Placed {
    fn find(dir: &Path, settings: &Settings, path: &Path) -> Result<Placed> {
        let located = list::locate_one(dir, settings, path)?;
        store::read_flags(&located.file)?;
        let state = store::state_of(&located.file)?;
        let overlay = journal::overlay(dir)?;
        if overlay.hides(&located.path) {
            bail!(
                "{}",
                fill(t().show_not_message, &[&path.display(), &dir.display()])
            );
        }
        Ok(Placed {
            shown: overlay
                .folder_of(&located.path)
                .map_or_else(|| located.folder.clone(), str::to_string),
            uidvalidity: state.uidvalidity,
            located,
        })
    }
}

pub fn run(dir: &Path, path: &Path, target: &str, json: bool) -> Result<()> {
    let mut settings = settings::load(dir)?;
    let Placed {
        located,
        uidvalidity,
        shown,
    } = Placed::find(dir, &settings, path)?;
    let id = &located.account;
    let folder = match year_folder(settings.account(id)?, target, &located.time) {
        Some(folder) => {
            assign_year_folder(dir, &mut settings, id, &folder)?;
            folder
        }
        None => resolve(settings.account(id)?, target).with_context(|| missing(id, target))?,
    };
    let account = settings.account(id)?;
    let junk = |folder: &str| account.kind_of(folder) == Some(Kind::Junk);
    let verdict = match (junk(&folder), junk(&located.folder)) {
        (true, false) => Some(Change::Junk),
        (false, true) => Some(Change::NotJunk),
        _ => None,
    };
    let wish = (folder != located.folder).then(|| folder.clone());
    journal::note_move(dir, &located.path, uidvalidity, verdict, wish)?;
    let kind = account.kind_of(&folder);
    let moved = Moved {
        style: settings.style(kind),
        kind,
        path: located.path,
        from: shown,
        folder,
    };
    output::stdout(|out| {
        if json {
            output::json_line(out, &moved)
        } else {
            writeln!(
                out,
                "{}",
                fill(t().move_done, &[&moved.path, &moved.folder])
            )
        }
    })
}

pub fn delete(dir: &Path, path: &Path) -> Result<()> {
    let settings = settings::load(dir)?;
    let placed = Placed::find(dir, &settings, path)?;
    let path = placed.located.path;
    if settings.kind_of(&placed.located.account, &placed.shown) != Some(Kind::Trash) {
        bail!("{}", fill(t().delete_not_trash, &[&path]));
    }
    journal::record(dir, &path, placed.uidvalidity, Action::Discard)?;
    println!("{}", fill(t().delete_done, &[&path]));
    Ok(())
}

fn missing(account: &str, target: &str) -> String {
    match target_kind(target) {
        Some(kind) => kind::no_folder(account, kind),
        None => fill(t().move_no_folder, &[&account, &target]),
    }
}

fn target_kind(target: &str) -> Option<Kind> {
    TARGETS.into_iter().find(|kind| kind.as_str() == target)
}

fn year_folder(account: &Account, target: &str, time: &str) -> Option<String> {
    if target_kind(target) != Some(Kind::Archive) || !account.archives_by_year() {
        return None;
    }
    let year = time.get(..4).filter(|year| folder_name::is_year(year))?;
    let kinds = Kinds::of(account);
    let archive = kinds.folder(Kind::Archive)?;
    let base = match archive.rsplit_once('/') {
        Some((parent, last)) if folder_name::is_year(last) => parent,
        _ => archive,
    };
    let folder = format!("{base}/{year}");
    let free = match account.folders.get(&folder) {
        None => true,
        Some(assigned) => {
            assigned.kind.is_none() || (assigned.kind == Some(Kind::Archive) && assigned.sync)
        }
    };
    free.then_some(folder)
}

fn assign_year_folder(dir: &Path, settings: &mut Settings, id: &str, folder: &str) -> Result<()> {
    let folders = &mut settings.account_mut(id)?.folders;
    if folders
        .get(folder)
        .is_some_and(|assigned| assigned.kind == Some(Kind::Archive))
    {
        return Ok(());
    }
    folders.insert(folder.to_string(), YEAR_FOLDER);
    settings::save(dir, settings)
}

fn resolve(account: &Account, target: &str) -> Option<String> {
    let folder = match target_kind(target) {
        Some(kind) => Kinds::of(account).folder(kind)?.to_string(),
        None => folder_name::normalize(target),
    };
    account.synced_kind(&folder).is_some().then_some(folder)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::show;
    use crate::testing::{Fixture, assigned};

    #[test]
    fn a_target_is_the_default_folder_of_a_kind_or_a_folder_of_the_account() {
        let mut account = Account::new("imap.example.com".to_string(), 993, "me".to_string());
        account.folders = assigned(&[
            ("inbox", Kind::Inbox),
            ("papierkorb", Kind::Trash),
            ("[gmail]/all mail", Kind::Archive),
            ("bin", Kind::Trash),
        ]);
        account.folders.get_mut("papierkorb").unwrap().default = true;
        account.folders.get_mut("bin").unwrap().sync = false;
        let find = |target: &str| resolve(&account, target);

        assert_eq!(find("inbox").as_deref(), Some("inbox"));
        assert_eq!(find("trash").as_deref(), Some("papierkorb"));
        assert_eq!(find("archive").as_deref(), Some("[gmail]/all mail"));
        assert_eq!(find("junk"), None);
        assert_eq!(find("/Papierkorb").as_deref(), Some("papierkorb"));
        assert_eq!(find("bin"), None);
        assert_eq!(find("nowhere"), None);
        assert!(missing("private", "junk").contains("lamemail account folder"));
    }

    #[test]
    fn archived_mail_goes_into_the_folder_of_the_year_it_arrived_unless_switched_off() {
        let mut account = Account::new("imap.example.com".to_string(), 993, "me".to_string());
        account.folders = assigned(&[("inbox", Kind::Inbox), ("archive", Kind::Archive)]);
        let year =
            |account: &Account, target: &str| year_folder(account, target, "2025-12-31_2359");

        assert_eq!(year(&account, "archive").as_deref(), Some("archive/2025"));
        assert_eq!(year(&account, "inbox"), None);
        assert_eq!(year(&account, "archive/2024"), None);
        assert_eq!(year_folder(&account, "archive", "x"), None);
        account.folders.insert(
            "archive/2025".to_string(),
            Assigned {
                kind: None,
                sync: false,
                default: false,
            },
        );
        assert_eq!(year(&account, "archive").as_deref(), Some("archive/2025"));
        account.folders.get_mut("archive/2025").unwrap().kind = Some(Kind::Archive);
        assert_eq!(year(&account, "archive"), None);
        account.folders.get_mut("archive/2025").unwrap().sync = true;
        assert_eq!(year(&account, "archive").as_deref(), Some("archive/2025"));
        let yearly_default = Account {
            folders: assigned(&[("archives/2024", Kind::Archive)]),
            ..account.clone()
        };
        assert_eq!(
            year(&yearly_default, "archive").as_deref(),
            Some("archives/2025")
        );
        account.archive_by_year = Some(false);
        assert_eq!(year(&account, "archive"), None);
        let gmail = Account {
            folders: account.folders.clone(),
            ..Account::new("imap.gmail.com".to_string(), 993, "me".to_string())
        };
        assert_eq!(year(&gmail, "archive"), None);
        let without = Account::new("imap.example.com".to_string(), 993, "me".to_string());
        assert_eq!(year(&without, "archive"), None);
    }

    #[tokio::test]
    async fn the_push_creates_the_folder_of_the_year_and_the_sync_keeps_it() {
        let fixture = Fixture::new("moving-year");
        let dir = &fixture.dir;
        fixture.marked_folder("Archive", "archive");
        fixture.sync().await;
        let inbox = store::index(&dir.join("private/inbox")).unwrap();
        let path = store::relative(&inbox[&1], dir);
        let year = &list::locate_one(dir, &fixture.settings(), Path::new(&path))
            .unwrap()
            .time[..4];
        let folder = format!("archive/{year}");

        run(dir, Path::new(&path), "archive", false).unwrap();
        let settings = fixture.settings();
        assert_eq!(
            settings.account("private").unwrap().synced_kind(&folder),
            Some(Kind::Archive)
        );
        let (moved, _) = show::open(dir, &settings, Path::new(&path)).unwrap();
        assert_eq!(
            (moved.folder.as_str(), moved.kind),
            (folder.as_str(), Some(Kind::Archive))
        );

        let pushed = fixture.push().await;
        assert_eq!((pushed.pushed, pushed.dropped.len()), (1, 0));
        assert!(
            fixture
                .dev
                .join(format!("private/Archive/{year}/1,S.eml"))
                .exists()
        );
        assert_eq!(
            store::index(&dir.join("private").join(&folder))
                .unwrap()
                .len(),
            1
        );
        fixture.sync().await;
        let settings = fixture.settings();
        assert_eq!(
            settings.account("private").unwrap().synced_kind(&folder),
            Some(Kind::Archive)
        );
        assert_eq!(
            store::index(&dir.join("private").join(&folder))
                .unwrap()
                .len(),
            1
        );
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[tokio::test]
    async fn only_mail_in_the_trash_is_deleted_for_good_with_the_next_push() {
        let fixture = Fixture::new("moving-delete");
        let dir = &fixture.dir;
        let trash = fixture.marked_folder("Trash", "trash");
        fixture.marked_folder("Bin", "trash");
        fs::write(trash.join("1.eml"), "Subject: old\r\n\r\nBye\r\n").unwrap();
        fixture.sync().await;
        let settings = fixture.settings();
        let kinds = Kinds::of(settings.account("private").unwrap());
        assert_eq!(kinds.folder(Kind::Trash), Some("bin"));
        let inbox = store::index(&dir.join("private/inbox")).unwrap();
        let trashed = store::index(&dir.join("private/trash")).unwrap();
        let (kept, old) = (
            store::relative(&inbox[&1], dir),
            store::relative(&trashed[&1], dir),
        );

        assert!(delete(dir, Path::new(&kept)).is_err());
        delete(dir, Path::new(&old)).unwrap();
        assert!(journal::overlay(dir).unwrap().hides(&old));
        assert!(delete(dir, Path::new(&old)).is_err());
        assert!(run(dir, Path::new(&old), "inbox", false).is_err());
        run(dir, Path::new(&kept), "trash", false).unwrap();
        let (moved, _) = show::open(dir, &settings, Path::new(&kept)).unwrap();
        assert_eq!(moved.style, settings.style(Some(Kind::Trash)));
        delete(dir, Path::new(&kept)).unwrap();

        let pushed = fixture.push().await;
        assert_eq!((pushed.pushed, pushed.dropped.len()), (2, 0));
        assert!(!trash.join("1.eml").exists());
        assert!(!fixture.dev.join("private/INBOX/1,S.eml").exists());
        assert!(!inbox[&1].exists() && !trashed[&1].exists());
        fs::remove_dir_all(&fixture.base).unwrap();
    }
}
