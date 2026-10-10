use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::i18n::{fill, t};
use crate::message::Flags;

pub const STATE_FILE: &str = ".folder.json";
const LOCK_FILE: &str = ".lock";
const MAX_SLUG: usize = 60;

#[derive(Deserialize)]
struct FlagsOnly {
    flags: Flags,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderState {
    pub imap_name: String,
    pub uidvalidity: u32,
    pub last_sync: Option<String>,
}

pub fn file_name(received: &jiff::Zoned, subject: &str, uid: u32) -> String {
    format!("{}-{}-{uid}.json", stamp(received), slug(subject))
}

pub fn stamp(received: &jiff::Zoned) -> String {
    received.strftime("%Y-%m-%d_%H%M").to_string()
}

pub fn slug(subject: &str) -> String {
    let mut out = String::with_capacity(subject.len());
    let mut dash = true;
    for c in subject.to_lowercase().chars() {
        let mapped: &str = match c {
            'a'..='z' | '0'..='9' => {
                out.push(c);
                dash = false;
                continue;
            }
            'ä' => "ae",
            'ö' => "oe",
            'ü' => "ue",
            'ß' => "ss",
            _ => {
                if !dash {
                    out.push('-');
                    dash = true;
                }
                continue;
            }
        };
        out.push_str(mapped);
        dash = false;
    }
    out.truncate(MAX_SLUG);
    let trimmed = out.trim_end_matches('-');
    if trimmed.is_empty() {
        "no-subject".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn relative(path: &Path, base: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

pub fn uid_from_name(name: &str) -> Option<u32> {
    if name.starts_with('.') {
        return None;
    }
    let stem = name.strip_suffix(".json")?;
    let (_, uid) = stem.rsplit_once('-')?;
    uid.parse().ok()
}

pub fn index(folder_dir: &Path) -> Result<BTreeMap<u32, PathBuf>> {
    let mut map = BTreeMap::new();
    let entries = match fs::read_dir(folder_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(map),
        Err(err) => return Err(err).with_context(|| format!("reading {}", folder_dir.display())),
    };
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        if let Some(uid) = entry.file_name().to_str().and_then(uid_from_name) {
            map.insert(uid, entry.path());
        }
    }
    Ok(map)
}

pub fn read<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let raw = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_slice(&raw).with_context(|| format!("parsing {}", path.display()))
}

pub fn read_flags(path: &Path) -> Result<Flags> {
    read(path).map(|only: FlagsOnly| only.flags)
}

pub fn read_or_skip<T: DeserializeOwned>(path: &Path) -> Option<T> {
    match read(path) {
        Ok(file) => Some(file),
        Err(err) => {
            eprintln!(
                "{}",
                fill(t().file_skipped, &[&path.display(), &format!("{err:#}")])
            );
            None
        }
    }
}

pub fn write_atomic<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let mut json = serde_json::to_string_pretty(value)?;
    json.push('\n');
    write_bytes_atomic(path, json.as_bytes())
}

pub fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .context("file name")?;
    let tmp = path.with_file_name(format!(".{file_name}.tmp"));
    fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("renaming to {}", path.display()))
}

pub fn read_optional<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    match fs::read(path) {
        Ok(raw) => Ok(Some(
            serde_json::from_slice(&raw).with_context(|| format!("parsing {}", path.display()))?,
        )),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("reading {}", path.display())),
    }
}

pub fn read_state(folder_dir: &Path) -> Result<Option<FolderState>> {
    read_optional(&folder_dir.join(STATE_FILE))
}

pub fn state_of(message_file: &Path) -> Result<FolderState> {
    let folder_dir = message_file
        .parent()
        .context("message file without folder")?;
    read_state(folder_dir)?.with_context(|| format!("no {STATE_FILE} in {}", folder_dir.display()))
}

pub fn write_state(folder_dir: &Path, state: &FolderState) -> Result<()> {
    write_atomic(&folder_dir.join(STATE_FILE), state)
}

pub fn clear_messages(folder_dir: &Path) -> Result<usize> {
    let messages = index(folder_dir)?;
    for path in messages.values() {
        fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
    }
    Ok(messages.len())
}

pub fn remove_folder(folder_dir: &Path) -> Result<usize> {
    let removed = clear_messages(folder_dir)?;
    let state = folder_dir.join(STATE_FILE);
    if state.exists() {
        fs::remove_file(&state).with_context(|| format!("removing {}", state.display()))?;
    }
    let empty = fs::read_dir(folder_dir)
        .with_context(|| format!("reading {}", folder_dir.display()))?
        .next()
        .is_none();
    if empty {
        fs::remove_dir(folder_dir).with_context(|| format!("removing {}", folder_dir.display()))?;
    }
    Ok(removed)
}

pub fn wait_for_lock(dir: &Path) -> Result<File> {
    lock(&dir.join(LOCK_FILE))
}

pub fn lock(path: &Path) -> Result<File> {
    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    file.lock()
        .with_context(|| format!("locking {}", path.display()))?;
    Ok(file)
}

pub fn local_folders(account_dir: &Path) -> Result<Vec<(PathBuf, FolderState)>> {
    let mut found = Vec::new();
    let mut pending = vec![account_dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err).with_context(|| format!("reading {}", dir.display())),
        };
        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                pending.push(entry.path());
            }
        }
        if let Some(state) = read_state(&dir)? {
            found.push((dir, state));
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_short_lowercase_ascii() {
        assert_eq!(slug("Offer for project"), "offer-for-project");
        assert_eq!(slug("Angebot für Größe & Maß"), "angebot-fuer-groesse-mass");
        assert_eq!(slug("  Re: [list] Hello!!  "), "re-list-hello");
        assert_eq!(slug(""), "no-subject");
        assert_eq!(slug("!!!"), "no-subject");
        assert_eq!(slug("日本語"), "no-subject");
        let long = slug(&"word ".repeat(30));
        assert!(long.len() <= MAX_SLUG);
        assert!(!long.ends_with('-'));
    }

    #[test]
    fn uid_comes_from_the_file_name() {
        assert_eq!(
            uid_from_name("2026-09-15_1432-offer-for-project-4711.json"),
            Some(4711)
        );
        assert_eq!(uid_from_name("2026-09-15_1432-no-subject-1.json"), Some(1));
        assert_eq!(uid_from_name(".folder.json"), None);
        assert_eq!(uid_from_name(".2026-09-15_1432-x-3.json.tmp"), None);
        assert_eq!(uid_from_name("notes.txt"), None);
        assert_eq!(uid_from_name("x-abc.json"), None);
    }

    #[test]
    fn flags_are_read_only_from_a_complete_file() {
        let dir = std::env::temp_dir().join(format!("lamemail-store-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("2026-09-15_1432-x-1.json");
        let flags = r#""flags":{"seen":true,"answered":false,"flagged":false,"draft":false,"keywords":["$label1"]}"#;
        fs::write(&path, format!(r#"{{"uid":1,{flags},"body":"hi"}}"#)).unwrap();
        let read = read_flags(&path).unwrap();
        assert!(read.seen && !read.flagged);
        assert_eq!(read.keywords, ["$label1"]);
        fs::write(&path, format!(r#"{{"uid":1,{flags},"body":"h"#)).unwrap();
        assert!(read_flags(&path).is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn file_names_carry_time_slug_and_uid() {
        let zoned: jiff::Zoned = "2026-09-15T14:32:05+02:00[+02:00]".parse().unwrap();
        assert_eq!(
            file_name(&zoned, "Offer for project", 4711),
            "2026-09-15_1432-offer-for-project-4711.json"
        );
    }
}
