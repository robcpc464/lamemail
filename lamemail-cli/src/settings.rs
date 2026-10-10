use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::i18n::{fill, t};
use crate::kind::{Assigned, Kind};
use crate::store;
use crate::style::{self, Color, Overrides, Style};

pub const SETTINGS_FILE: &str = "settings.json";
pub const SMTP_PORT: u16 = 465;
const GOOGLE: [&str; 2] = ["gmail.com", "googlemail.com"];
const MICROSOFT: [&str; 2] = ["office365.com", "outlook.com"];

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub version: u32,
    #[serde(default)]
    pub accounts: BTreeMap<String, Account>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub kind_styles: BTreeMap<Kind, Overrides>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai: Option<Ai>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_limit: Option<usize>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            version: 1,
            accounts: BTreeMap::new(),
            kind_styles: BTreeMap::new(),
            ai: None,
            custom_command: None,
            context_limit: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Ai {
    ClaudeCode,
    Ollama(String),
    Custom,
    #[serde(untagged)]
    Command {
        command: String,
    },
}

impl Settings {
    pub fn account(&self, id: &str) -> Result<&Account> {
        self.accounts
            .get(id)
            .with_context(|| fill(t().unknown_account, &[&id]))
    }

    pub fn account_mut(&mut self, id: &str) -> Result<&mut Account> {
        self.accounts
            .get_mut(id)
            .with_context(|| fill(t().unknown_account, &[&id]))
    }

    pub fn color(&self, id: &str) -> Color {
        let position = self.accounts.keys().position(|key| key == id).unwrap_or(0);
        self.accounts
            .get(id)
            .and_then(|account| account.color)
            .unwrap_or(Color::nth(position))
    }

    pub fn add(&mut self, id: String, mut account: Account) {
        let taken: Vec<Color> = self.accounts.keys().map(|id| self.color(id)).collect();
        for (existing, color) in self.accounts.values_mut().zip(&taken) {
            existing.color = Some(*color);
        }
        account.color = Some(Color::unused(&taken));
        self.accounts.insert(id, account);
    }

    pub fn kind_of(&self, id: &str, folder: &str) -> Option<Kind> {
        self.accounts.get(id)?.kind_of(folder)
    }

    pub fn style(&self, kind: Option<Kind>) -> Style {
        style::resolve(kind, &self.kind_styles)
    }

    pub fn login(&self, id: &str) -> Option<&str> {
        self.accounts
            .get(id)
            .map(|account| account.username.as_str())
    }

