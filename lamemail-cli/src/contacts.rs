use std::collections::{BTreeSet, HashMap};
use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::kind::Kind;
use crate::list::Located;
use crate::message::{self, MailFile};
use crate::settings::Settings;
use crate::style::Color;
use crate::{output, settings, show, smtp};

const ACCOUNT_WIDTH: usize = 10;

#[derive(Debug, Serialize)]
pub struct Contact {
    pub name: Option<String>,
    pub address: String,
    pub account: String,
    pub color: Color,
    pub last: String,
    pub written: bool,
}

struct Seen {
    located: Located,
    file: MailFile,
    own: bool,
}

pub fn run(dir: &Path, json: bool) -> Result<()> {
    let settings = settings::load(dir)?;
    let contacts = contacts(dir, &settings)?;
    output::stdout(|out| write(out, &contacts, json))
}

fn contacts(dir: &Path, settings: &Settings) -> Result<Vec<Contact>> {
    let mut mails = Vec::new();
    let mut mine = BTreeSet::new();
    for id in settings.accounts.keys() {
        let (read, own) = show::read_account(dir, settings, id)?;
        mails.extend(
            read.into_iter()
                .filter(|(located, _)| located.kind != Some(Kind::Junk))
                .map(|(located, file)| Seen {
                    own: show::is_own(&file, located.kind, &own),
                    located,
                    file,
                }),
        );
        mine.extend(own);
    }
    mails.sort_by(|a, b| {
        b.located
            .time
            .cmp(&a.located.time)
            .then(b.located.uid.cmp(&a.located.uid))
    });
    let mut contacts = people(&mails, &mine);
    contacts.sort_by_key(|contact| !contact.written);
    Ok(contacts)
}

fn people(newest_first: &[Seen], mine: &BTreeSet<String>) -> Vec<Contact> {
    let mut known: HashMap<String, usize> = HashMap::new();
    let mut contacts: Vec<Contact> = Vec::new();
    for seen in newest_first {
        let file = &seen.file;
        for addr in file
            .from
            .iter()
            .chain(&file.to)
            .chain(&file.cc)
            .chain(&file.bcc)
        {
            let Some(address) = addr
                .address
                .as_deref()
                .map(str::trim)
                .filter(|address| smtp::is_address(address))
            else {
                continue;
            };
            let key = address.to_lowercase();
            if mine.contains(&key) {
                continue;
            }
            let name = name(addr.name.as_deref(), address);
            match known.get(&key) {
                Some(&at) => {
                    let contact = &mut contacts[at];
                    contact.name = contact.name.take().or(name);
                    contact.written |= seen.own;
                }
                None => {
                    known.insert(key, contacts.len());
                    contacts.push(Contact {
                        name,
                        address: address.to_string(),
                        account: seen.located.account.clone(),
                        color: seen.located.color,
                        last: file.received.clone(),
                        written: seen.own,
                    });
                }
            }
        }
    }
    contacts
}

fn name(name: Option<&str>, address: &str) -> Option<String> {
    let name = name?.split_whitespace().collect::<Vec<_>>().join(" ");
    (!name.is_empty() && !name.eq_ignore_ascii_case(address)).then_some(name)
}

fn write(out: &mut dyn Write, contacts: &[Contact], json: bool) -> io::Result<()> {
    for contact in contacts {
        if json {
            output::json_line(out, contact)?;
        } else {
            let mark = if contact.written { '>' } else { ' ' };
            let person = match &contact.name {
                Some(name) => format!("{name} <{}>", contact.address),
                None => contact.address.clone(),
            };
            writeln!(
                out,
                "{mark} {}  {:<ACCOUNT_WIDTH$}  {person}",
                message::minute(&contact.last),
                contact.account
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use super::*;
    use crate::settings::Account;
    use crate::testing::{assigned, write_message};

    fn account(username: &str) -> Account {
        let mut account = Account::new("imap.example.com".to_string(), 993, username.to_string());
        account.folders = assigned(&[
            ("inbox", Kind::Inbox),
            ("sent", Kind::Sent),
            ("spam", Kind::Junk),
        ]);
        account
    }

    #[test]
    fn contacts_are_the_people_written_to_first_then_everyone_newest_first() {
        let dir = std::env::temp_dir().join(format!("lamemail-contacts-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut accounts = BTreeMap::new();
        accounts.insert("business".to_string(), account("work@example.com"));
        accounts.insert("private".to_string(), account("me@example.com"));
        let settings = Settings {
            accounts,
            ..Settings::default()
        };
        write_message(
            &dir,
            "private/inbox/2026-01-01_1000-a-1.json",
            "Paul Berger <paul@example.com>",
            &["me@example.com", "work@example.com"],
            None,
        );
        write_message(
            &dir,
            "private/sent/2026-01-02_1000-b-1.json",
            "me@example.com",
            &["Maria <maria@example.com>", "paul@example.com"],
            None,
        );
        write_message(
            &dir,
            "business/inbox/2026-01-03_1000-c-1.json",
            "paul@example.com",
            &["Work <work@example.com>"],
            None,
        );
        write_message(
            &dir,
            "private/inbox/2026-01-04_1000-d-2.json",
            "news@example.com",
            &["me@example.com", "Undisclosed recipients"],
            None,
        );
        write_message(
            &dir,
            "private/spam/2026-01-05_1000-e-1.json",
            "spam@example.com",
            &["me@example.com"],
            None,
        );

        let contacts = contacts(&dir, &settings).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        let found: Vec<_> = contacts
            .iter()
            .map(|contact| {
                (
                    contact.address.as_str(),
                    contact.name.as_deref(),
                    contact.account.as_str(),
                    contact.written,
                )
            })
            .collect();
        assert_eq!(
            found,
            [
                ("paul@example.com", Some("Paul Berger"), "business", true),
                ("maria@example.com", Some("Maria"), "private", true),
                ("news@example.com", None, "private", false),
            ]
        );
        assert_eq!(contacts[0].last, "2026-01-03T10:00:00+02:00");
    }
}
