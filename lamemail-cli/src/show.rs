use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::Path;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::i18n::{fill, t};
use crate::kind::{self, Kind};
use crate::list::{self, Located};
use crate::message::{self, Addr, Attachment, BodySource, Flags, MailFile};
use crate::settings::Settings;
use crate::style::{Color, Style};
use crate::{compose, journal, output, reply, settings, store};

#[derive(Debug, Serialize)]
pub struct Message {
    pub account: String,
    pub color: Color,
    pub folder: String,
    pub style: Style,
    pub kind: Option<Kind>,
    pub uid: u32,
    pub path: String,
    pub own: bool,
    pub received: String,
    pub date: Option<String>,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub from: Vec<Addr>,
    pub reply_to: Vec<Addr>,
    pub to: Vec<Addr>,
    pub cc: Vec<Addr>,
    pub bcc: Vec<Addr>,
    pub subject: String,
    pub flags: Flags,
    pub forwarded: bool,
    pub size: u64,
    pub body: String,
    pub body_source: BodySource,
    pub text: String,
    pub parts: Vec<reply::Part>,
    pub answer_to: Vec<Addr>,
    pub answer_cc: Vec<Addr>,
    pub attachments: Vec<Attachment>,
}

#[derive(Serialize)]
struct Shown<'a> {
    #[serde(flatten)]
    message: &'a Message,
    answer_from: Option<Addr>,
    answer_subject: String,
    forward_subject: String,
    quote: String,
    signature: Option<&'a str>,
    parent: Option<String>,
}

impl<'a> Shown<'a> {
    fn of(
        message: &'a Message,
        me: &Identities,
        signature: Option<&'a str>,
        parent: Option<String>,
    ) -> Self {
        Shown {
            answer_from: compose::answer_from(message, me),
            answer_subject: compose::answer_subject(&message.subject),
            forward_subject: compose::forward_subject(&message.subject),
            quote: compose::quote(message),
            signature,
            parent,
            message,
        }
    }
}

pub type Mails = Vec<(Located, MailFile)>;

pub struct Identities {
    pub addresses: BTreeSet<String>,
    pub primary: Option<String>,
    pub senders: Vec<Addr>,
}

#[derive(Deserialize)]
struct FromOnly {
    from: Vec<Addr>,
}

#[derive(Deserialize)]
pub struct IdOnly {
    pub message_id: Option<String>,
}

pub fn run(dir: &Path, path: &Path, json: bool) -> Result<()> {
    let settings = settings::load(dir)?;
    let (message, me) = open(dir, &settings, path)?;
    if !json {
        return output::stdout(|out| write_text(out, &message));
    }
    let signature = settings
        .accounts
        .get(&message.account)
        .and_then(|account| account.signature.as_deref());
    let parent = parent(dir, &settings, &message)?;
    output::stdout(|out| output::json_line(out, &Shown::of(&message, &me, signature, parent)))
}

fn parent(dir: &Path, settings: &Settings, message: &Message) -> Result<Option<String>> {
    let (Some(Kind::Drafts), Some(wanted)) = (message.kind, message.in_reply_to.as_deref()) else {
        return Ok(None);
    };
    let overlay = journal::overlay(dir)?;
    Ok(list::locate_account(dir, settings, &message.account)?
        .into_iter()
        .filter(|located| !overlay.hides(&located.path))
        .filter(|located| {
            store::read_or_skip(&located.file)
                .is_some_and(|only: IdOnly| only.message_id.as_deref() == Some(wanted))
        })
        .map(|mut located| {
            overlay.relocate(&mut located, settings);
            located
        })
        .min_by_key(|located| kind::rank(false, located.kind))
        .map(|located| located.path))
}

pub fn open(dir: &Path, settings: &Settings, path: &Path) -> Result<(Message, Identities)> {
    let mut located = list::locate_one(dir, settings, path)?;
    let overlay = journal::overlay(dir)?;
    if overlay.hides(&located.path) {
        bail!(
            "{}",
            fill(t().show_not_message, &[&path.display(), &dir.display()])
        );
    }
    let mut file: MailFile = store::read(&located.file)?;
    overlay.apply(&located.path, &mut file.flags);
    overlay.relocate(&mut located, settings);
    let me = identities(dir, settings, &located.account)?;
    Ok((message(file, located, &me.addresses), me))
}