    pub fn chosen(&self, ids: Vec<String>) -> Result<Vec<(String, Account)>> {
        let ids: BTreeSet<String> = if ids.is_empty() {
            self.accounts.keys().cloned().collect()
        } else {
            ids.into_iter().collect()
        };
        if ids.is_empty() {
            bail!("{}", t().no_accounts);
        }
        ids.into_iter()
            .map(|id| {
                let account = self.account(&id)?.clone();
                Ok((id, account))
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smtp: Option<Smtp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_folder: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_by_year: Option<bool>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub folders: BTreeMap<String, Assigned>,
}

impl Account {
    pub fn new(host: String, port: u16, username: String) -> Account {
        Account {
            host,
            port,
            username,
            smtp: None,
            color: None,
            signature: None,
            context_folder: None,
            archive_by_year: None,
            folders: BTreeMap::new(),
        }
    }

    pub fn archives_by_year(&self) -> bool {
        self.archive_by_year
            .unwrap_or_else(|| !on_domain(&self.host, &GOOGLE))
    }

    pub fn kind_of(&self, folder: &str) -> Option<Kind> {
        self.folders.get(folder).and_then(|assigned| assigned.kind)
    }

    pub fn synced_kind(&self, folder: &str) -> Option<Kind> {
        self.folders
            .get(folder)
            .filter(|assigned| assigned.sync)
            .and_then(|assigned| assigned.kind)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Smtp {
    pub host: String,
    pub port: u16,
    pub copy_to_sent: bool,
}

impl Smtp {
    pub fn new(host: String, port: u16, copy_to_sent: Option<bool>) -> Smtp {
        let copy_to_sent = copy_to_sent
            .unwrap_or_else(|| !on_domain(&host, &GOOGLE) && !on_domain(&host, &MICROSOFT));
        Smtp {
            host,
            port,
            copy_to_sent,
        }
    }
}

pub fn changed_smtp(
    current: Option<&Smtp>,
    host: Option<String>,
    port: Option<u16>,
    copy: Option<bool>,
) -> Option<Smtp> {
    let host = host.or_else(|| current.map(|smtp| smtp.host.clone()))?;
    let port = port.or(current.map(|smtp| smtp.port)).unwrap_or(SMTP_PORT);
    Some(Smtp::new(
        host,
        port,
        copy.or(current.map(|smtp| smtp.copy_to_sent)),
    ))
}

fn on_domain(host: &str, domains: &[&str]) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    domains.iter().any(|domain| {
        host == *domain
            || host
                .strip_suffix(domain)
                .is_some_and(|rest| rest.ends_with('.'))
    })
}

pub fn signature(text: &str) -> Option<String> {
    let mut lines = text
        .lines()
        .map(str::trim_end)
        .skip_while(|line| line.is_empty())
        .peekable();
    lines.next_if_eq(&"--");
    let text = lines.collect::<Vec<_>>().join("\n");
    let text = text.trim_matches('\n');
    (!text.is_empty()).then(|| text.to_string())
}

pub fn context_folder(path: &Path) -> Result<PathBuf> {
    let folder =
        std::path::absolute(path).with_context(|| format!("resolving {}", path.display()))?;
    if !folder.is_dir() {
        bail!("{}", fill(t().not_a_folder, &[&folder.display()]));
    }
    Ok(folder)
}

pub fn sync_dir(cli: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(dir) = cli {
        return Ok(dir);
    }
    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home).join(".lamemail"))
}

pub fn load(dir: &Path) -> Result<Settings> {
    let path = dir.join(SETTINGS_FILE);
    let raw = match fs::read(&path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            bail!("{}", fill(t().no_settings, &[&dir.display()]))
        }
        Err(err) => return Err(err).with_context(|| format!("reading {}", path.display())),
    };
    let settings: Settings =
        serde_json::from_slice(&raw).with_context(|| format!("parsing {}", path.display()))?;
    if settings.version != 1 {
        bail!("unsupported settings version {}", settings.version);
    }
    Ok(settings)
}

pub fn save(dir: &Path, settings: &Settings) -> Result<()> {
    store::write_atomic(&dir.join(SETTINGS_FILE), settings)
}

pub fn init(dir: &Path) -> Result<bool> {
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    if dir.join(SETTINGS_FILE).exists() {
        return Ok(false);
    }
    save(dir, &Settings::default())?;
    Ok(true)
}

pub fn validate_id(id: &str) -> Result<()> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !ok {
        bail!("{}", fill(t().invalid_id, &[&id]));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        Account, Kind, Path, Settings, Smtp, changed_smtp, context_folder, fill, signature, t,
        validate_id,
    };
    use crate::style::Color;
    use std::collections::BTreeMap;

    #[test]
    fn accounts_take_the_palette_in_id_order_unless_a_colour_is_set() {
        let account = |color| Account {
            color,
            ..Account::new(
                "imap.example.com".to_string(),
                993,
                "me@example.com".to_string(),
            )
        };
        let settings = Settings {
            accounts: BTreeMap::from([
                ("private".to_string(), account(None)),
                ("business".to_string(), account(None)),
                ("club".to_string(), account(Some(Color::Orange))),
            ]),
            ..Settings::default()
        };
        assert_eq!(settings.color("business"), Color::Blue);
        assert_eq!(settings.color("club"), Color::Orange);
        assert_eq!(settings.color("private"), Color::Magenta);
        assert_eq!(Color::nth(7), Color::Green);
    }

