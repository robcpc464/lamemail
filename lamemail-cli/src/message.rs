use jiff::Timestamp;
use jiff::tz::{Offset, TimeZone};
use mail_parser::{
    Address, HeaderValue, Message, MessageParser, MessagePart, MimeHeaders, PartType,
};
use serde::{Deserialize, Serialize};

use crate::html;

pub const FORMAT_VERSION: u32 = 1;
const MINUTE_CHARS: usize = 16;
const LINE_BREAK_TAGS: &[&str] = &["<br>", "<br/>", "<br />"];

#[derive(Debug, Serialize, Deserialize)]
pub struct MailFile {
    pub format: u32,
    pub account: String,
    pub folder: String,
    pub uid: u32,
    pub flags: Flags,
    pub received: String,
    pub date: Option<String>,
    pub size: u64,
    pub message_id: Option<String>,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    pub from: Vec<Addr>,
    pub reply_to: Vec<Addr>,
    pub to: Vec<Addr>,
    pub cc: Vec<Addr>,
    pub bcc: Vec<Addr>,
    pub subject: String,
    pub body: String,
    pub body_source: BodySource,
    pub attachments: Vec<Attachment>,
}

pub const FORWARDED: &str = "$Forwarded";
pub const JUNK: &str = "$Junk";
pub const NOT_JUNK: &str = "$NotJunk";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flags {
    pub seen: bool,
    pub answered: bool,
    pub flagged: bool,
    pub draft: bool,
    pub keywords: Vec<String>,
}

impl Flags {
    pub fn has(&self, keyword: &str) -> bool {
        self.keywords
            .iter()
            .any(|known| known.eq_ignore_ascii_case(keyword))
    }

