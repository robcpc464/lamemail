use std::collections::BTreeSet;
use std::hash::{BuildHasher, Hasher, RandomState};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use jiff::Timestamp;
use mail_builder::MessageBuilder;
use mail_builder::headers::address::Address;
use mail_builder::headers::date::Date;
use serde::{Deserialize, Deserializer, Serialize};

use crate::i18n::{fill, t};
use crate::message::{self, Addr, Attachment};
use crate::show::{Identities, Message};
use crate::smtp;

const REPLY_PREFIXES: [&str; 2] = ["re:", "aw:"];
const FORWARD_PREFIXES: [&str; 3] = ["fwd:", "fw:", "wg:"];

#[derive(Default)]
pub struct Choice {
    pub from: Option<Addr>,
    pub to: Vec<Addr>,
    pub cc: Vec<Addr>,
    pub bcc: Vec<Addr>,
    pub subject: Option<String>,
    pub files: Vec<PathBuf>,
    pub draft: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outgoing {
    pub message_id: String,
    pub date: String,
    pub from: Addr,
    pub to: Vec<Addr>,
    pub cc: Vec<Addr>,
    pub bcc: Vec<Addr>,
    pub subject: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_reply_to: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<String>,
    pub body: String,
    #[serde(
        default,
        alias = "original",
        deserialize_with = "one_or_more",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub originals: Vec<Original>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<LocalFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalFile {
    pub path: PathBuf,
    pub name: String,
    #[serde(rename = "type")]
    pub content_type: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Original {
    pub folder: String,
    pub uidvalidity: u32,
    pub uid: u32,
    pub attachments: Vec<Attachment>,
}

fn one_or_more<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Original>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Originals {
        More(Vec<Original>),
        One(Original),
    }
    Ok(match Originals::deserialize(deserializer)? {
        Originals::More(originals) => originals,
        Originals::One(original) => vec![original],
    })
}

pub struct File {
    pub name: String,
    pub content_type: String,
    pub content: Vec<u8>,
}

struct Parties {
    from: Addr,
    to: Vec<Addr>,
    cc: Vec<Addr>,
    bcc: Vec<Addr>,
}

impl Outgoing {
    pub fn sender(&self) -> &str {
        self.from.address.as_deref().unwrap_or_default()
    }

    pub fn recipients(&self) -> Vec<String> {
        self.to
            .iter()
            .chain(&self.cc)
            .chain(&self.bcc)
            .filter_map(|addr| addr.address.clone())
            .collect()
    }

    pub fn raw(&self, files: &[File]) -> Result<Vec<u8>> {
        self.write(files, false)
    }

    pub fn copy(&self, files: &[File]) -> Result<Vec<u8>> {
        self.write(files, true)
    }

    fn write(&self, files: &[File], with_bcc: bool) -> Result<Vec<u8>> {
        let date: Timestamp = self
            .date
            .parse()
            .with_context(|| format!("the date of {}", self.message_id))?;
        let mut builder = MessageBuilder::new()
            .message_id(self.message_id.as_str())
            .date(Date::new(date.as_second()))
            .from(address(&self.from))
            .subject(self.subject.as_str());
        if !self.to.is_empty() {
            builder = builder.to(list(&self.to));
        }
        if !self.cc.is_empty() {
            builder = builder.cc(list(&self.cc));
        }
        if with_bcc && !self.bcc.is_empty() {
            builder = builder.bcc(list(&self.bcc));
        }
        if let Some(parent) = &self.in_reply_to {
            builder = builder
                .in_reply_to(parent.as_str())
                .references(self.references.clone());
        }
        for file in files {
            builder = builder.attachment(
                file.content_type.as_str(),
                file.name.as_str(),
                file.content.as_slice(),
            );
        }
        builder
            .text_body(self.body.as_str())
            .write_to_vec()
            .context("building the message")
    }
}

pub fn choice(
    from: Option<&str>,
    to: &[String],
    cc: &[String],
    bcc: &[String],
    subject: Option<String>,
) -> Result<Choice> {
    let parse_all = |addresses: &[String]| -> Result<Vec<Addr>> {
        addresses
            .iter()
            .map(|address| parse_address(address))
            .collect()
    };
    Ok(Choice {
        from: from.map(parse_address).transpose()?,
        to: parse_all(to)?,
        cc: parse_all(cc)?,
        bcc: parse_all(bcc)?,
        subject,
        files: Vec::new(),
        draft: false,
    })
}

pub fn reply(
    message: &Message,
    me: &Identities,
    choice: &Choice,
    all: bool,
    forward: bool,
    text: &str,
    now: Timestamp,
) -> Result<Outgoing> {
    let text = text_of(text, choice.draft)?;
    let from = sender(choice, me, answer_from(message, me))
        .with_context(|| fill(t().reply_no_sender, &[&message.account]))?;
    let mut seen = BTreeSet::new();
    let to = if choice.to.is_empty() && !forward {
        &message.answer_to
    } else {
        &choice.to
    };
    let to = recipients(to, &mut seen);
    if to.is_empty() && !choice.draft {
        bail!(
            "{}",
            if forward {
                t().send_no_recipient
            } else {
                t().reply_no_recipient
            }
        );
    }
    let cc = match (choice.cc.is_empty(), all) {
        (false, _) => recipients(&choice.cc, &mut seen),
        (true, true) => recipients(&message.answer_cc, &mut seen),
        (true, false) => Vec::new(),
    };
    let bcc = recipients(&choice.bcc, &mut seen);
    let mut outgoing = build(
        Parties { from, to, cc, bcc },
        subject_of(choice).unwrap_or_else(|| {
            if forward {
                forward_subject(&message.subject)
            } else {
                answer_subject(&message.subject)
            }
        }),
        format!("{text}\n"),
        now,
    );
    if let Some(parent) = &message.message_id {
        outgoing.references = message.references.clone();
        outgoing.references.push(parent.clone());
        outgoing.in_reply_to = Some(parent.clone());
    }
    Ok(outgoing)
}

pub fn new_mail(
    me: &Identities,
    account: &str,
    choice: &Choice,
    text: &str,
    now: Timestamp,
) -> Result<Outgoing> {
    let text = text_of(text, choice.draft)?;
    let from = sender(choice, me, account_from(me))
        .with_context(|| fill(t().reply_no_sender, &[&account]))?;
    let mut seen = BTreeSet::new();
    let to = recipients(&choice.to, &mut seen);
    if to.is_empty() && !choice.draft {
        bail!("{}", t().send_no_recipient);
    }
    let cc = recipients(&choice.cc, &mut seen);
    let bcc = recipients(&choice.bcc, &mut seen);
    Ok(build(
        Parties { from, to, cc, bcc },
        subject_of(choice).unwrap_or_default(),
        format!("{text}\n"),
        now,
    ))
}

pub fn answer_from(message: &Message, me: &Identities) -> Option<Addr> {
    let seen: Vec<&Addr> = if message.own {
        message.from.iter().collect()
    } else {
        message.to.iter().chain(&message.cc).collect()
    };
    let address = seen
        .iter()
        .filter_map(|addr| addr.address.as_deref())
        .find(|address| me.addresses.contains(&address.to_lowercase()))
        .or(me.primary.as_deref())
        .filter(|address| smtp::is_address(address))?;
    Some(named(address, me, &seen))
}

pub fn account_from(me: &Identities) -> Option<Addr> {
    me.primary
        .as_deref()
        .filter(|address| smtp::is_address(address))
        .map(|address| named(address, me, &[]))
}

pub fn answer_subject(original: &str) -> String {
    prefixed(original, "Re:", &REPLY_PREFIXES)
}

pub fn forward_subject(original: &str) -> String {
    prefixed(original, "Fwd:", &FORWARD_PREFIXES)
}

fn prefixed(original: &str, prefix: &str, known: &[&str]) -> String {
    let original = clean(Some(original)).unwrap_or_default();
    let lower = original.to_lowercase();
    if known.iter().any(|known| lower.starts_with(known)) {
        original
    } else {
        format!("{prefix} {original}").trim_end().to_string()
    }
}

pub fn parse_address(text: &str) -> Result<Addr> {
    let text = text.trim();
    let (name, address) = match text
        .strip_suffix('>')
        .and_then(|rest| rest.rsplit_once('<'))
    {
        Some((name, address)) => (clean(Some(name.trim().trim_matches('"'))), address.trim()),
        None => (None, text),
    };
    if !smtp::is_address(address) {
        bail!("{}", fill(t().send_not_address, &[&text]));
    }
    Ok(Addr {
        name,
        address: Some(address.to_string()),
    })
}

fn text_of(text: &str, draft: bool) -> Result<String> {
    let text = text.replace("\r\n", "\n");
    let text = text.trim_end();
    if text.trim().is_empty() && !draft {
        bail!("{}", t().reply_empty);
    }
    Ok(text.to_string())
}

fn build(parties: Parties, subject: String, body: String, now: Timestamp) -> Outgoing {
    let Parties { from, to, cc, bcc } = parties;
    Outgoing {
        message_id: message_id(from.address.as_deref().unwrap_or_default(), now),
        date: message::format_local(now.as_second()),
        from,
        to,
        cc,
        bcc,
        subject,
        in_reply_to: None,
        references: Vec::new(),
        body,
        originals: Vec::new(),
        files: Vec::new(),
    }
}

fn sender(choice: &Choice, me: &Identities, default: Option<Addr>) -> Option<Addr> {
    match &choice.from {
        Some(from) if from.name.is_some() => Some(from.clone()),
        Some(from) => from
            .address
            .as_deref()
            .map(|address| named(address, me, &[])),
        None => default,
    }
}

fn named(address: &str, me: &Identities, seen: &[&Addr]) -> Addr {
    let name_of = |wanted: &str| {
        me.senders
            .iter()
            .chain(seen.iter().copied())
            .filter(|addr| {
                addr.address
                    .as_deref()
                    .is_some_and(|known| known.eq_ignore_ascii_case(wanted))
            })
            .find_map(|addr| clean(addr.name.as_deref()))
    };
    let name = name_of(address).or_else(|| me.primary.as_deref().and_then(name_of));
    Addr {
        name,
        address: Some(address.to_string()),
    }
}

fn recipients(addrs: &[Addr], seen: &mut BTreeSet<String>) -> Vec<Addr> {
    addrs
        .iter()
        .filter_map(|addr| {
            let address = addr.address.as_deref()?.trim();
            (smtp::is_address(address) && seen.insert(address.to_lowercase())).then(|| Addr {
                name: clean(addr.name.as_deref()),
                address: Some(address.to_string()),
            })
        })
        .collect()
}

fn subject_of(choice: &Choice) -> Option<String> {
    choice
        .subject
        .as_deref()
        .map(|subject| clean(Some(subject)).unwrap_or_default())
}

fn clean(text: Option<&str>) -> Option<String> {
    let text: String = text?
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty()).then_some(text)
}

fn message_id(from: &str, now: Timestamp) -> String {
    let random = || {
        let mut hasher = RandomState::new().build_hasher();
        hasher.write_i128(now.as_nanosecond());
        hasher.write_u32(std::process::id());
        hasher.finish()
    };
    let domain = from
        .rsplit_once('@')
        .map_or("localhost", |(_, domain)| domain);
    format!("{:016x}{:016x}@{domain}", random(), random())
}

pub fn quote(message: &Message) -> String {
    let quoted = message.body.trim_end();
    if quoted.trim().is_empty() {
        return String::new();
    }
    let mut quote = attribution(message);
    quote.push('\n');
    for line in quoted.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            quote.push('>');
        } else {
            quote.push_str("> ");
            quote.push_str(line);
        }
        quote.push('\n');
    }
    quote
}