    #[test]
    fn styles_are_kept_per_kind_and_the_old_sections_by_name_are_dropped_on_save() {
        let read: Settings = serde_json::from_str(
            r#"{"version":1,"folders":{"spam":{"hidden":true}},"folder_styles":{"lists/rust":{"dense":true}},"kind_styles":{"junk":{"dense":false}}}"#,
        )
        .unwrap();
        assert!(!read.style(Some(Kind::Junk)).dense);
        assert!(read.style(Some(Kind::Junk)).dim);
        let written = serde_json::to_string(&read).unwrap();
        assert!(written.ends_with(r#""kind_styles":{"junk":{"dense":false}}}"#));
        assert!(!written.contains("folders") && !written.contains("lists/rust"));
    }

    #[test]
    fn a_new_account_pins_the_colours_and_takes_the_first_free_one() {
        let account = || {
            Account::new(
                "imap.example.com".to_string(),
                993,
                "me@example.com".to_string(),
            )
        };
        let mut settings = Settings {
            accounts: BTreeMap::from([
                ("private".to_string(), account()),
                ("business".to_string(), account()),
            ]),
            ..Settings::default()
        };
        settings.add("archive".to_string(), account());
        assert_eq!(settings.color("business"), Color::Blue);
        assert_eq!(settings.color("private"), Color::Green);
        assert_eq!(settings.color("archive"), Color::Magenta);
        settings.accounts.remove("archive");
        settings.accounts.remove("business");
        assert_eq!(settings.color("private"), Color::Green);
    }

    #[test]
    fn chosen_accounts_are_the_given_ones_once_or_all() {
        let account = Account::new(
            "imap.example.com".to_string(),
            993,
            "me@example.com".to_string(),
        );
        let mut settings = Settings {
            accounts: BTreeMap::from([
                ("private".to_string(), account.clone()),
                ("business".to_string(), account),
            ]),
            ..Settings::default()
        };
        let ids = |given: &[&str]| {
            settings
                .chosen(given.iter().map(|id| id.to_string()).collect())
                .map(|chosen| chosen.into_iter().map(|(id, _)| id).collect::<Vec<_>>())
        };
        assert_eq!(ids(&[]).unwrap(), ["business", "private"]);
        assert_eq!(ids(&["private", "private"]).unwrap(), ["private"]);
        assert!(ids(&["nosuch"]).is_err());
        settings.accounts.clear();
        assert!(settings.chosen(Vec::new()).is_err());
    }

    #[test]
    fn a_signature_is_kept_without_its_separator_and_the_blank_edges() {
        assert_eq!(
            signature("\n-- \nRobin  \n\n  example.com\n\n").as_deref(),
            Some("Robin\n\n  example.com")
        );
        assert_eq!(signature("Robin\r\n").as_deref(), Some("Robin"));
        assert_eq!(signature("--Robin").as_deref(), Some("--Robin"));
        assert_eq!(signature(" \n--\n\n"), None);
        assert_eq!(signature(""), None);
    }

    #[test]
    fn a_context_folder_is_kept_absolute_and_must_be_a_folder() {
        let here = context_folder(Path::new("src")).unwrap();
        assert!(here.is_absolute() && here.ends_with("src"));
        let file = std::path::absolute("Cargo.toml").unwrap();
        assert_eq!(
            context_folder(Path::new("Cargo.toml"))
                .unwrap_err()
                .to_string(),
            fill(t().not_a_folder, &[&file.display()])
        );
    }

    #[test]
    fn ids_are_plain_directory_names() {
        assert!(validate_id("private").is_ok());
        assert!(validate_id("work-2").is_ok());
        assert!(validate_id("").is_err());
        assert!(validate_id("Private").is_err());
        assert!(validate_id("a/b").is_err());
        assert!(validate_id("..").is_err());
    }

    #[test]
    fn gmail_and_microsoft_keep_sent_mail_without_a_copy() {
        let copy = |host: &str| Smtp::new(host.to_string(), 465, None).copy_to_sent;
        assert!(!copy("smtp.gmail.com"));
        assert!(!copy("smtp.googlemail.com"));
        assert!(!copy("smtp.office365.com"));
        assert!(!copy("SMTP-mail.Outlook.com."));
        assert!(copy("smtp.example.com"));
        assert!(copy("mail.notgmail.com"));
        assert!(Smtp::new("smtp.gmail.com".to_string(), 465, Some(true)).copy_to_sent);
        assert!(!Smtp::new("smtp.example.com".to_string(), 587, Some(false)).copy_to_sent);
    }

    #[test]
    fn mail_is_archived_by_year_unless_the_archive_is_gmails_all_mail_or_it_is_switched() {
        let account = |host: &str, by_year| Account {
            archive_by_year: by_year,
            ..Account::new(host.to_string(), 993, "me@example.com".to_string())
        };
        assert!(account("imap.example.com", None).archives_by_year());
        assert!(account("outlook.office365.com", None).archives_by_year());
        assert!(!account("imap.gmail.com", None).archives_by_year());
        assert!(!account("imap.example.com", Some(false)).archives_by_year());
        assert!(account("imap.gmail.com", Some(true)).archives_by_year());
    }

    #[test]
    fn omitted_smtp_options_keep_what_is_stored() {
        assert!(changed_smtp(None, None, Some(587), None).is_none());
        let first = changed_smtp(None, Some("smtp.gmail.com".to_string()), None, None).unwrap();
        assert_eq!((first.port, first.copy_to_sent), (465, false));
        let copied = changed_smtp(Some(&first), None, None, Some(true)).unwrap();
        assert_eq!(
            (copied.host.as_str(), copied.port, copied.copy_to_sent),
            ("smtp.gmail.com", 465, true)
        );
        let moved = changed_smtp(
            Some(&copied),
            Some("mail.example.com".to_string()),
            Some(587),
            None,
        )
        .unwrap();
        assert_eq!((moved.port, moved.copy_to_sent), (587, true));
    }
}
