use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};
use clap::ValueEnum;
use clap::builder::PossibleValuesParser;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::i18n::{fill, t};
use crate::imap::{self, Special};
use crate::settings::Account;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Inbox,
    Sent,
    Drafts,
    Junk,
    Trash,
    Archive,
}

pub const OTHER: &str = "other";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Assigned {
    #[serde(with = "or_other")]
    pub kind: Option<Kind>,
    pub sync: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub default: bool,
}

#[derive(Debug, Serialize)]
pub struct Kinds(BTreeMap<Kind, Option<String>>);

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Inbox => "inbox",
            Kind::Sent => "sent",
            Kind::Drafts => "drafts",
            Kind::Archive => "archive",
            Kind::Junk => "junk",
            Kind::Trash => "trash",
        }
    }

    fn marked(special: Special) -> Kind {
        match special {
            Special::Sent => Kind::Sent,
            Special::Drafts => Kind::Drafts,
            Special::Archive | Special::All => Kind::Archive,
            Special::Junk => Kind::Junk,
            Special::Trash => Kind::Trash,
        }
    }
}

pub fn parse(name: &str) -> Result<Option<Kind>, String> {
    if name == OTHER {
        return Ok(None);
    }
    Kind::from_str(name, false).map(Some)
}

pub fn names() -> PossibleValuesParser {
    PossibleValuesParser::new(
        Kind::value_variants()
            .iter()
            .map(|kind| kind.as_str())
            .chain([OTHER]),
    )
}

pub fn rank(moved: bool, kind: Option<Kind>) -> (bool, bool, Option<Kind>) {
    (!moved, kind.is_none(), kind)
}

pub mod or_other {
    use super::*;