fn attribution(message: &Message) -> String {
    let stamp = message.date.as_deref().unwrap_or(&message.received);
    let local = stamp.parse::<Timestamp>().map_or_else(
        |_| stamp.to_string(),
        |time| message::format_local(time.as_second()),
    );
    let minute = message::minute(&local);
    let (day, clock) = minute.split_once(' ').unwrap_or((&minute, ""));
    let author = message
        .from
        .first()
        .and_then(|addr| clean(addr.name.as_deref()).or_else(|| clean(addr.address.as_deref())))
        .unwrap_or_else(|| t().show_unnamed.to_string());
    fill(t().reply_attribution, &[&day, &clock, &author])
}

pub fn content_type(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_lowercase();
    match extension.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "txt" | "log" => "text/plain",
        "md" => "text/markdown",
        "csv" => "text/csv",
        "html" | "htm" => "text/html",
        "ics" => "text/calendar",
        "vcf" => "text/vcard",
        "json" => "application/json",
        "xml" => "application/xml",
        "zip" => "application/zip",
        "gz" => "application/gzip",
        "tar" => "application/x-tar",
        "eml" => "message/rfc822",
        "doc" => "application/msword",
        "xls" => "application/vnd.ms-excel",
        "ppt" => "application/vnd.ms-powerpoint",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "odt" => "application/vnd.oasis.opendocument.text",
        "ods" => "application/vnd.oasis.opendocument.spreadsheet",
        "odp" => "application/vnd.oasis.opendocument.presentation",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
}