pub fn message(file: MailFile, located: Located, mine: &BTreeSet<String>) -> Message {
    let own = is_own(&file, located.kind, mine);
    let answer_to = answer_to(&file, own);
    Message {
        answer_cc: answer_cc(&file, &answer_to, mine),
        answer_to,
        own,
        text: reply::text(&file.body),
        parts: reply::parts(&file.body),
        account: located.account,
        color: located.color,
        folder: located.folder,
        style: located.style,
        kind: located.kind,
        uid: file.uid,
        path: located.path,
        received: file.received,
        date: file.date,
        message_id: file.message_id,
        in_reply_to: file.in_reply_to,
        references: file.references,
        from: file.from,
        reply_to: file.reply_to,
        to: file.to,
        cc: file.cc,
        bcc: file.bcc,
        subject: file.subject,
        forwarded: file.flags.has(message::FORWARDED),
        flags: file.flags,
        size: file.size,
        body: file.body,
        body_source: file.body_source,
        attachments: file.attachments,
    }
}

pub fn read_account(
    dir: &Path,
    settings: &Settings,
    id: &str,
) -> Result<(Mails, BTreeSet<String>)> {
    let overlay = journal::overlay(dir)?;
    let read: Mails = list::locate_account(dir, settings, id)?
        .into_iter()
        .filter_map(|mut located| {
            if overlay.hides(&located.path) {
                return None;
            }
            let mut file: MailFile = store::read_or_skip(&located.file)?;
            overlay.apply(&located.path, &mut file.flags);
            overlay.relocate(&mut located, settings);
            Some((located, file))
        })
        .collect();
    let senders = read
        .iter()
        .filter(|(located, _)| is_own_folder(located.kind))
        .flat_map(|(_, file)| file.from.iter().cloned());
    let mine = Identities::of(settings.login(id), senders).addresses;
    Ok((read, mine))
}

pub fn identities(dir: &Path, settings: &Settings, id: &str) -> Result<Identities> {
    let mut own: Vec<Located> = list::locate_account(dir, settings, id)?
        .into_iter()
        .filter(|located| is_own_folder(located.kind))
        .collect();
    own.sort_by(|a, b| b.time.cmp(&a.time).then(b.uid.cmp(&a.uid)));
    let senders = own
        .iter()
        .filter_map(|located| store::read_or_skip(&located.file))
        .flat_map(|only: FromOnly| only.from);
    Ok(Identities::of(settings.login(id), senders))
}

impl Identities {
    pub fn of(
        login: Option<&str>,
        senders_newest_first: impl IntoIterator<Item = Addr>,
    ) -> Identities {
        let mut seen = BTreeSet::new();
        let senders: Vec<Addr> = senders_newest_first
            .into_iter()
            .filter(|sender| {
                sender
                    .address
                    .as_deref()
                    .is_some_and(|address| seen.insert(address.to_lowercase()))
            })
            .collect();
        let login = login.filter(|login| login.contains('@'));
        let primary = login
            .map(str::to_string)
            .or_else(|| senders.first().and_then(|sender| sender.address.clone()));
        let mut addresses: BTreeSet<String> = login.map(str::to_lowercase).into_iter().collect();
        addresses.extend(seen);
        Identities {
            addresses,
            primary,
            senders,
        }
    }
}

pub fn is_own_folder(kind: Option<Kind>) -> bool {
    matches!(kind, Some(Kind::Sent | Kind::Drafts))
}

pub fn addresses(addrs: &[Addr]) -> impl Iterator<Item = String> + '_ {
    addrs
        .iter()
        .filter_map(|addr| addr.address.as_deref())
        .map(str::to_lowercase)
}

fn answer_to(file: &MailFile, own: bool) -> Vec<Addr> {
    if own {
        file.to.clone()
    } else if file.reply_to.is_empty() {
        file.from.clone()
    } else {
        file.reply_to.clone()
    }
}

fn answer_cc(file: &MailFile, answer_to: &[Addr], mine: &BTreeSet<String>) -> Vec<Addr> {
    let mut taken: BTreeSet<String> = addresses(answer_to).chain(mine.iter().cloned()).collect();
    file.to
        .iter()
        .chain(&file.cc)
        .filter(|addr| {
            addr.address
                .as_deref()
                .is_some_and(|address| taken.insert(address.to_lowercase()))
        })
        .cloned()
        .collect()
}