    pub fn serialize<S: Serializer>(kind: &Option<Kind>, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(kind.map_or(OTHER, Kind::as_str))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Kind>, D::Error> {
        parse(&String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl Kinds {
    pub fn of(account: &Account) -> Kinds {
        Kinds(
            Kind::value_variants()
                .iter()
                .map(|kind| (*kind, default_folder(account, *kind)))
                .collect(),
        )
    }

    pub fn folder(&self, kind: Kind) -> Option<&str> {
        self.0.get(&kind)?.as_deref()
    }

    pub fn missing(&self) -> Vec<&'static str> {
        self.0
            .iter()
            .filter(|(_, folder)| folder.is_none())
            .map(|(kind, _)| kind.as_str())
            .collect()
    }

    pub fn note(&self, id: &str) -> Option<String> {
        let missing = self.missing();
        (!missing.is_empty()).then(|| fill(t().kinds_missing, &[&id, &missing.join(", "), &id]))
    }
}

pub fn no_folder(account: &str, kind: Kind) -> String {
    fill(
        t().no_kind_folder,
        &[&account, &kind.as_str(), &account, &kind.as_str()],
    )
}

fn default_folder(account: &Account, kind: Kind) -> Option<String> {
    account
        .folders
        .iter()
        .filter(|(_, assigned)| assigned.kind == Some(kind) && assigned.sync)
        .min_by_key(|(_, assigned)| !assigned.default)
        .map(|(name, _)| name.clone())
}

pub fn assign(folders: &mut BTreeMap<String, Assigned>, listed: &[imap::Folder]) -> bool {
    let listed: BTreeMap<(bool, String), &imap::Folder> = listed
        .iter()
        .filter(|folder| folder.selectable)
        .filter_map(|folder| {
            let local = folder.local_path().ok()?;
            let all = folder.special == Some(Special::All);
            Some(((all, local.to_string_lossy().into_owned()), folder))
        })
        .collect();
    let names: BTreeSet<&str> = listed.keys().map(|(_, local)| local.as_str()).collect();
    let before = folders.len();
    folders.retain(|name, _| names.contains(name.as_str()));
    let mut changed = folders.len() != before;
    for ((_, local), folder) in &listed {
        if folders.contains_key(local) {
            continue;
        }
        let kind = detect(folder);
        let default =
            kind.is_some_and(|kind| !folders.values().any(|other| other.kind == Some(kind)));
        folders.insert(
            local.clone(),
            Assigned {
                kind,
                sync: kind.is_some(),
                default,
            },
        );
        changed = true;
    }
    changed
}

fn detect(folder: &imap::Folder) -> Option<Kind> {
    if folder.is_inbox() {
        return Some(Kind::Inbox);
    }
    folder.special.map(Kind::marked)
}

pub fn reassign(
    folders: &mut BTreeMap<String, Assigned>,
    name: &str,
    kind: Option<Kind>,
    sync: Option<bool>,
    default: bool,
) -> Result<Assigned> {
    let Some(before) = folders.get(name).copied() else {
        bail!("{}", fill(t().folder_unknown, &[&name]));
    };
    if kind.is_none() && (default || sync == Some(true)) {
        bail!("{}", t().folder_other);
    }
    let same = before.kind == kind;
    let assigned = Assigned {
        kind,
        sync: sync.unwrap_or(if same { before.sync } else { kind.is_some() }),
        default: default || (same && before.default),
    };
    if assigned.default {
        for other in folders.values_mut() {
            other.default &= other.kind != kind;
        }
    }
    folders.insert(name.to_string(), assigned);
    Ok(assigned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(name: &str, special: Option<Special>, selectable: bool) -> imap::Folder {
        imap::Folder {
            name: name.to_string(),
            delimiter: Some("/".to_string()),
            selectable,
            special,
        }
    }

    fn kinds(folders: &BTreeMap<String, Assigned>) -> Vec<(&str, Option<Kind>, bool, bool)> {
        folders
            .iter()
            .map(|(name, assigned)| {
                (
                    name.as_str(),
                    assigned.kind,
                    assigned.sync,
                    assigned.default,
                )
            })
            .collect()
    }

    fn assigned(kind: Option<Kind>, sync: bool, default: bool) -> Assigned {
        Assigned {
            kind,
            sync,
            default,
        }
    }

    #[test]
    fn a_new_folder_is_the_inbox_or_takes_its_marking_else_other() {
        let server = [
            listed("INBOX", None, true),
            listed("INBOX/Rechnungen", None, true),
            listed("[Gmail]", None, false),
            listed("[Gmail]/All Mail", Some(Special::All), true),
            listed("[Gmail]/Important", None, true),
            listed("Archive", Some(Special::Archive), true),
            listed("Archive/2024", None, true),
            listed("Papierkorb", None, true),
            listed("Bin", Some(Special::Trash), true),
        ];
        let mut folders = BTreeMap::new();
        assert!(assign(&mut folders, &server));
        assert_eq!(
            kinds(&folders),
            [
                ("[gmail]/all mail", Some(Kind::Archive), true, false),
                ("[gmail]/important", None, false, false),
                ("archive", Some(Kind::Archive), true, true),
                ("archive/2024", None, false, false),
                ("bin", Some(Kind::Trash), true, true),
                ("inbox", Some(Kind::Inbox), true, true),
                ("inbox/rechnungen", None, false, false),
                ("papierkorb", None, false, false),
            ]
        );
        assert!(!assign(&mut folders, &server));
    }

    #[test]
    fn an_assignment_stays_and_a_folder_gone_from_the_server_leaves() {
        let server = [listed("INBOX", None, true), listed("Notes", None, true)];
        let mut folders = BTreeMap::new();
        assign(&mut folders, &server);
        folders.get_mut("notes").unwrap().kind = Some(Kind::Archive);
        assert!(!assign(&mut folders, &server));
        assert_eq!(folders["notes"].kind, Some(Kind::Archive));
        assert!(assign(&mut folders, &server[..1]));
        assert_eq!(kinds(&folders), [("inbox", Some(Kind::Inbox), true, true)]);
    }

    #[test]
    fn the_folder_of_a_kind_is_its_synchronized_default_else_the_first_synchronized() {
        let mut account = Account::new("imap.example.com".to_string(), 993, "me".to_string());
        account.folders = BTreeMap::from([
            ("inbox".to_string(), assigned(Some(Kind::Inbox), true, true)),
            (
                "archive".to_string(),
                assigned(Some(Kind::Archive), true, false),
            ),
            (
                "archive/2024".to_string(),
                assigned(Some(Kind::Archive), true, true),
            ),
            ("bin".to_string(), assigned(Some(Kind::Trash), false, true)),
            (
                "papierkorb".to_string(),
                assigned(Some(Kind::Trash), true, false),
            ),
            ("spam".to_string(), assigned(Some(Kind::Junk), false, true)),
            ("notes".to_string(), assigned(None, false, false)),
        ]);
        let kinds = Kinds::of(&account);
        assert_eq!(kinds.folder(Kind::Archive), Some("archive/2024"));
        assert_eq!(kinds.folder(Kind::Trash), Some("papierkorb"));
        assert_eq!(kinds.missing(), ["sent", "drafts", "junk"]);
        assert_eq!(
            serde_json::to_value(&kinds).unwrap(),
            serde_json::json!({
                "inbox": "inbox",
                "sent": null,
                "drafts": null,
                "junk": null,
                "trash": "papierkorb",
                "archive": "archive/2024"
            })
        );
    }

    #[test]
    fn a_reassigned_folder_keeps_what_is_not_said_and_takes_the_default_from_the_others() {
        let mut folders = BTreeMap::from([
            ("bin".to_string(), assigned(Some(Kind::Trash), true, true)),
            ("papierkorb".to_string(), assigned(None, false, false)),
        ]);
        let papierkorb =
            reassign(&mut folders, "papierkorb", Some(Kind::Trash), None, true).unwrap();
        assert_eq!(papierkorb, assigned(Some(Kind::Trash), true, true));
        assert!(!folders["bin"].default);
        let quiet = reassign(
            &mut folders,
            "papierkorb",
            Some(Kind::Trash),
            Some(false),
            false,
        );
        assert_eq!(quiet.unwrap(), assigned(Some(Kind::Trash), false, true));
        let again = reassign(&mut folders, "papierkorb", Some(Kind::Trash), None, false);
        assert_eq!(again.unwrap(), assigned(Some(Kind::Trash), false, true));
        let archive = reassign(&mut folders, "papierkorb", Some(Kind::Archive), None, false);
        assert_eq!(archive.unwrap(), assigned(Some(Kind::Archive), true, false));
        let other = reassign(&mut folders, "bin", None, None, false).unwrap();
        assert_eq!(other, assigned(None, false, false));
        assert!(reassign(&mut folders, "bin", None, Some(true), false).is_err());
        assert!(reassign(&mut folders, "bin", None, None, true).is_err());
        assert!(reassign(&mut folders, "nowhere", Some(Kind::Trash), None, false).is_err());
    }

    #[test]
    fn a_moved_copy_ranks_first_then_by_kind_and_no_kind_last() {
        let mut ranks = [
            rank(false, None),
            rank(false, Some(Kind::Trash)),
            rank(true, Some(Kind::Trash)),
            rank(false, Some(Kind::Inbox)),
        ];
        ranks.sort();
        assert_eq!(
            ranks,
            [
                rank(true, Some(Kind::Trash)),
                rank(false, Some(Kind::Inbox)),
                rank(false, Some(Kind::Trash)),
                rank(false, None),
            ]
        );
    }

    #[test]
    fn other_is_a_kind_of_its_own_in_the_settings() {
        assert_eq!(parse("other"), Ok(None));
        assert_eq!(parse("trash"), Ok(Some(Kind::Trash)));
        assert!(parse("bin").is_err());
        let read: Assigned = serde_json::from_str(r#"{"kind":"other","sync":false}"#).unwrap();
        assert_eq!(read, assigned(None, false, false));
        assert_eq!(
            serde_json::to_string(&assigned(Some(Kind::Sent), true, true)).unwrap(),
            r#"{"kind":"sent","sync":true,"default":true}"#
        );
        assert_eq!(
            serde_json::to_string(&assigned(None, false, false)).unwrap(),
            r#"{"kind":"other","sync":false}"#
        );
    }
}