    pub fn set(&mut self, keyword: &str, on: bool) {
        self.keywords
            .retain(|known| !known.eq_ignore_ascii_case(keyword));
        if on {
            self.keywords.push(keyword.to_string());
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Addr {
    pub name: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BodySource {
    Text,
    Html,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub index: u32,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub content_type: String,
    pub size: u64,
}

pub struct Meta<'a> {
    pub account: &'a str,
    pub folder: &'a str,
    pub uid: u32,
    pub flags: Flags,
    pub received: i64,
    pub size: u64,
}

pub fn convert(raw: &[u8], meta: Meta<'_>) -> MailFile {
    let parsed = MessageParser::new().parse(raw);
    let mut file = MailFile {
        format: FORMAT_VERSION,
        account: meta.account.to_string(),
        folder: meta.folder.to_string(),
        uid: meta.uid,
        flags: meta.flags,
        received: format_local(meta.received),
        date: None,
        size: meta.size,
        message_id: None,
        in_reply_to: None,
        references: Vec::new(),
        from: Vec::new(),
        reply_to: Vec::new(),
        to: Vec::new(),
        cc: Vec::new(),
        bcc: Vec::new(),
        subject: String::new(),
        body: String::new(),
        body_source: BodySource::None,
        attachments: Vec::new(),
    };
    if let Some(message) = parsed {
        fill_headers(&mut file, &message);
        fill_body(&mut file, &message);
        fill_attachments(&mut file, &message);
    }
    file
}

fn fill_headers(file: &mut MailFile, message: &Message<'_>) {
    file.date = message.date().filter(|d| d.is_valid()).and_then(|d| {
        let offset = Offset::from_seconds(
            (i32::from(d.tz_hour) * 3600 + i32::from(d.tz_minute) * 60)
                * if d.tz_before_gmt { -1 } else { 1 },
        )
        .ok()?;
        let timestamp = Timestamp::from_second(d.to_timestamp()).ok()?;
        Some(format_rfc3339(&timestamp.to_zoned(TimeZone::fixed(offset))))
    });
    file.message_id = message.message_id().map(str::to_string);
    file.in_reply_to = text_list(message.in_reply_to()).into_iter().next();
    file.references = text_list(message.references());
    file.from = addrs(message.from());
    file.reply_to = addrs(message.reply_to());
    file.to = message.all_to().flat_map(|a| addrs(Some(a))).collect();
    file.cc = message.all_cc().flat_map(|a| addrs(Some(a))).collect();
    file.bcc = message.all_bcc().flat_map(|a| addrs(Some(a))).collect();
    file.subject = message.subject().unwrap_or_default().to_string();
}

fn fill_body(file: &mut MailFile, message: &Message<'_>) {
    let mut parts = Vec::new();
    for (index, part) in bodies(message).into_iter().enumerate() {
        let (source, text) = match &part.body {
            PartType::Html(html) => (BodySource::Html, Some(html::to_text(html))),
            PartType::Text(text) => (BodySource::Text, Some(text.replace("\r\n", "\n"))),
            _ => (BodySource::Text, None),
        };
        if index == 0 {
            file.body_source = source;
        }
        parts.extend(text.map(|text| tidy(&text)).filter(|text| !text.is_empty()));
    }
    file.body = parts.join("\n");
    if file.body.is_empty() {
        file.body_source = BodySource::None;
    }
}

fn bodies<'a>(message: &'a Message<'_>) -> Vec<&'a MessagePart<'a>> {
    let has_html = message
        .html_bodies()
        .any(|part| matches!(part.body, PartType::Html(_)));
    let text_is_markup = has_html
        && message
            .text_bodies()
            .any(|part| matches!(&part.body, PartType::Text(text) if has_line_break_tags(text)));
    if text_is_markup {
        message.html_bodies().collect()
    } else {
        message.text_bodies().collect()
    }
}

fn has_line_break_tags(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    LINE_BREAK_TAGS.iter().any(|tag| text.contains(tag))
}

fn tidy(text: &str) -> String {
    let mut tidy = String::new();
    let mut gap = false;
    for line in text.lines() {
        if line
            .chars()
            .all(|c| c.is_whitespace() || html::is_invisible(c))
        {
            gap = !tidy.is_empty();
            continue;
        }
        if std::mem::take(&mut gap) {
            tidy.push('\n');
        }
        tidy.push_str(line);
        tidy.push('\n');
    }
    tidy
}

fn fill_attachments(file: &mut MailFile, message: &Message<'_>) {
    file.attachments = message
        .attachments()
        .enumerate()
        .map(|(index, part)| Attachment {
            index: u32::try_from(index).unwrap_or(u32::MAX),
            name: part.attachment_name().map(str::to_string),
            content_type: part
                .content_type()
                .map(|ct| match &ct.c_subtype {
                    Some(sub) => format!("{}/{}", ct.c_type, sub),
                    None => ct.c_type.to_string(),
                })
                .unwrap_or_else(|| "application/octet-stream".to_string()),
            size: part.contents().len() as u64,
        })
        .collect();
}

pub fn attachment_content(raw: &[u8], index: u32) -> Option<Vec<u8>> {
    let message = MessageParser::new().parse(raw)?;
    let part = message.attachments().nth(usize::try_from(index).ok()?)?;
    Some(part.contents().to_vec())
}

fn addrs(address: Option<&Address<'_>>) -> Vec<Addr> {
    address
        .map(|a| {
            a.iter()
                .map(|addr| Addr {
                    name: addr.name().map(str::to_string),
                    address: addr.address().map(str::to_string),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn text_list(value: &HeaderValue<'_>) -> Vec<String> {
    match value {
        HeaderValue::Text(text) => vec![text.to_string()],
        HeaderValue::TextList(list) => list.iter().map(ToString::to_string).collect(),
        _ => Vec::new(),
    }
}

pub fn format_rfc3339(zoned: &jiff::Zoned) -> String {
    zoned.strftime("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

pub fn minute(rfc3339: &str) -> String {
    rfc3339
        .chars()
        .take(MINUTE_CHARS)
        .map(|c| if c == 'T' { ' ' } else { c })
        .collect()
}

pub fn local_zoned(unix_seconds: i64) -> jiff::Zoned {
    Timestamp::from_second(unix_seconds)
        .unwrap_or_default()
        .to_zoned(TimeZone::system())
}

pub fn format_local(unix_seconds: i64) -> String {
    format_rfc3339(&local_zoned(unix_seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minute_drops_seconds_and_offset() {
        assert_eq!(minute("2026-09-15T14:32:05+02:00"), "2026-09-15 14:32");
        assert_eq!(minute(""), "");
    }

    fn meta() -> Meta<'static> {
        Meta {
            account: "private",
            folder: "INBOX",
            uid: 4711,
            flags: Flags {
                seen: true,
                ..Flags::default()
            },
            received: 1_757_939_525,
            size: 1234,
        }
    }

    #[test]
    fn plain_text_mail() {
        let file = convert(include_bytes!("../tests/fixtures/plain.eml"), meta());
        assert_eq!(file.format, 1);
        assert_eq!(file.uid, 4711);
        assert!(file.flags.seen);
        assert_eq!(file.subject, "Offer for project");
        assert_eq!(file.from[0].name.as_deref(), Some("Jane Doe"));
        assert_eq!(file.from[0].address.as_deref(), Some("jane@example.com"));
        assert_eq!(file.to[0].address.as_deref(), Some("me@example.com"));
        assert_eq!(file.message_id.as_deref(), Some("a1b2c3@example.com"));
        assert_eq!(file.in_reply_to.as_deref(), Some("x9y8@example.com"));
        assert_eq!(
            file.references,
            vec!["root@example.com", "x9y8@example.com"]
        );
        assert_eq!(file.date.as_deref(), Some("2026-09-15T14:32:00+02:00"));
        assert_eq!(
            file.body,
            "Hi,\n\nplease find the offer attached.\n\nJane\n"
        );
        assert_eq!(file.body_source, BodySource::Text);
        assert!(file.attachments.is_empty());
    }

    #[test]
    fn html_only_mail_becomes_text() {
        let file = convert(include_bytes!("../tests/fixtures/html-only.eml"), meta());
        assert_eq!(file.body_source, BodySource::Html);
        assert_eq!(
            file.body,
            "Newsletter\n\nHello reader,\nthis is the September issue.\n\n\
             Read more <https://example.com/more>\n"
        );
    }

    #[test]
    fn blank_lines_shrink_to_one_across_parts_and_invisible_lines() {
        let raw = "Subject: Parts\r\nMIME-Version: 1.0\r\n\
                   Content-Type: multipart/mixed; boundary=\"b\"\r\n\r\n\
                   --b\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n\
                   Hi,\r\n\r\n\r\n\r\nnews.\r\n\u{200e}\r\n\u{200b} \u{a0}\r\nBye\r\n\r\n\r\n\
                   --b\r\nContent-Type: image/png\r\nContent-Disposition: inline\r\n\r\nx\r\n\
                   --b\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n\r\nMore.\r\n\
                   --b--\r\n";
        let file = convert(raw.as_bytes(), meta());
        assert_eq!(file.body, "Hi,\n\nnews.\n\nBye\n\nMore.\n");
    }

    #[test]
    fn html_wins_over_a_text_part_with_line_break_tags() {
        let raw = |text: &str| {
            format!(
                "Subject: Both\r\nMIME-Version: 1.0\r\n\
                 Content-Type: multipart/alternative; boundary=\"b\"\r\n\r\n\
                 --b\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{text}\r\n\
                 --b\r\nContent-Type: text/html; charset=utf-8\r\n\r\n\
                 <p>Hello,</p><p>news.</p>\r\n--b--\r\n"
            )
        };
        let file = convert(raw("Hello,<br />\r\n<BR>\r\nnews.").as_bytes(), meta());
        assert_eq!(file.body_source, BodySource::Html);
        assert_eq!(file.body, "Hello,\n\nnews.\n");
        let file = convert(raw("Hello,\r\n\r\nnews as text.").as_bytes(), meta());
        assert_eq!(file.body_source, BodySource::Text);
        assert_eq!(file.body, "Hello,\n\nnews as text.\n");
        let text_only = "Subject: Text\r\n\r\nA line break is <br />.\r\n";
        let file = convert(text_only.as_bytes(), meta());
        assert_eq!(file.body_source, BodySource::Text);
        assert_eq!(file.body, "A line break is <br />.\n");
    }

    #[test]
    fn multipart_with_attachment() {
        let raw = include_bytes!("../tests/fixtures/multipart-attachment.eml");
        let file = convert(raw, meta());
        assert_eq!(file.body_source, BodySource::Text);
        assert_eq!(file.body.trim(), "Text version.");
        assert_eq!(file.attachments.len(), 1);
        assert_eq!(file.attachments[0].index, 0);
        assert_eq!(file.attachments[0].name.as_deref(), Some("offer.pdf"));
        assert_eq!(file.attachments[0].content_type, "application/pdf");
        assert_eq!(file.attachments[0].size, 13);
        assert_eq!(
            attachment_content(raw, 0).as_deref(),
            Some(&b"%PDF-1.4 fake"[..])
        );
        assert_eq!(attachment_content(raw, 1), None);
        assert_eq!(file.subject, "Angebot für Projekt");
    }

    #[test]
    fn broken_charset_does_not_panic() {
        let file = convert(
            include_bytes!("../tests/fixtures/broken-charset.eml"),
            meta(),
        );
        assert_eq!(file.body_source, BodySource::Text);
        assert!(file.body.contains("Gr"), "{}", file.body);
    }

    #[test]
    fn missing_date_is_null() {
        let file = convert(include_bytes!("../tests/fixtures/no-date.eml"), meta());
        assert_eq!(file.date, None);
        assert_eq!(file.subject, "");
        assert_eq!(file.body_source, BodySource::Text);
    }

    #[test]
    fn garbage_still_produces_a_file() {
        let file = convert(b"\xff\xfe not a mail", meta());
        assert_eq!(file.uid, 4711);
        assert_eq!(file.subject, "");
    }
}