pub fn is_own(file: &MailFile, kind: Option<Kind>, mine: &BTreeSet<String>) -> bool {
    is_own_folder(kind)
        || file.flags.draft
        || addresses(&file.from).any(|address| mine.contains(&address))
}

pub fn write_text(out: &mut dyn Write, message: &Message) -> io::Result<()> {
    let texts = t();
    let labels = [
        texts.show_from,
        texts.show_to,
        texts.show_cc,
        texts.show_date,
        texts.show_subject,
        texts.show_account,
        texts.show_folder,
        texts.show_attachments,
    ];
    let width = labels
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or_default()
        + 2;
    header(out, width, texts.show_from, &people(&message.from))?;
    header(out, width, texts.show_to, &people(&message.to))?;
    if !message.cc.is_empty() {
        header(out, width, texts.show_cc, &people(&message.cc))?;
    }
    header(
        out,
        width,
        texts.show_date,
        &message::minute(&message.received),
    )?;
    header(out, width, texts.show_subject, &message.subject)?;
    header(out, width, texts.show_account, &message.account)?;
    header(out, width, texts.show_folder, &message.folder)?;
    for (index, attachment) in message.attachments.iter().enumerate() {
        let label = if index == 0 {
            texts.show_attachments
        } else {
            ""
        };
        header(out, width, label, &attachment_text(attachment))?;
    }
    writeln!(out)?;
    if message.body_source == BodySource::None {
        writeln!(out, "{}", texts.show_no_text)?;
    } else {
        write!(out, "{}", message.body)?;
        if !message.body.ends_with('\n') {
            writeln!(out)?;
        }
    }
    Ok(())
}

fn header(out: &mut dyn Write, width: usize, label: &str, value: &str) -> io::Result<()> {
    let label = if label.is_empty() {
        String::new()
    } else {
        format!("{label}:")
    };
    writeln!(out, "{label:<width$}{value}")
}

pub fn people(addrs: &[Addr]) -> String {
    addrs.iter().map(person).collect::<Vec<_>>().join(", ")
}

fn person(addr: &Addr) -> String {
    let name = addr
        .name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty());
    let address = addr
        .address
        .as_deref()
        .map(str::trim)
        .filter(|a| !a.is_empty());
    match (name, address) {
        (Some(name), Some(address)) => format!("{name} <{address}>"),
        (Some(name), None) => name.to_string(),
        (None, Some(address)) => address.to_string(),
        (None, None) => String::new(),
    }
}

fn attachment_text(attachment: &Attachment) -> String {
    let name = attachment.name.as_deref().unwrap_or(t().show_unnamed);
    format!(
        "{name} ({}, {})",
        attachment.content_type,
        size_text(attachment.size)
    )
}

