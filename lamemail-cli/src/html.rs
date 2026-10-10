use html5gum::{DefaultEmitter, StartTag, Token, Tokenizer};

const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];
const INVISIBLE: &[&str] = &["script", "style", "template", "title"];
const PARAGRAPHS: &[&str] = &[
    "blockquote",
    "dl",
    "figure",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "hr",
    "p",
    "pre",
];
const LINES: &[&str] = &[
    "address",
    "article",
    "aside",
    "caption",
    "center",
    "dd",
    "details",
    "dialog",
    "div",
    "dt",
    "fieldset",
    "figcaption",
    "footer",
    "form",
    "header",
    "hgroup",
    "legend",
    "li",
    "main",
    "nav",
    "section",
    "summary",
    "table",
    "tr",
];
const LISTS: &[&str] = &["menu", "ol", "ul"];
const TABLE_PARTS: &[&str] = &[
    "caption", "table", "tbody", "td", "tfoot", "th", "thead", "tr",
];
const SCOPE: &[&str] = &["button", "caption", "html", "table", "td", "template", "th"];
const LIST_SCOPE: &[&str] = &[
    "button", "caption", "html", "ol", "table", "td", "template", "th", "ul",
];
const DEFINITION_SCOPE: &[&str] = &[
    "button", "caption", "dl", "html", "table", "td", "template", "th",
];
const ROW_SCOPE: &[&str] = &["html", "table", "template", "tr"];
const TABLE_SCOPE: &[&str] = &["html", "table", "template"];
const HIDDEN_STYLES: &[&str] = &["display:none", "visibility:hidden"];
const LINK_SCHEMES: &[&str] = &["http://", "https://", "mailto:"];
const MAILTO: &str = "mailto:";
const OUTLOOK_LINE: &str = "MsoNormal";
const NO_BREAK_SPACE: char = '\u{a0}';
const CELL_SEPARATOR: &str = " · ";
const QUOTE_PREFIX: &str = "> ";
const LIST_INDENT: usize = 2;

pub fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00ad}'
            | '\u{034f}'
            | '\u{061c}'
            | '\u{115f}'
            | '\u{1160}'
            | '\u{17b4}'
            | '\u{17b5}'
            | '\u{180e}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2064}'
            | '\u{206a}'..='\u{206f}'
            | '\u{2800}'
            | '\u{3164}'
            | '\u{feff}'
            | '\u{ffa0}'
    )
}

