use html5gum::{DefaultEmitter, StartTag, Token, Tokenizer};

use crate::reply::{MAX_QUOTE_DEPTH, SIGNATURE_SEPARATOR};

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
const STRUCK: &[&str] = &["del", "s", "strike"];
const STRUCK_STYLES: &[&str] = &["text-decoration", "text-decoration-line"];
const LINE_THROUGH: &str = "line-through";
const SIGNATURE_CLASSES: &[&str] = &[
    "gmail_signature",
    "moz-signature",
    "protonmail_signature_block",
];
const SIGNATURE_IDS: &[&str] = &[
    "AppleMailSignature",
    "Signature",
    "ms-outlook-mobile-signature",
];
const LINK_SCHEMES: &[&str] = &["http://", "https://", "mailto:"];
const MAILTO: &str = "mailto:";
const WWW: &str = "www.";
const OUTLOOK_LINES: &[&str] = &["MsoNormal", "MsoListParagraph"];
const OUTLOOK_QUOTE: &str = "divRplyFwdMsg";
const OUTLOOK_PREFIX: &str = "x_";
const WORD_MARKER: &str = "[if !supportLists]";
const WORD_MARKER_END: &str = "[endif]";
const WORD_LEVEL: &str = "level";
const IMPORTANT: &str = "!important";
const NO_BREAK_SPACE: char = '\u{a0}';
const CELL_SEPARATOR: &str = " · ";
const QUOTE_PREFIX: &str = "> ";
const STRIKE: &str = "~~";
const BULLET: &str = "-";
const LIST_INDENT: usize = 2;
const MAX_OPEN: usize = 512;

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
            Token::Comment(comment) => converter.comment(&String::from_utf8_lossy(&comment)),
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
    Word,
    Space,
    Cell,
    Cells(usize),
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
    quote: usize,
    struck: bool,
    signature: bool,
    spacer: Option<usize>,
}

impl Open {
    fn unseen(name: String, role: Role) -> Self {
        Self {
            name,
            role,
            block: None,
            quote: 0,
            struck: false,
            signature: false,
            spacer: None,
        }
    }
}

enum List {
    Bullets,
    Numbers(i64),
}

#[derive(Default)]
struct Table {
    data: bool,
    cells: usize,
    row: usize,
}

struct Link {
    href: String,
    label: Option<(usize, usize)>,
    image: bool,
}

struct Style(Vec<(String, String)>);

impl Style {
    fn of(tag: &StartTag<()>) -> Self {
        let style = attribute(tag, "style")
            .unwrap_or_default()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_ascii_lowercase();
        Self(
            style
                .split(';')
                .filter_map(|declaration| declaration.split_once(':'))
                .map(|(property, value)| {
                    let value = value.trim_end_matches(IMPORTANT);
                    (property.to_string(), value.to_string())
                })
                .collect(),
        )
    }

    fn value(&self, property: &str) -> Option<&str> {
        self.0
            .iter()
            .rev()
            .find(|(name, _)| name == property)
            .map(|(_, value)| value.as_str())
    }

    fn is(&self, property: &str, value: &str) -> bool {
        self.value(property) == Some(value)
    }

    fn word_level(&self) -> Option<usize> {
        let (_, level) = self.value("mso-list")?.split_once(WORD_LEVEL)?;
        let digits: String = level.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    }
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
    written: usize,
    quote: usize,
    pre: usize,
    hidden: usize,
    sup: usize,
    sub: usize,
    strikes: usize,
    strike_open: bool,
    signatures: usize,
    signing: bool,
    after_separator: bool,
    bullet: Option<String>,
    word_marker: Option<String>,
    word_level: usize,
    lists: Vec<List>,
    tables: Vec<Table>,
    links: Vec<Link>,
    stack: Vec<Open>,
}