fn size_text(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * KB;
    if bytes < KB {
        format!("{bytes} B")
    } else if bytes < 10 * MB {
        format!("{} KB", bytes.div_ceil(KB))
    } else {
        format!("{} MB", bytes.div_ceil(MB))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::{Meta, convert};
    use crate::testing::Fixture;
    use crate::{kind, style};
    use std::fs;
    use std::path::PathBuf;

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

    fn located(path: &str, folder: &str) -> Located {
        let kind = kind::parse(folder).ok().flatten();
        Located {
            account: "private".to_string(),
            color: Color::Cyan,
            folder: folder.to_string(),
            style: style::default_for(kind),
            kind,
            file: PathBuf::from("/sync").join(path),
            path: path.to_string(),
            time: "2025-09-15_1432".to_string(),
            uid: 4711,
        }
    }

    fn shown() -> Message {
        message(
            sample(),
            located(
                "private/inbox/2025-09-15_1432-offer-for-project-4711.json",
                "inbox",
            ),
            &BTreeSet::new(),
        )
    }

    #[test]
    fn json_keeps_the_documented_fields() {
        let json = serde_json::to_value(shown()).unwrap();
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
            "in_reply_to",
            "references",
            "from",
            "reply_to",
            "to",
            "cc",
            "bcc",
            "subject",
            "flags",
            "size",
            "own",
            "body",
            "body_source",
            "text",
            "parts",
            "answer_to",
            "answer_cc",
            "attachments",
        ] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
        assert_eq!(json["color"], "cyan");
        assert_eq!(json["folder"], "inbox");
        assert_eq!(
            json["path"],
            "private/inbox/2025-09-15_1432-offer-for-project-4711.json"
        );
        assert_eq!(json["style"]["icon"], "incoming");
        assert_eq!(json["kind"], "inbox");
        assert_eq!(json["body_source"], "html");
        assert_eq!(json["attachments"][0]["name"], "offer.pdf");
        assert_eq!(json["own"], false);
        assert_eq!(json["text"], "HTML version.");
        assert_eq!(json["parts"][0]["kind"], "text");
        assert_eq!(json["parts"][0]["text"], "HTML version.");
        assert_eq!(json["answer_to"][0]["address"], "jane@example.com");
        assert!(json.get("format").is_none());
    }

    #[test]
    fn json_carries_what_a_reply_starts_from() {
        let message = shown();
        let me = Identities::of(Some("me@example.com"), []);
        let json = serde_json::to_value(Shown::of(&message, &me, Some("Robin"), Some("p".into())))
            .unwrap();
        assert_eq!(json["answer_from"]["address"], "me@example.com");
        assert_eq!(json["signature"], "Robin");
        assert_eq!(json["parent"], "p");
        assert_eq!(json["answer_subject"], "Re: Angebot für Projekt");
        assert_eq!(json["forward_subject"], "Fwd: Angebot für Projekt");
        assert!(
            json["quote"]
                .as_str()
                .unwrap()
                .ends_with("\n> HTML version.\n")
        );
        assert_eq!(json["answer_to"][0]["address"], "jane@example.com");
        let stranger = Identities::of(None, []);
        let json = serde_json::to_value(Shown::of(&message, &stranger, None, None)).unwrap();
        assert!(json["answer_from"].is_null());
        assert!(json["signature"].is_null());
        assert!(json["parent"].is_null());
    }

    #[test]
    fn answers_go_to_reply_to_the_sender_or_for_own_mail_the_recipients() {
        let nobody = BTreeSet::new();
        let inbox = message(sample(), located("p", "inbox"), &nobody);
        assert_eq!(
            inbox.answer_to[0].address.as_deref(),
            Some("jane@example.com")
        );
        let mut asked = sample();
        asked.reply_to = vec![Addr {
            name: None,
            address: Some("offers@example.com".to_string()),
        }];
        let asked = message(asked, located("p", "inbox"), &nobody);
        assert_eq!(
            asked.answer_to[0].address.as_deref(),
            Some("offers@example.com")
        );
        let sent = message(sample(), located("p", "sent"), &nobody);
        assert_eq!(sent.answer_to[0].address.as_deref(), Some("me@example.com"));
    }

    #[test]
    fn a_reply_to_all_copies_the_others_but_not_me() {
        let addr = |address: &str| Addr {
            name: None,
            address: Some(address.to_string()),
        };
        let mut file = sample();
        file.to = vec![addr("me@example.com"), addr("paul@example.com")];
        file.cc = vec![
            addr("Jane@example.com"),
            addr("post@example.com"),
            addr("paul@example.com"),
        ];
        let mine = BTreeSet::from(["me@example.com".to_string(), "post@example.com".to_string()]);
        let inbox = message(file, located("p", "inbox"), &mine);
        let cc: Vec<_> = inbox
            .answer_cc
            .iter()
            .filter_map(|a| a.address.as_deref())
            .collect();
        assert_eq!(cc, ["paul@example.com"]);

        let mut file = sample();
        file.to = vec![addr("paul@example.com")];
        file.cc = vec![addr("maria@example.com"), addr("me@example.com")];
        let sent = message(file, located("p", "sent"), &mine);
        let to: Vec<_> = sent
            .answer_to
            .iter()
            .filter_map(|a| a.address.as_deref())
            .collect();
        let cc: Vec<_> = sent
            .answer_cc
            .iter()
            .filter_map(|a| a.address.as_deref())
            .collect();
        assert_eq!(to, ["paul@example.com"]);
        assert_eq!(cc, ["maria@example.com"]);
    }

    #[test]
    fn own_mail_is_sent_drafts_or_from_an_own_address() {
        let nobody = BTreeSet::new();
        assert!(message(sample(), located("p", "sent"), &nobody).own);
        let jane = Identities::of(Some("JANE@example.com"), []).addresses;
        assert!(message(sample(), located("p", "inbox"), &jane).own);
        let mut draft = sample();
        draft.flags.draft = true;
        assert!(message(draft, located("p", "inbox"), &nobody).own);
        let other = Identities::of(Some("me@example.com"), []).addresses;
        assert!(!message(sample(), located("p", "inbox"), &other).own);
        assert!(Identities::of(Some("jane"), []).addresses.is_empty());
    }

    #[test]
    fn text_has_labels_attachments_and_body() {
        let mut out = Vec::new();
        write_text(&mut out, &shown()).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with(&format!("{}:", t().show_from)), "{text}");
        assert!(text.contains("Angebot für Projekt"), "{text}");
        assert!(text.contains("offer.pdf (application/pdf, "), "{text}");
        assert!(text.contains("\n\nHTML version.\n"), "{text}");
        assert!(text.contains("2025-09-15 "), "{text}");
    }

    #[test]
    fn text_marks_a_missing_body() {
        let mut message = shown();
        message.body.clear();
        message.body_source = BodySource::None;
        message.attachments.clear();
        let mut out = Vec::new();
        write_text(&mut out, &message).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.ends_with(&format!("\n\n{}\n", t().show_no_text)),
            "{text}"
        );
    }

    #[test]
    fn people_are_name_and_address() {
        let addr = |name: Option<&str>, address: Option<&str>| Addr {
            name: name.map(str::to_string),
            address: address.map(str::to_string),
        };
        assert_eq!(
            people(&[
                addr(Some("Jane Doe"), Some("jane@example.com")),
                addr(None, Some("me@example.com")),
                addr(Some(" Only Name "), None),
            ]),
            "Jane Doe <jane@example.com>, me@example.com, Only Name"
        );
        assert_eq!(people(&[]), "");
    }

    #[test]
    fn sizes_are_rounded_up_to_whole_units() {
        assert_eq!(size_text(0), "0 B");
        assert_eq!(size_text(1023), "1023 B");
        assert_eq!(size_text(1024), "1 KB");
        assert_eq!(size_text(120_394), "118 KB");
        assert_eq!(size_text(9 * 1024 * 1024), "9216 KB");
        assert_eq!(size_text(25 * 1024 * 1024 + 1), "26 MB");
    }

    #[tokio::test]
    async fn a_draft_names_the_message_it_answers_its_best_copy() {
        let fixture = Fixture::new("show-parent");
        let original = include_bytes!("../tests/fixtures/plain.eml");
        fs::write(
            fixture.marked_folder("Archive", "archive").join("1,S.eml"),
            original,
        )
        .unwrap();
        let draft = |id: &str, parent: &str| {
            format!(
                "From: me@example.com\r\nTo: jane@example.com\r\nSubject: Re: Offer\r\n\
                 Message-ID: <{id}>\r\nIn-Reply-To: <{parent}>\r\n\r\nHalf done.\r\n"
            )
        };
        let drafts = fixture.marked_folder("Drafts", "drafts");
        fs::write(
            drafts.join("1,SD.eml"),
            draft("d1@example.com", "a1b2c3@example.com"),
        )
        .unwrap();
        fs::write(
            drafts.join("2,SD.eml"),
            draft("d2@example.com", "gone@example.com"),
        )
        .unwrap();
        fixture.sync().await;
        let dir = &fixture.dir;
        let settings = fixture.settings();
        let located = list::locate_account(dir, &settings, "private").unwrap();
        let path = |folder: &str, uid: u32| {
            located
                .iter()
                .find(|located| located.folder == folder && located.uid == uid)
                .unwrap()
                .path
                .clone()
        };
        let parent_of = |path: String| {
            let (message, _) = open(dir, &settings, Path::new(&path)).unwrap();
            parent(dir, &settings, &message).unwrap()
        };

        assert_eq!(parent_of(path("drafts", 1)), Some(path("inbox", 1)));
        assert_eq!(parent_of(path("drafts", 2)), None);
        assert_eq!(parent_of(path("inbox", 1)), None);
        fs::remove_dir_all(&fixture.base).unwrap();
    }
}