pub fn to_text(html: &str) -> String {
    let mut emitter = DefaultEmitter::default();
    emitter.naively_switch_states(true);
    let mut converter = Converter::default();
    for Ok(token) in Tokenizer::new_with_emitter(html, emitter) {
        match token {
            Token::StartTag(tag) => converter.start(&tag),
            Token::EndTag(tag) => converter.end(&String::from_utf8_lossy(&tag.name)),
            Token::String(text) => converter.text(&String::from_utf8_lossy(&text)),
            _ => {}
        }
    }
    converter.finish()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Block {
    Line,
    Paragraph,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
enum Gap {
    #[default]
    None,
    Space,
    Cell,
}

enum Role {
    Shown,
    Link,
    Hidden,
    Inert,
}

struct Open {
    name: String,
    role: Role,
    block: Option<Block>,
}

enum List {
    Bullets,
    Numbers(i64),
}

struct Link {
    href: String,
    finished_lines: usize,
    start: Option<usize>,
    image: bool,
}

#[derive(Default)]
struct Converter {
    out: String,
    line: String,
    prefix_len: usize,
    open: bool,
    visible: bool,
    gap: Gap,
    blank: bool,
    last_blank: bool,
    trailing_blank: usize,
    last_quote: usize,
    line_quote: usize,
    finished_lines: usize,
    quote: usize,
    pre: usize,
    hidden: usize,
    marker: Option<String>,
    lists: Vec<List>,
    links: Vec<Link>,
    stack: Vec<Open>,
}

impl Converter {
    fn start(&mut self, tag: &StartTag<()>) {
        let name = String::from_utf8_lossy(&tag.name).into_owned();
        self.close_implied(&name);
        if VOID.contains(&name.as_str()) {
            if self.hidden == 0 && !is_hidden(tag) {
                self.void(&name, tag);
            }
            return;
        }
        let block = self.block_of(&name, tag);
        let role = if self.hidden > 0 {
            Role::Inert
        } else if INVISIBLE.contains(&name.as_str()) || is_hidden(tag) {
            self.hidden += 1;
            Role::Hidden
        } else {
            self.enter(&name, tag, block)
        };
        self.stack.push(Open { name, role, block });
    }

    fn end(&mut self, name: &str) {
        let scope = if TABLE_PARTS.contains(&name) {
            TABLE_SCOPE
        } else {
            SCOPE
        };
        self.close(&[name], scope);
    }

    fn close_implied(&mut self, name: &str) {
        match name {
            "li" => self.close(&["li"], LIST_SCOPE),
            "dd" | "dt" => self.close(&["dd", "dt"], DEFINITION_SCOPE),
            "td" | "th" => self.close(&["td", "th"], ROW_SCOPE),
            "tr" => self.close(&["tr"], TABLE_SCOPE),
            _ => {}
        }
        if [PARAGRAPHS, LINES, LISTS]
            .iter()
            .any(|names| names.contains(&name))
        {
            self.close(&["p"], SCOPE);
        }
    }

    fn close(&mut self, targets: &[&str], boundaries: &[&str]) {
        let Some(index) = self.stack.iter().rposition(|open| {
            targets.contains(&open.name.as_str()) || boundaries.contains(&open.name.as_str())
        }) else {
            return;
        };
        if !targets.contains(&self.stack[index].name.as_str()) {
            return;
        }
        for open in self.stack.split_off(index).into_iter().rev() {
            self.leave(open);
        }
    }

    fn block_of(&self, name: &str, tag: &StartTag<()>) -> Option<Block> {
        if LISTS.contains(&name) {
            return Some(self.list_block());
        }
        let outlook_line = name == "p"
            && attribute(tag, "class").is_some_and(|class| class.contains(OUTLOOK_LINE));
        if PARAGRAPHS.contains(&name) && !outlook_line {
            Some(Block::Paragraph)
        } else if LINES.contains(&name) || outlook_line {
            Some(Block::Line)
        } else {
            None
        }
    }

    fn list_block(&self) -> Block {
        if self.lists.is_empty() {
            Block::Paragraph
        } else {
            Block::Line
        }
    }

    fn enter(&mut self, name: &str, tag: &StartTag<()>, block: Option<Block>) -> Role {
        if let Some(block) = block {
            self.block(block);
        }
        match name {
            "a" => {
                if let Some(href) = link_target(tag) {
                    self.links.push(Link {
                        href,
                        finished_lines: self.finished_lines,
                        start: self.open.then_some(self.line.len()),
                        image: false,
                    });
                    return Role::Link;
                }
            }
            "blockquote" => self.quote += 1,
            "pre" => self.pre += 1,
            "ol" => self.lists.push(List::Numbers(list_start(tag))),
            "ul" | "menu" => self.lists.push(List::Bullets),
            "li" => self.marker = Some(self.bullet()),
            "td" | "th" if self.open => self.gap = Gap::Cell,
            _ => {}
        }
        Role::Shown
    }

    fn leave(&mut self, open: Open) {
        match open.role {
            Role::Inert => {}
            Role::Hidden => self.hidden -= 1,
            Role::Link => {
                if let Some(link) = self.links.pop() {
                    self.link_end(link);
                }
            }
            Role::Shown => {
                if let Some(block) = open.block {
                    self.block(block);
                }
                match open.name.as_str() {
                    "blockquote" => self.quote -= 1,
                    "pre" => self.pre -= 1,
                    "menu" | "ol" | "ul" => {
                        self.lists.pop();
                    }
                    "li" => self.marker = None,
                    _ => {}
                }
            }
        }
    }

    fn void(&mut self, name: &str, tag: &StartTag<()>) {
        match name {
            "br" => self.hard_break(),
            "img" => self.image(tag),
            _ => {
                if let Some(block) = self.block_of(name, tag) {
                    self.block(block);
                }
            }
        }
    }

    fn image(&mut self, tag: &StartTag<()>) {
        let Some(link) = self.links.last_mut() else {
            return;
        };
        if is_pixel(tag) {
            return;
        }
        link.image = true;
        if let Some(alt) = attribute(tag, "alt") {
            self.text(&format!(" {alt} "));
        }
    }

    fn bullet(&mut self) -> String {
        let indent = " ".repeat(LIST_INDENT * self.lists.len().saturating_sub(1));
        match self.lists.last_mut() {
            Some(List::Numbers(next)) => {
                let number = *next;
                *next += 1;
                format!("{indent}{number}. ")
            }
            _ => format!("{indent}- "),
        }
    }

    fn link_end(&mut self, link: Link) {
        let target = display(&link.href);
        if self.finished_lines != link.finished_lines {
            self.text(&format!(" <{target}>"));
            return;
        }
        let start = link.start.unwrap_or(self.prefix_len).min(self.line.len());
        let rest = &self.line[start..];
        let label_start = start + rest.len() - rest.trim_start().len();
        let label = rest.trim();
        let empty = label.is_empty();
        let same = bare(label) == bare(&target);
        if empty {
            if link.image {
                self.text(&format!(" {target}"));
            }
        } else if same {
            self.line.truncate(label_start);
            self.line.push_str(&target);
        } else {
            self.text(&format!(" <{target}>"));
        }
    }

    fn text(&mut self, text: &str) {
        if self.hidden > 0 {
            return;
        }
        for c in text.chars() {
            if is_invisible(c) {
                continue;
            }
            if self.pre > 0 && c == '\n' {
                self.hard_break();
            } else if c == NO_BREAK_SPACE {
                self.put(' ');
            } else if self.pre > 0 || !c.is_whitespace() {
                self.put(c);
            } else if self.open {
                self.gap = self.gap.max(Gap::Space);
            }
        }
    }

    fn put(&mut self, c: char) {
        if !self.open {
            self.open_line();
        }
        match self.gap {
            Gap::Cell if c == ' ' => return,
            Gap::Cell if self.visible => self.line.push_str(CELL_SEPARATOR),
            Gap::Cell => self.line.truncate(self.prefix_len),
            Gap::Space => self.line.push(' '),
            Gap::None => {}
        }
        self.gap = Gap::None;
        self.line.push(c);
        self.visible |= !c.is_whitespace();
    }

    fn open_line(&mut self) {
        if std::mem::take(&mut self.blank) {
            self.blank_line(self.last_quote.min(self.quote));
        }
        self.line_quote = self.quote;
        self.line.push_str(&QUOTE_PREFIX.repeat(self.quote));
        match self.marker.take() {
            Some(marker) => self.line.push_str(&marker),
            None => self
                .line
                .push_str(&" ".repeat(LIST_INDENT * self.lists.len())),
        }
        self.prefix_len = self.line.len();
        self.open = true;
    }

    fn end_line(&mut self) {
        if self.visible {
            self.out.push_str(self.line.trim_end());
            self.out.push('\n');
            self.last_blank = false;
            self.last_quote = self.line_quote;
        } else {
            self.blank_line(self.line_quote);
        }
        self.line.clear();
        self.open = false;
        self.visible = false;
        self.gap = Gap::None;
        self.finished_lines += 1;
    }

    fn blank_line(&mut self, quote: usize) {
        if self.out.is_empty() || self.last_blank {
            return;
        }
        self.trailing_blank = self.out.len();
        self.out.push_str(QUOTE_PREFIX.repeat(quote).trim_end());
        self.out.push('\n');
        self.last_blank = true;
    }

    fn block(&mut self, block: Block) {
        if self.open {
            self.end_line();
        }
        self.gap = Gap::None;
        if block == Block::Paragraph {
            self.blank = true;
        }
    }

    fn hard_break(&mut self) {
        if !self.open {
            self.open_line();
        }
        self.end_line();
    }

    fn finish(mut self) -> String {
        if self.open {
            self.end_line();
        }
        if self.last_blank {
            self.out.truncate(self.trailing_blank);
        }
        self.out
    }
}

fn attribute(tag: &StartTag<()>, name: &str) -> Option<String> {
    tag.attributes
        .get(name.as_bytes())
        .map(|value| String::from_utf8_lossy(value).into_owned())
}

fn is_hidden(tag: &StartTag<()>) -> bool {
    tag.attributes.contains_key("hidden".as_bytes())
        || attribute(tag, "style").is_some_and(|style| {
            let style: String = style
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
                .to_ascii_lowercase();
            HIDDEN_STYLES.iter().any(|hidden| style.contains(hidden))
        })
}

fn is_pixel(tag: &StartTag<()>) -> bool {
    ["width", "height"].iter().any(|name| {
        attribute(tag, name)
            .and_then(|size| size.trim().trim_end_matches("px").parse::<u32>().ok())
            .is_some_and(|size| size <= 1)
    })
}

fn list_start(tag: &StartTag<()>) -> i64 {
    attribute(tag, "start")
        .and_then(|start| start.trim().parse().ok())
        .unwrap_or(1)
}

fn link_target(tag: &StartTag<()>) -> Option<String> {
    let href: String = attribute(tag, "href")?
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    let href = href.trim();
    LINK_SCHEMES
        .iter()
        .any(|scheme| has_prefix(href, scheme))
        .then(|| href.to_string())
}

fn display(href: &str) -> String {
    if has_prefix(href, MAILTO) {
        let address = &href[MAILTO.len()..];
        address.split('?').next().unwrap_or(address).to_string()
    } else {
        href.to_string()
    }
}

fn bare(url: &str) -> String {
    let url = url.to_lowercase();
    let url = LINK_SCHEMES
        .iter()
        .find_map(|scheme| url.strip_prefix(scheme))
        .unwrap_or(&url);
    let url = url.strip_prefix("www.").unwrap_or(url);
    url.trim_end_matches('/').to_string()
}

fn has_prefix(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
}

#[cfg(test)]
mod tests {
    use super::to_text;
    use crate::reply::{Part, parts};

    #[test]
    fn paragraphs_and_divs() {
        assert_eq!(
            to_text("<p>Hello   Robin,</p><p>see\n  you <b>soon</b>.</p>"),
            "Hello Robin,\n\nsee you soon.\n"
        );
        assert_eq!(
            to_text("<div>First</div><div>Second</div>"),
            "First\nSecond\n"
        );
        assert_eq!(
            to_text("<div>Hi Paul,</div><div><br></div><div>text<br></div><div>more</div>"),
            "Hi Paul,\n\ntext\nmore\n"
        );
        assert_eq!(
            to_text("one<br>two<br><br><br><br>three<br><br>"),
            "one\ntwo\n\nthree\n"
        );
        assert_eq!(
            to_text("<h1>Title</h1>Body<hr>End"),
            "Title\n\nBody\n\nEnd\n"
        );
        assert_eq!(to_text(""), "");
        assert_eq!(to_text("<p> </p><div>&nbsp;</div>"), "");
    }

    #[test]
    fn unclosed_and_stray_tags() {
        assert_eq!(
            to_text("<p>one<p>two<div>three</span>"),
            "one\n\ntwo\n\nthree\n"
        );
        assert_eq!(
            to_text("<p style=\"display:none\">preview<p>shown"),
            "shown\n"
        );
        assert_eq!(
            to_text("<ul><li>one<li>two</ul>after"),
            "- one\n- two\n\nafter\n"
        );
    }

    #[test]
    fn outlook_lines_and_no_break_spaces() {
        let html = "<p class=MsoNormal>Hallo Paul,<o:p></o:p></p>\
                    <p class=MsoNormal><o:p>&nbsp;</o:p></p>\
                    <p class=MsoNormal>Zeile eins<o:p></o:p></p>\
                    <p class=MsoNormal>Zeile zwei<o:p></o:p></p>";
        assert_eq!(to_text(html), "Hallo Paul,\n\nZeile eins\nZeile zwei\n");
        assert_eq!(to_text("a&nbsp;&nbsp;b"), "a  b\n");
    }

    #[test]
    fn lists() {
        let html = "<ul><li>one</li><li>two<ul><li>two a</li></ul></li><li>three</li></ul>\
                    <ol start=\"3\"><li>third<li>fourth</ol>";
        assert_eq!(
            to_text(html),
            "- one\n- two\n  - two a\n- three\n\n3. third\n4. fourth\n"
        );
        assert_eq!(to_text("<ul><li>a<br>b</li></ul>"), "- a\n  b\n");
    }

    #[test]
    fn table_rows_are_lines_and_empty_cells_vanish() {
        let html = "<table><tr><td>Item</td><td>Price</td></tr>\
                    <tr><td>&nbsp;</td><td>Scarf</td><td>12 €</td><td></td></tr>\
                    <tr><td><table><tr><td><p>Nested</p></td></tr></table></td></tr></table>";
        assert_eq!(to_text(html), "Item · Price\nScarf · 12 €\n\nNested\n");
    }

    #[test]
    fn preformatted_text_keeps_its_spaces() {
        assert_eq!(
            to_text("<p>Code:</p><pre>\n  a   b\n\tc\n</pre>after"),
            "Code:\n\n  a   b\n\tc\n\nafter\n"
        );
    }

    #[test]
    fn links_show_their_address() {
        assert_eq!(
            to_text("Go <a href=\"https://example.com/sale?a=1&amp;b=2\">shop now</a>, today"),
            "Go shop now <https://example.com/sale?a=1&b=2>, today\n"
        );
        assert_eq!(
            to_text("<a href=\"https://example.com/x\">https://example.com/x</a>"),
            "https://example.com/x\n"
        );
        assert_eq!(
            to_text("<a href=\"https://www.example.com/\">example.com</a>"),
            "https://www.example.com/\n"
        );
        assert_eq!(
            to_text("Maria &lt;<a href=\"mailto:maria@example.com\">maria@example.com</a>&gt;"),
            "Maria <maria@example.com>\n"
        );
        assert_eq!(
            to_text("<a href=\"mailto:news@example.com?subject=Stop\">Write us</a>"),
            "Write us <news@example.com>\n"
        );
        assert_eq!(
            to_text("<a href=\"#top\">Top</a> <a href=\"tel:+49\">call</a> <a>plain</a>"),
            "Top call plain\n"
        );
        assert_eq!(
            to_text("<a href=\"https://example.com/a\"><div>Title</div></a>"),
            "Title\n<https://example.com/a>\n"
        );
        assert_eq!(to_text("x<a href=\"https://example.com/\"> </a>y"), "x y\n");
    }

    #[test]
    fn images_count_only_in_links() {
        let html = "<img src=\"https://example.com/photo.jpg\" alt=\"Photo\">\
                    <a href=\"https://example.com/\"><img src=\"logo.png\" alt=\"Example Shop\"></a>\
                    <p><a href=\"https://example.com/fb\"><img src=\"fb.png\"></a></p>\
                    <a href=\"https://example.com/t\"><img src=\"t.gif\" width=\"1\" height=\"1\"></a>\
                    <img src=\"https://track.example.com/p.gif\" width=\"1px\" height=\"1px\" alt=\"x\">";
        assert_eq!(
            to_text(html),
            "Example Shop <https://example.com/>\n\nhttps://example.com/fb\n"
        );
    }

    #[test]
    fn hidden_parts_and_fillers_are_dropped() {
        let html = "<html><head><title>News</title><style>.x { color: red }</style></head>\
                    <body><script>alert(1)</script>\
                    <div style=\"display: none; max-height: 0\">Preview text &zwnj;&nbsp;</div>\
                    <span hidden>gone</span><span style=\"VISIBILITY:hidden\">gone</span>\
                    <p>Vis\u{ad}i&#8203;ble&#847;</p></body></html>";
        assert_eq!(to_text(html), "Visible\n");
    }

    #[test]
    fn quotes_become_quote_parts_with_their_attribution() {
        let html = "<div>Sure.</div><div><br></div>\
                    <div class=\"gmail_quote\"><div class=\"gmail_attr\">Am 28.09.2026 um 10:00 \
                    schrieb Maria &lt;<a href=\"mailto:maria@example.com\">maria@example.com</a>&gt;:<br></div>\
                    <blockquote class=\"gmail_quote\"><div>Friday?</div><div><br></div>\
                    <blockquote type=\"cite\">Older</blockquote></blockquote></div>";
        let text = to_text(html);
        assert_eq!(
            text,
            "Sure.\n\nAm 28.09.2026 um 10:00 schrieb Maria <maria@example.com>:\n\n\
             > Friday?\n>\n> > Older\n"
        );
        assert_eq!(
            parts(&text),
            vec![
                Part::Text {
                    text: "Sure.".to_string()
                },
                Part::Quote {
                    attribution: Some(
                        "Am 28.09.2026 um 10:00 schrieb Maria <maria@example.com>".to_string()
                    ),
                    parts: vec![
                        Part::Text {
                            text: "Friday?".to_string()
                        },
                        Part::Quote {
                            attribution: None,
                            parts: vec![Part::Text {
                                text: "Older".to_string()
                            }],
                        },
                    ],
                },
            ]
        );
    }
}