impl Converter {
    fn start(&mut self, tag: &StartTag<()>) {
        let name = String::from_utf8_lossy(&tag.name).into_owned();
        self.close_implied(&name);
        let void = VOID.contains(&name.as_str());
        if self.hidden > 0 {
            if !void && self.stack.len() < MAX_OPEN {
                self.stack.push(Open::unseen(name, Role::Inert));
            }
            return;
        }
        let style = Style::of(tag);
        let hidden = INVISIBLE.contains(&name.as_str()) || is_hidden(tag, &style);
        if void {
            if !hidden {
                self.void(&name, tag, &style);
            }
        } else if self.stack.len() < MAX_OPEN {
            let open = if hidden {
                self.hidden += 1;
                Open::unseen(name, Role::Hidden)
            } else {
                self.shown(name, tag, &style)
            };
            self.stack.push(open);
        }
    }

    fn shown(&mut self, name: String, tag: &StartTag<()>, style: &Style) -> Open {
        let quote = self.quote;
        let block = self.block_of(&name, tag);
        let role = self.enter(&name, tag, style, block);
        let struck = is_struck(&name, style);
        let signature = is_signature(tag);
        if struck {
            self.strikes += 1;
        }
        if signature {
            self.signing |= self.signatures == 0;
            self.signatures += 1;
        }
        Open {
            name,
            role,
            block,
            quote,
            struck,
            signature,
            spacer: is_spacer(tag, style).then_some(self.written),
        }
    }

    fn end(&mut self, name: &str) {
        let scope = if TABLE_PARTS.contains(&name) {
            TABLE_SCOPE
        } else {
            SCOPE
        };
        self.close(&[name], scope);
    }

    fn comment(&mut self, comment: &str) {
        if self.hidden > 0 {
            return;
        }
        if comment == WORD_MARKER {
            self.flush_word_marker();
            self.word_marker = Some(String::new());
        } else if comment == WORD_MARKER_END
            && let Some(marker) = self.word_marker.take()
        {
            self.word_bullet(&marker);
        }
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
        let line = name == "p"
            && (self.stack.last().is_some_and(|open| open.name == "li")
                || attribute(tag, "class")
                    .is_some_and(|class| OUTLOOK_LINES.iter().any(|mark| class.contains(mark))));
        if PARAGRAPHS.contains(&name) && !line {
            Some(Block::Paragraph)
        } else if LINES.contains(&name) || line {
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

    fn quote_depth(&self) -> usize {
        self.quote.min(MAX_QUOTE_DEPTH)
    }

    fn indent_room(&self) -> usize {
        MAX_QUOTE_DEPTH - self.quote_depth()
    }

    fn list_depth(&self) -> usize {
        self.lists.len().min(self.indent_room())
    }

    fn enter(
        &mut self,
        name: &str,
        tag: &StartTag<()>,
        style: &Style,
        block: Option<Block>,
    ) -> Role {
        if let Some(block) = block {
            self.block(block);
        }
        if let Some(level) = style.word_level() {
            self.word_level = level;
        }
        match name {
            "a" => {
                if let Some(href) = link_target(tag) {
                    self.links.push(Link {
                        href,
                        label: None,
                        image: false,
                    });
                    return Role::Link;
                }
            }
            "blockquote" if !is_indent(tag, style) => self.quote += 1,
            "div" if is_outlook_quote(tag, style) => {
                self.block(Block::Paragraph);
                self.quote += 1;
            }
            "pre" => self.pre += 1,
            "sub" => self.sub += 1,
            "sup" => self.sup += 1,
            "ol" => self.lists.push(List::Numbers(list_start(tag))),
            "ul" | "menu" => self.lists.push(List::Bullets),
            "li" => self.bullet = Some(self.next_bullet()),
            "table" => self.tables.push(Table::default()),
            "tr" => {
                if let Some(table) = self.tables.last_mut() {
                    table.cells = 0;
                    table.row = self.finished_lines;
                }
            }
            "td" | "th" => self.cell(name == "th"),
            _ => {}
        }
        Role::Shown
    }

    fn leave(&mut self, open: Open) {
        match open.role {
            Role::Inert => return,
            Role::Hidden => {
                self.hidden -= 1;
                return;
            }
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
                    "blockquote" => self.quote = open.quote,
                    "pre" => self.pre -= 1,
                    "sub" => self.sub -= 1,
                    "sup" => self.sup -= 1,
                    "menu" | "ol" | "ul" => {
                        self.lists.pop();
                    }
                    "table" => {
                        self.tables.pop();
                    }
                    "li" => self.bullet = None,
                    _ => {}
                }
            }
        }
        if open.struck {
            self.strikes -= 1;
            if self.strikes == 0 {
                self.close_strike();
            }
        }
        if open.signature {
            self.signatures -= 1;
            self.signing &= self.signatures > 0;
        }
        if open.spacer == Some(self.written) && !self.visible {
            self.blank = true;
        }
    }