fn address(addr: &Addr) -> Address<'static> {
    Address::new_address(addr.name.clone(), addr.address.clone().unwrap_or_default())
}

fn list(addrs: &[Addr]) -> Address<'static> {
    Address::new_list(addrs.iter().map(address).collect())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use jiff::tz::TimeZone;
    use mail_parser::MessageParser;

    use super::*;
    use crate::list::Located;
    use crate::message::{Flags, Meta, convert};
    use crate::show;
    use crate::style::{self, Color};

    const NOW: i64 = 1_790_000_000;

    fn addr(name: Option<&str>, address: &str) -> Addr {
        Addr {
            name: name.map(str::to_string),
            address: Some(address.to_string()),
        }
    }

    fn me() -> Identities {
        Identities {
            addresses: BTreeSet::from([
                "me@example.com".to_string(),
                "post@example.com".to_string(),
            ]),
            primary: Some("me@example.com".to_string()),
            senders: vec![
                addr(Some("Robin Example"), "Me@example.com"),
                addr(None, "post@example.com"),
            ],
        }
    }

    fn original(folder: &str, edit: impl FnOnce(&mut crate::message::MailFile)) -> Message {
        let mut file = convert(
            include_bytes!("../tests/fixtures/plain.eml"),
            Meta {
                account: "private",
                folder: "INBOX",
                uid: 7,
                flags: Flags::default(),
                received: 1_757_939_525,
                size: 1,
            },
        );
        edit(&mut file);
        let kind = crate::kind::parse(folder).ok().flatten();
        let located = Located {
            account: "private".to_string(),
            color: Color::Blue,
            folder: folder.to_string(),
            style: style::default_for(kind),
            kind,
            file: PathBuf::from("/sync/private").join(folder).join("x-7.json"),
            path: format!("private/{folder}/x-7.json"),
            time: "2026-09-15_1432".to_string(),
            uid: 7,
        };
        show::message(file, located, &me().addresses)
    }

    fn now() -> Timestamp {
        Timestamp::from_second(NOW).unwrap()
    }

    fn reply_to(message: &Message, all: bool) -> Outgoing {
        let text = "Thanks, Jane.\r\n\r\n";
        reply(message, &me(), &Choice::default(), all, false, text, now()).unwrap()
    }

    fn header(raw: &[u8], name: &str) -> String {
        let parsed = MessageParser::new().parse(raw).unwrap();
        parsed
            .header_raw(name)
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    #[test]
    fn a_reply_answers_the_sender_from_the_address_it_was_sent_to() {
        let outgoing = reply_to(&original("inbox", |_| {}), false);
        assert_eq!(outgoing.sender(), "me@example.com");
        assert_eq!(outgoing.from.name.as_deref(), Some("Robin Example"));
        assert_eq!(outgoing.recipients(), ["jane@example.com"]);
        assert_eq!(outgoing.subject, "Re: Offer for project");
        let (local, domain) = outgoing.message_id.split_once('@').unwrap();
        assert_eq!(local.len(), 32);
        assert!(local.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(domain, "example.com");
        assert_ne!(
            reply_to(&original("inbox", |_| {}), false).message_id,
            outgoing.message_id
        );

        let raw = &outgoing.raw(&[]).unwrap();
        let parsed = MessageParser::new().parse(raw).unwrap();
        assert_eq!(parsed.subject(), Some("Re: Offer for project"));
        assert_eq!(parsed.in_reply_to().as_text(), Some("a1b2c3@example.com"));
        assert_eq!(
            header(raw, "References"),
            "<root@example.com> <x9y8@example.com> <a1b2c3@example.com>"
        );
        assert_eq!(header(raw, "From"), "\"Robin Example\" <me@example.com>");
        assert_eq!(header(raw, "To"), "\"Jane Doe\" <jane@example.com>");
        assert!(parsed.header("Cc").is_none());
        assert_eq!(parsed.message_id(), Some(outgoing.message_id.as_str()));
        assert_eq!(parsed.date().unwrap().to_timestamp(), NOW);

        let body = parsed.body_text(0).unwrap().replace("\r\n", "\n");
        assert_eq!(body, "Thanks, Jane.\n");
        let quote = quote(&original("inbox", |_| {}));
        let quoted = "\n> Hi,\n>\n> please find the offer attached.\n>\n> Jane\n";
        assert!(quote.ends_with(quoted), "{quote}");
        let parts = crate::reply::parts(&quote);
        let crate::reply::Part::Quote {
            attribution: Some(attribution),
            parts: quoted,
        } = &parts[0]
        else {
            panic!("{parts:?}");
        };
        let sent: Timestamp = "2026-09-15T14:32:00+02:00".parse().unwrap();
        let day = sent
            .to_zoned(TimeZone::system())
            .strftime("%Y-%m-%d")
            .to_string();
        assert!(attribution.contains(&day), "{attribution}");
        assert!(attribution.contains("Jane Doe"), "{attribution}");
        assert_eq!(quoted.len(), 1);
    }

    #[test]
    fn reply_all_copies_the_others_and_aliases_answer_as_themselves() {
        let message = original("inbox", |file| {
            file.to = vec![addr(Some("Robin"), "post@example.com")];
            file.cc = vec![
                addr(Some("Paul"), "paul@example.com"),
                addr(None, "me@example.com"),
            ];
            file.subject = "AW: Termin".to_string();
        });
        let single = reply_to(&message, false);
        assert!(single.cc.is_empty());
        assert_eq!(single.subject, "AW: Termin");
        let all = reply_to(&message, true);
        assert_eq!(all.sender(), "post@example.com");
        assert_eq!(all.from.name.as_deref(), Some("Robin"));
        assert_eq!(all.recipients(), ["jane@example.com", "paul@example.com"]);
        assert_eq!(
            header(&all.raw(&[]).unwrap(), "Cc"),
            "\"Paul\" <paul@example.com>"
        );
    }

    #[test]
    fn a_reply_to_own_mail_goes_to_its_recipients() {
        let message = original("sent", |file| {
            file.from = vec![addr(Some("Robin"), "post@example.com")];
            file.to = vec![addr(Some("Tom"), "tom@example.com")];
            file.message_id = None;
            file.references.clear();
        });
        let outgoing = reply_to(&message, false);
        assert_eq!(outgoing.sender(), "post@example.com");
        assert_eq!(outgoing.recipients(), ["tom@example.com"]);
        assert!(
            MessageParser::new()
                .parse(&outgoing.raw(&[]).unwrap())
                .unwrap()
                .header("In-Reply-To")
                .is_none()
        );
    }

    #[test]
    fn hostile_headers_are_cleaned_and_bad_addresses_dropped() {
        let message = original("inbox", |file| {
            file.from = vec![addr(Some("Eve\r\nBcc: x@example.com"), "eve@example.com")];
            file.reply_to = vec![
                addr(None, "eve@example.com>\r\nRCPT TO:<x@example.com"),
                addr(None, "eve@example.com"),
            ];
            file.subject = "Hi\nBcc: y@example.com".to_string();
        });
        let outgoing = reply_to(&message, false);
        assert_eq!(outgoing.recipients(), ["eve@example.com"]);
        assert_eq!(outgoing.subject, "Re: Hi Bcc: y@example.com");
        let raw = outgoing.raw(&[]).unwrap();
        let parsed = MessageParser::new().parse(&raw).unwrap();
        assert!(parsed.header("Bcc").is_none());
    }

    #[test]
    fn nothing_is_built_without_text_recipient_or_own_address() {
        let message = original("inbox", |_| {});
        let chosen = Choice::default();
        let err = reply(&message, &me(), &chosen, false, false, " \n\n", now())
            .err()
            .unwrap();
        assert_eq!(err.to_string(), t().reply_empty);
        let nobody = original("inbox", |file| file.from.clear());
        assert!(reply(&nobody, &me(), &chosen, false, false, "Hi", now()).is_err());
        let stranger = Identities {
            addresses: BTreeSet::new(),
            primary: None,
            senders: Vec::new(),
        };
        let err = reply(&message, &stranger, &chosen, false, false, "Hi", now())
            .err()
            .unwrap();
        assert_eq!(err.to_string(), fill(t().reply_no_sender, &[&"private"]));
    }

    #[test]
    fn a_new_mail_goes_from_the_account_to_the_chosen_people() {
        let people = choice(
            None,
            &[
                "Jane Doe <jane@example.com>".into(),
                "paul@example.com".into(),
            ],
            &[
                "\"Maria\" <maria@example.com>".into(),
                "PAUL@example.com".into(),
            ],
            &[],
            Some(" Lunch\n".into()),
        )
        .unwrap();
        let outgoing = new_mail(&me(), "private", &people, "Tuesday?\r\n", now()).unwrap();
        assert_eq!(outgoing.sender(), "me@example.com");
        assert_eq!(outgoing.from.name.as_deref(), Some("Robin Example"));
        assert_eq!(
            outgoing.recipients(),
            ["jane@example.com", "paul@example.com", "maria@example.com"]
        );
        assert_eq!(outgoing.subject, "Lunch");
        let raw = &outgoing.raw(&[]).unwrap();
        let parsed = MessageParser::new().parse(raw).unwrap();
        assert_eq!(
            header(raw, "To"),
            "\"Jane Doe\" <jane@example.com>, <paul@example.com>"
        );
        assert_eq!(header(raw, "Cc"), "\"Maria\" <maria@example.com>");
        assert!(parsed.header("In-Reply-To").is_none());
        assert!(parsed.header("References").is_none());
        assert_eq!(
            parsed.body_text(0).unwrap().replace("\r\n", "\n"),
            "Tuesday?\n"
        );
        assert_eq!(outgoing.copy(&[]).unwrap(), outgoing.raw(&[]).unwrap());

        let nobody = Choice::default();
        let err = new_mail(&me(), "private", &nobody, "Hi", now())
            .err()
            .unwrap();
        assert_eq!(err.to_string(), t().send_no_recipient);
        let stranger = Identities {
            addresses: BTreeSet::new(),
            primary: Some("robin".to_string()),
            senders: Vec::new(),
        };
        let err = new_mail(&stranger, "private", &people, "Hi", now())
            .err()
            .unwrap();
        assert_eq!(err.to_string(), fill(t().reply_no_sender, &[&"private"]));
    }

    #[test]
    fn blind_copies_go_out_unseen_and_stay_in_the_copy() {
        let people = choice(
            None,
            &["jane@example.com".into()],
            &[],
            &["Tom <tom@example.com>".into(), "JANE@example.com".into()],
            None,
        )
        .unwrap();
        let outgoing = new_mail(&me(), "private", &people, "Hi", now()).unwrap();
        assert_eq!(
            outgoing.recipients(),
            ["jane@example.com", "tom@example.com"]
        );
        assert_eq!(outgoing.subject, "");
        let raw = outgoing.raw(&[]).unwrap();
        let copy = outgoing.copy(&[]).unwrap();
        let sent = MessageParser::new().parse(&raw).unwrap();
        assert!(sent.header("Bcc").is_none());
        assert_eq!(header(&copy, "Bcc"), "\"Tom\" <tom@example.com>");
        let kept = MessageParser::new().parse(&copy).unwrap();
        assert_eq!(kept.message_id(), sent.message_id());
    }

    #[test]
    fn the_sender_can_be_any_address_and_keeps_the_own_name() {
        let alias = |from: &str| choice(Some(from), &["jane@example.com".into()], &[], &[], None);
        let outgoing = new_mail(
            &me(),
            "private",
            &alias("test2@example.com").unwrap(),
            "Hi",
            now(),
        )
        .unwrap();
        assert_eq!(outgoing.sender(), "test2@example.com");
        assert_eq!(outgoing.from.name.as_deref(), Some("Robin Example"));
        assert!(outgoing.message_id.ends_with("@example.com"));
        let named = alias("Shop <shop@example.com>").unwrap();
        let outgoing = new_mail(&me(), "private", &named, "Hi", now()).unwrap();
        assert_eq!(
            header(&outgoing.raw(&[]).unwrap(), "From"),
            "\"Shop\" <shop@example.com>"
        );
        assert!(alias("robin").is_err());

        let message = original("inbox", |_| {});
        let outgoing = reply(
            &message,
            &me(),
            &alias("post@example.com").unwrap(),
            false,
            false,
            "Hi",
            now(),
        )
        .unwrap();
        assert_eq!(outgoing.sender(), "post@example.com");
        assert_eq!(outgoing.from.name.as_deref(), Some("Robin Example"));
    }

    #[test]
    fn a_reply_goes_to_the_chosen_people_with_the_chosen_subject() {
        let message = original("inbox", |file| {
            file.cc = vec![addr(Some("Paul"), "paul@example.com")];
        });
        let people = choice(
            None,
            &["maria@example.com".into()],
            &["Jane Doe <jane@example.com>".into()],
            &["tom@example.com".into()],
            Some("Neues Angebot".into()),
        )
        .unwrap();
        let outgoing = reply(&message, &me(), &people, true, false, "Hi", now()).unwrap();
        assert_eq!(
            outgoing.recipients(),
            ["maria@example.com", "jane@example.com", "tom@example.com"]
        );
        assert_eq!(outgoing.subject, "Neues Angebot");
        let raw = outgoing.raw(&[]).unwrap();
        let parsed = MessageParser::new().parse(&raw).unwrap();
        assert_eq!(parsed.in_reply_to().as_text(), Some("a1b2c3@example.com"));
        let hidden = Choice {
            bcc: vec![addr(None, "tom@example.com")],
            ..Choice::default()
        };
        let outgoing = reply(&message, &me(), &hidden, true, false, "Hi", now()).unwrap();
        assert_eq!(
            outgoing.recipients(),
            ["jane@example.com", "paul@example.com", "tom@example.com"]
        );
        assert_eq!(outgoing.subject, "Re: Offer for project");
        let copied = Choice {
            cc: vec![addr(None, "maria@example.com")],
            subject: Some(" \n".to_string()),
            ..Choice::default()
        };
        let outgoing = reply(&message, &me(), &copied, false, false, "Hi", now()).unwrap();
        assert_eq!(
            outgoing.recipients(),
            ["jane@example.com", "maria@example.com"]
        );
        assert_eq!(outgoing.subject, "");
    }

    #[test]
    fn addresses_are_plain_or_with_a_name() {
        let named = parse_address(" Jane Doe <jane@example.com> ").unwrap();
        assert_eq!(named.name.as_deref(), Some("Jane Doe"));
        assert_eq!(named.address.as_deref(), Some("jane@example.com"));
        let plain = parse_address("jane@example.com").unwrap();
        assert_eq!(plain.name, None);
        for bad in [
            "jane",
            "jane@example.com>\r\nRCPT TO:<x@example.com",
            "a b@example.com",
        ] {
            let err = parse_address(bad).err().unwrap();
            assert_eq!(err.to_string(), fill(t().send_not_address, &[&bad.trim()]));
        }
    }

    #[test]
    fn subjects_get_one_prefix() {
        assert_eq!(answer_subject("Offer"), "Re: Offer");
        assert_eq!(answer_subject("RE: Offer"), "RE: Offer");
        assert_eq!(answer_subject("Aw: Angebot"), "Aw: Angebot");
        assert_eq!(answer_subject(""), "Re:");
        assert_eq!(answer_subject("  Rechnung \t 2026 "), "Re: Rechnung 2026");
        assert_eq!(forward_subject("Re: Offer"), "Fwd: Re: Offer");
        assert_eq!(forward_subject("FW: Offer"), "FW: Offer");
        assert_eq!(forward_subject("WG: Angebot"), "WG: Angebot");
        assert_eq!(forward_subject(""), "Fwd:");
    }

    #[test]
    fn a_file_is_typed_by_its_extension() {
        assert_eq!(content_type(Path::new("/tmp/Offer.PDF")), "application/pdf");
        assert_eq!(
            content_type(Path::new("plan.xlsx")),
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        );
        assert_eq!(content_type(Path::new("notes")), "application/octet-stream");
        assert_eq!(
            content_type(Path::new("archive.7z")),
            "application/octet-stream"
        );
    }
}