    fn void(&mut self, name: &str, tag: &StartTag<()>, style: &Style) {
        match name {
            "br" => self.hard_break(),
            "img" => self.image(tag, style),
            _ => {
                if let Some(block) = self.block_of(name, tag) {
                    self.block(block);
                }
            }
        }
    }

    fn image(&mut self, tag: &StartTag<()>, style: &Style) {
        if is_pixel(tag, style) {
            return;
        }
        let alt = attribute(tag, "alt").unwrap_or_default();
        let alt = alt.trim();
        if let Some(link) = self.links.last_mut() {
            link.image = true;
        }
        if self.links.is_empty() && alt.chars().any(char::is_alphanumeric) {
            self.text(&format!(" [{alt}] "));
        } else {
            self.text(&format!(" {alt} "));
        }
    }

    fn cell(&mut self, header: bool) {
        let kept = self.tables.last_mut().is_some_and(|table| {
            table.data |= header;
            table.cells += 1;
            table.data && table.cells > 1
        });
        if kept {
            self.gap = match self.gap {
                Gap::Cells(cells) => Gap::Cells(cells + 1),
                _ => Gap::Cells(1),
            };
        } else if self.open {
            self.gap = Gap::Cell;
        }
    }

    fn kept_cells(&mut self, cells: usize) {
        if self.visible {
            self.line.push_str(&CELL_SEPARATOR.trim_end().repeat(cells));
            self.line.push(' ');
            return;
        }
        self.line.truncate(self.prefix_len);
        if self
            .tables
            .last()
            .is_some_and(|table| table.row == self.finished_lines)
        {
            self.line
                .push_str(&CELL_SEPARATOR.trim_start().repeat(cells));
        }
    }

    fn next_bullet(&mut self) -> String {
        let indent = " ".repeat(LIST_INDENT * self.list_depth().saturating_sub(1));
        match self.lists.last_mut() {
            Some(List::Numbers(next)) => {
                let number = *next;
                *next = next.saturating_add(1);
                format!("{indent}{number}. ")
            }
            _ => format!("{indent}{BULLET} "),
        }
    }

    fn word_bullet(&mut self, marker: &str) {
        let marker: String = marker.chars().filter(|c| !is_invisible(*c)).collect();
        let marker = marker.trim();
        if marker.is_empty() {
            return;
        }
        if self.visible {
            self.text(&format!("{marker} "));
            return;
        }
        let bullet = if marker.chars().count() == 1 {
            BULLET
        } else {
            marker
        };
        let level = self.word_level.saturating_sub(1).min(self.indent_room());
        if !self.open {
            self.open_line();
        }
        self.line.truncate(self.prefix_len);
        self.line.push_str(&" ".repeat(LIST_INDENT * level));
        self.line.push_str(bullet);
        self.line.push(' ');
        self.prefix_len = self.line.len();
        self.gap = Gap::None;
    }

    fn link_end(&mut self, link: Link) {
        let target = display(&link.href);
        match link.label {
            None if link.image => self.address(&format!(" {target}")),
            None => {}
            Some((line, start)) if line == self.finished_lines && self.shows(start, &target) => {
                self.line.truncate(start);
                self.line.push_str(&target);
                self.gap = self.gap.max(Gap::Word);
            }
            Some(_) => self.address(&format!(" <{target}>")),
        }
    }

    fn shows(&self, start: usize, target: &str) -> bool {
        self.line
            .get(start..)
            .is_some_and(|label| bare(label.trim_end()).eq_ignore_ascii_case(bare(target)))
    }

    fn address(&mut self, address: &str) {
        self.text(address);
        self.gap = self.gap.max(Gap::Word);
    }

    fn text(&mut self, text: &str) {
        if self.hidden > 0 {
            return;
        }
        if let Some(marker) = &mut self.word_marker {
            marker.push_str(text);
            return;
        }
        let script = self.script(text);
        for c in text.chars() {
            if is_invisible(c) {
                continue;
            }
            let c = script.map_or(c, |script| script(c));
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

    fn script(&self, text: &str) -> Option<fn(char) -> char> {
        let script: fn(char) -> char = match (self.sup, self.sub) {
            (0, 0) => return None,
            (0, _) => subscript,
            _ => superscript,
        };
        text.chars()
            .all(|c| c.is_whitespace() || is_invisible(c) || script(c) != c)
            .then_some(script)
    }

    fn put(&mut self, c: char) {
        if !self.open {
            self.open_line();
        }
        match self.gap {
            Gap::Cell | Gap::Cells(_) if c == ' ' => return,
            Gap::Cells(cells) => self.kept_cells(cells),
            Gap::Cell if self.visible => self.line.push_str(CELL_SEPARATOR),
            Gap::Cell => self.line.truncate(self.prefix_len),
            Gap::Space => self.line.push(' '),
            Gap::Word if c.is_alphanumeric() => self.line.push(' '),
            Gap::Word | Gap::None => {}
        }
        self.gap = Gap::None;
        if !c.is_whitespace() {
            self.begin_visible();
        }
        self.line.push(c);
    }

    fn begin_visible(&mut self) {
        if self.strikes > 0 && !self.strike_open {
            self.line.push_str(STRIKE);
            self.strike_open = true;
        }
        let place = (self.finished_lines, self.line.len());
        self.links
            .iter_mut()
            .rev()
            .take_while(|link| link.label.is_none())
            .for_each(|link| link.label = Some(place));
        self.visible = true;
        self.written += 1;
    }

    fn close_strike(&mut self) {
        if std::mem::take(&mut self.strike_open) {
            self.line.truncate(self.line.trim_end().len());
            self.line.push_str(STRIKE);
        }
    }

    fn open_line(&mut self) {
        let quote = self.quote_depth();
        if std::mem::take(&mut self.blank) {
            self.blank_line(self.last_quote.min(quote));
        }
        self.line_quote = quote;
        self.line.push_str(&QUOTE_PREFIX.repeat(quote));
        match self.bullet.take() {
            Some(bullet) => self.line.push_str(&bullet),
            None => self
                .line
                .push_str(&" ".repeat(LIST_INDENT * self.list_depth())),
        }
        self.prefix_len = self.line.len();
        self.open = true;
    }

    fn end_line(&mut self) {
        self.close_strike();
        if self.visible {
            self.separate_signature();
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

    fn separate_signature(&mut self) {
        let separator = self
            .line
            .get(self.prefix_len..)
            .is_some_and(|text| text.trim() == SIGNATURE_SEPARATOR);
        if std::mem::take(&mut self.signing) && !separator && !self.after_separator {
            self.blank_line(self.line_quote);
            self.out.push_str(&QUOTE_PREFIX.repeat(self.line_quote));
            self.out.push_str(SIGNATURE_SEPARATOR);
            self.out.push('\n');
        }
        self.after_separator = separator;
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
        self.flush_word_marker();
        if self.open {
            self.end_line();
        }
        self.gap = Gap::None;
        if block == Block::Paragraph {
            self.blank = true;
        }
    }

    fn flush_word_marker(&mut self) {
        if let Some(marker) = self.word_marker.take() {
            self.text(&marker);
        }
    }

    fn hard_break(&mut self) {
        self.flush_word_marker();
        if !self.open {
            self.open_line();
        }
        self.end_line();
    }

    fn finish(mut self) -> String {
        self.flush_word_marker();
        while let Some(open) = self.stack.pop() {
            self.leave(open);
        }
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

fn is_hidden(tag: &StartTag<()>, style: &Style) -> bool {
    tag.attributes.contains_key("hidden".as_bytes())
        || style.is("display", "none")
        || style.is("visibility", "hidden")
        || style.value("opacity").is_some_and(is_zero)
        || style.value("max-height").is_some_and(is_zero) && style.is("overflow", "hidden")
}

fn is_struck(name: &str, style: &Style) -> bool {
    STRUCK.contains(&name)
        || STRUCK_STYLES.iter().any(|property| {
            style
                .value(property)
                .is_some_and(|value| value.contains(LINE_THROUGH))
        })
}

fn is_signature(tag: &StartTag<()>) -> bool {
    attribute(tag, "id").is_some_and(|id| SIGNATURE_IDS.contains(&unprefixed(&id)))
        || attribute(tag, "class").is_some_and(|class| {
            class
                .split_whitespace()
                .any(|name| SIGNATURE_CLASSES.contains(&unprefixed(name)))
        })
}

fn unprefixed(name: &str) -> &str {
    name.trim_start_matches(OUTLOOK_PREFIX)
}

fn is_indent(tag: &StartTag<()>, style: &Style) -> bool {
    !tag.attributes.contains_key("type".as_bytes())
        && style.is("border", "none")
        && style.value("border-left").is_none()
}

fn is_outlook_quote(tag: &StartTag<()>, style: &Style) -> bool {
    let starts = |property, start| {
        style
            .value(property)
            .is_some_and(|value| value.starts_with(start))
    };
    attribute(tag, "id").is_some_and(|id| unprefixed(&id) == OUTLOOK_QUOTE)
        || style.is("border", "none") && starts("border-top", "solid") && starts("padding", "3.0pt")
}

fn is_spacer(tag: &StartTag<()>, style: &Style) -> bool {
    style.value("line-height").is_some()
        && (style.value("height").is_some_and(is_length)
            || attribute(tag, "height").is_some_and(|height| is_length(&height)))
}

fn number(size: &str) -> &str {
    let size = size.trim();
    let end = size
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(size.len());
    &size[..end]
}

fn is_zero(size: &str) -> bool {
    number(size).contains('0') && !is_length(size)
}

fn is_length(size: &str) -> bool {
    number(size).contains(|c: char| matches!(c, '1'..='9'))
}

fn is_pixel(tag: &StartTag<()>, style: &Style) -> bool {
    ["width", "height"].iter().any(|name| {
        attribute(tag, name).is_some_and(|size| is_tiny(&size))
            || style.value(name).is_some_and(is_tiny)
    })
}

fn is_tiny(size: &str) -> bool {
    size.trim()
        .trim_end_matches("px")
        .parse::<u32>()
        .is_ok_and(|size| size <= 1)
}

fn superscript(c: char) -> char {
    match c {
        '1' => '\u{b9}',
        '2' => '\u{b2}',
        '3' => '\u{b3}',
        '0' | '4'..='9' => shifted(c, '\u{2070}'),
        '+' => '\u{207a}',
        '-' => '\u{207b}',
        _ => c,
    }
}

fn subscript(c: char) -> char {
    match c {
        '0'..='9' => shifted(c, '\u{2080}'),
        '+' => '\u{208a}',
        '-' => '\u{208b}',
        _ => c,
    }
}

fn shifted(digit: char, zero: char) -> char {
    char::from_u32(u32::from(zero) + u32::from(digit) - u32::from('0')).unwrap_or(digit)
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

fn bare(url: &str) -> &str {
    let url = LINK_SCHEMES
        .iter()
        .find(|scheme| has_prefix(url, scheme))
        .map_or(url, |scheme| &url[scheme.len()..]);
    let url = if has_prefix(url, WWW) {
        &url[WWW.len()..]
    } else {
        url
    };
    url.trim_end_matches('/')
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
        assert_eq!(
            to_text("<ol start=\"9223372036854775807\"><li>a<li>b</ol>"),
            "9223372036854775807. a\n9223372036854775807. b\n"
        );
        assert_eq!(
            to_text("<p>Intro</p><ul><li><p>one</p></li><li><p>two</p><p>more</p></li></ul>"),
            "Intro\n\n- one\n- two\n  more\n"
        );
    }

    #[test]
    fn word_lists_become_lists() {
        let html = r#"<p class=MsoNormal>Intro</p>
            <p class=MsoListParagraphCxSpFirst style='text-indent:-18.0pt;mso-list:l0 level1 lfo1'><![if !supportLists]><span
            style='font-family:Symbol'><span style='mso-list:Ignore'>·<span style='font:7.0pt "Times New Roman"'>&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
            </span></span></span><![endif]>First</p>
            <p class=MsoListParagraphCxSpMiddle style='mso-list:l0 level2 lfo1'><![if !supportLists]><span
            style='mso-list:Ignore'>o<span>&nbsp;&nbsp; </span></span><![endif]>Nested</p>
            <p class=MsoListParagraphCxSpLast style='mso-list:l1 level1 lfo2'><![if !supportLists]><span
            style='mso-list:Ignore'>1.<span>&nbsp;&nbsp;&nbsp; </span></span><![endif]>Numbered</p>
            <p class=MsoNormal><![if !supportLists]>lost<br>marker</p>
            <p class=MsoNormal><![if !supportLists]>one<![if !supportLists]>two<![endif]>three</p>
            <p class=MsoNormal>End</p>"#;
        assert_eq!(
            to_text(html),
            "Intro\n- First\n  - Nested\n1. Numbered\nlost\nmarker\nonetwo three\nEnd\n"
        );
    }

    #[test]
    fn table_rows_are_lines_and_empty_cells_vanish() {
        let html = "<table><tr><td>Item</td><td>Price</td></tr>\
                    <tr><td>&nbsp;</td><td>Scarf</td><td>12 €</td><td></td></tr>\
                    <tr><td><table><tr><td><p>Nested</p></td></tr></table></td></tr></table>";
        assert_eq!(to_text(html), "Item · Price\nScarf · 12 €\n\nNested\n");
    }

    #[test]
    fn tables_with_headers_keep_their_empty_cells() {
        let html = "<table><tr><th></th><th>Qty</th><th>Price</th></tr>\
                    <tr><td>Scarf</td><td></td><td>12 €</td></tr>\
                    <tr><td>Hat</td><td>2</td><td>&nbsp;</td></tr>\
                    <tr><td></td><td></td><td></td></tr>\
                    <tr><td>Shop</td><td></td>\
                    <td><a href=\"https://example.com/\">example.com</a></td></tr>\
                    <tr><td><p>Note</p></td><td>late</td></tr></table>";
        assert_eq!(
            to_text(html),
            "· Qty · Price\nScarf · · 12 €\nHat · 2\nShop · · https://example.com/\n\nNote\n\nlate\n"
        );
    }

    #[test]
    fn an_empty_block_of_a_set_height_is_a_blank_line() {
        let html = "<table><tr><td><div>Title</div></td></tr>\
                    <tr><td style=\"font-size:0px\">\
                    <div style=\"height:30px;line-height:30px;\">&#8202;</div></td></tr>\
                    <tr><td><div>Hello,</div></td></tr>\
                    <tr><td height=\"20\" style=\"line-height: 20px\"></td></tr>\
                    <tr><td style=\"height:30px;line-height:30px\">X</td>\
                    <td style=\"height:30px;line-height:30px\">Y</td></tr>\
                    <tr><td style=\"height:0;line-height:0\"></td></tr>\
                    <tr><td style=\"height:30px\"></td></tr>\
                    <tr><td>Side</td><td height=\"100\" style=\"line-height:1px\"></td></tr>\
                    <tr><td>End</td></tr></table>";
        assert_eq!(to_text(html), "Title\n\nHello,\n\nX · Y\nSide\nEnd\n");
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
        assert_eq!(
            to_text("\u{a0}<a href=\"https://example.com/\"><td>\u{e4}</td></a>"),
            "\u{e4} <https://example.com/>\n"
        );
        assert_eq!(
            to_text("\u{a0}<a href=\"https://example.com/\"><td>x</td></a>"),
            "x <https://example.com/>\n"
        );
        assert_eq!(
            to_text("<a href=\"https://example.com/x\">https://example.com/x </a>"),
            "https://example.com/x\n"
        );
        assert_eq!(
            to_text("Visit <a href=\"https://example.com/x\">\n https://example.com/x\n</a> today"),
            "Visit https://example.com/x today\n"
        );
        assert_eq!(
            to_text("<a href=\"https://example.com/x\">shop"),
            "shop <https://example.com/x>\n"
        );
        assert_eq!(to_text("<a href=\"https://example.com/x\"><br></a>"), "");
        assert_eq!(
            to_text(
                "<table><tr><td>E-Mail:</td>\
                 <td><a href=\"mailto:maria@example.com\">maria@example.com</a></td></tr></table>"
            ),
            "E-Mail: · maria@example.com\n"
        );
        assert_eq!(
            to_text(
                "<a href=\"https://example.com/p\"><img src=\"p.png\" alt=\"Scarf\"></a>\
                 <a href=\"https://example.com/q\">Scarf, blue</a>\
                 <a href=\"https://example.com/r\">https://example.com/r</a>next."
            ),
            "Scarf <https://example.com/p> Scarf, blue <https://example.com/q> \
             https://example.com/r next.\n"
        );
    }

    #[test]
    fn images_show_their_alt_text() {
        let html = "<img src=\"https://example.com/photo.jpg\" alt=\"Photo\">\
                    <a href=\"https://example.com/\"><img src=\"logo.png\" alt=\"Example Shop\"></a>\
                    <p><a href=\"https://example.com/fb\"><img src=\"fb.png\"></a></p>\
                    <a href=\"https://example.com/t\"><img src=\"t.gif\" width=\"1\" height=\"1\"></a>\
                    <a href=\"https://example.com/s\"><img src=\"s.gif\" style=\"width: 1px; height: 1px\"></a>\
                    <img src=\"https://track.example.com/p.gif\" width=\"1px\" height=\"1px\" alt=\"x\">";
        assert_eq!(
            to_text(html),
            "[Photo] Example Shop <https://example.com/>\n\nhttps://example.com/fb\n"
        );
        assert_eq!(
            to_text("<p>See:</p><img src=\"cid:a\" alt=\"Screenshot 1\"><p>Bye</p>"),
            "See:\n\n[Screenshot 1]\n\nBye\n"
        );
        assert_eq!(
            to_text(
                "Thanks <img src=\"e.png\" alt=\"\u{1f60a}\"><img src=\"a.png\" alt=\"\"><img src=\"b.png\">"
            ),
            "Thanks \u{1f60a}\n"
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
    fn only_styles_that_hide_everywhere_hide() {
        let html = "<div style=\"opacity: 0\">a</div>\
                    <div style=\"max-height: 0px !important; overflow: hidden\">b</div>\
                    <div style=\"max-height: 0\">c</div>\
                    <div style=\"font-size: 0\"><div style=\"font-size: 14px\">d</div></div>\
                    <span style=\"mso-hide: all\">e</span> <span style=\"opacity: 0.5\">f</span>\
                    <div style=\"display: none; display: block\">g</div>\
                    <div style=\"display: block; display: none\">h</div>";
        assert_eq!(to_text(html), "c\nd\ne f\ng\n");
    }

    #[test]
    fn nesting_is_limited() {
        assert_eq!(
            to_text(&format!("{}x", "<blockquote>".repeat(100))),
            format!("{}x\n", "> ".repeat(32))
        );
        assert_eq!(
            to_text(&format!("{}x", "<ul><li>".repeat(100))),
            format!("{}- x\n", " ".repeat(62))
        );
        assert_eq!(
            to_text(&format!("{}<ul><li>x", "<blockquote>".repeat(40))),
            format!("{}- x\n", "> ".repeat(32))
        );
        assert_eq!(
            to_text(&format!("{}x{}y", "<b>".repeat(2000), "</b>".repeat(2000))),
            "xy\n"
        );
    }

    #[test]
    fn struck_and_raised_text_keeps_its_meaning() {
        assert_eq!(
            to_text(
                "<p>Price: <s>100 EUR</s> 80 EUR, \
                 <span style=\"text-decoration: line-through\">old</span> new, a <del> </del> b</p>"
            ),
            "Price: ~~100 EUR~~ 80 EUR, ~~old~~ new, a b\n"
        );
        assert_eq!(
            to_text("<del><p>one</p><p>two</p></del>"),
            "~~one~~\n\n~~two~~\n"
        );
        assert_eq!(
            to_text(
                "<s><a href=\"https://example.com/x\">https://example.com/x</a></s> \
                 <a href=\"https://example.com/y\"><s>old</s></a>"
            ),
            "~~https://example.com/x~~ ~~old~~ <https://example.com/y>\n"
        );
        assert_eq!(
            to_text(
                "20 m<sup>2&#8203;</sup>, 10<sup>-6</sup>, H<sub>2</sub>O, 1<sup>st</sup>, \
                 Brand<sup>\u{ae}</sup>, <sub>valid until 2026</sub>"
            ),
            "20 m\u{b2}, 10\u{207b}\u{2076}, H\u{2082}O, 1st, Brand\u{ae}, valid until 2026\n"
        );
    }

    #[test]
    fn signatures_get_their_separator() {
        let text = to_text("<div>Text</div><div class=\"gmail_signature\">Maria<br>Example</div>");
        assert_eq!(text, "Text\n\n--\nMaria\nExample\n");
        assert_eq!(
            parts(&text),
            vec![
                Part::Text {
                    text: "Text".to_string()
                },
                Part::Signature {
                    text: "Maria\nExample".to_string()
                },
            ]
        );
        assert_eq!(
            to_text(
                "<div>Text</div><span class=\"gmail_signature_prefix\">-- </span><br>\
                 <div dir=\"ltr\" class=\"gmail_signature\">Maria</div>"
            ),
            "Text\n--\nMaria\n"
        );
        assert_eq!(
            to_text("<div>Text</div><div class=\"moz-signature\">-- <br>Paul</div>"),
            "Text\n--\nPaul\n"
        );
        assert_eq!(
            to_text("<div>Text</div><div id=\"Signature\"></div><div>More</div>"),
            "Text\nMore\n"
        );
        assert_eq!(
            to_text("<blockquote><div>Old</div><div id=\"x_Signature\">Jane</div></blockquote>"),
            "> Old\n>\n> --\n> Jane\n"
        );
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

    #[test]
    fn outlook_replies_become_quotes() {
        let word = "<p class=MsoNormal>Fine.</p>\
                    <div><div style='border:none;border-top:solid #E1E1E1 1.0pt;padding:3.0pt 0cm 0cm 0cm'>\
                    <p class=MsoNormal><b>From:</b> Maria &lt;maria@example.com&gt;<br>\
                    <b>Subject:</b> Date</p></div></div><p class=MsoNormal>Friday?</p>";
        let text = to_text(word);
        assert_eq!(
            text,
            "Fine.\n\n> From: Maria <maria@example.com>\n> Subject: Date\n> Friday?\n"
        );
        assert_eq!(
            parts(&text),
            vec![
                Part::Text {
                    text: "Fine.".to_string()
                },
                Part::Quote {
                    attribution: None,
                    parts: vec![Part::Text {
                        text: "From: Maria <maria@example.com>\nSubject: Date\nFriday?".to_string()
                    }],
                },
            ]
        );
        let web = "<div>Fine.</div><hr style=\"display:inline-block;width:98%\" tabindex=\"-1\">\
                   <div id=\"divRplyFwdMsg\" dir=\"ltr\"><b>From:</b> Maria<br><b>Subject:</b> Date</div>\
                   <div>Friday?</div>";
        assert_eq!(
            to_text(web),
            "Fine.\n\n> From: Maria\n> Subject: Date\n> Friday?\n"
        );
        let inside = "<blockquote type=\"cite\"><div>Reply</div>\
                      <div id=\"x_divRplyFwdMsg\">From: Paul</div><div>Older</div></blockquote>\
                      <div>After</div>";
        assert_eq!(
            to_text(inside),
            "> Reply\n>\n> > From: Paul\n> > Older\n\nAfter\n"
        );
    }

    #[test]
    fn an_indented_block_is_no_quote() {
        let html = "<p>Agenda:</p>\
                    <blockquote style=\"margin: 0 0 0 40px; border: none; padding: 0px\">Point</blockquote>\
                    <blockquote style=\"border:none;border-left:solid #CCCCCC 1.0pt\">Quoted</blockquote>\
                    <blockquote type=\"cite\" style=\"border: none\">Cited</blockquote>";
        assert_eq!(to_text(html), "Agenda:\n\nPoint\n\n> Quoted\n>\n> Cited\n");
    }
}
