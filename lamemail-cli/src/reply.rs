use serde::Serialize;

pub const SIGNATURE_SEPARATOR: &str = "--";
pub const MAX_QUOTE_DEPTH: usize = 32;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Part {
    Text {
        text: String,
    },
    Signature {
        text: String,
    },
    Quote {
        attribution: Option<String>,
        parts: Vec<Part>,
    },
}

pub fn text(body: &str) -> String {
    match closing(body).0.trim() {
        "" => body.trim().to_string(),
        kept => kept.to_string(),
    }
}

pub fn closing(text: &str) -> (&str, &str) {
    let lines: Vec<&str> = text.lines().collect();
    let quote = without_trailing_quote(&lines);
    let signature = without_signature(&lines[..quote]);
    let mut start = without_trailing_quote(&lines[..signature]);
    while lines.get(start).is_some_and(|line| line.trim().is_empty()) {
        start += 1;
    }
    let at = text.split_inclusive('\n').take(start).map(str::len).sum();
    text.split_at(at)
}

pub fn parts(body: &str) -> Vec<Part> {
    nested(body, 0)
}

fn nested(body: &str, depth: usize) -> Vec<Part> {
    let nests = |line: &str| depth < MAX_QUOTE_DEPTH && is_quoted(line);
    let lines: Vec<&str> = body.lines().collect();
    let mut parts = Vec::new();
    let mut plain: Vec<&str> = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if !nests(lines[index]) {
            plain.push(lines[index]);
            index += 1;
            continue;
        }
        let start = index;
        let mut end = index;
        while index < lines.len() {
            if nests(lines[index]) {
                end = index + 1;
            } else if !(lines[index].trim().is_empty()
                && lines.get(index + 1).is_some_and(|next| nests(next)))
            {
                break;
            }
            index += 1;
        }
        let attribution = take_attribution(&mut plain);
        push_plain(&mut parts, &plain);
        plain.clear();
        let quoted: Vec<&str> = lines[start..end].iter().map(|line| unquote(line)).collect();
        parts.push(Part::Quote {
            attribution,
            parts: nested(&quoted.join("\n"), depth + 1),
        });
        index = end;
    }
    push_plain(&mut parts, &plain);
    parts
}

fn is_quoted(line: &str) -> bool {
    line.trim_start().starts_with('>')
}

fn unquote(line: &str) -> &str {
    let rest = line.trim_start().strip_prefix('>').unwrap_or(line);
    rest.strip_prefix(' ').unwrap_or(rest)
}

fn take_attribution(plain: &mut Vec<&str>) -> Option<String> {
    let index = plain.iter().rposition(|line| !line.trim().is_empty())?;
    if !is_attribution(plain[index]) {
        return None;
    }
    let attribution = plain[index]
        .trim()
        .trim_end_matches(':')
        .trim_end()
        .to_string();
    plain.truncate(index);
    Some(attribution)
}

fn push_plain(parts: &mut Vec<Part>, lines: &[&str]) {
    let split = without_signature(lines);
    if let Some(text) = joined(&lines[..split]) {
        parts.push(Part::Text { text });
    }
    if split < lines.len()
        && let Some(text) = joined(&lines[split + 1..])
    {
        parts.push(Part::Signature { text });
    }
}

fn joined(lines: &[&str]) -> Option<String> {
    let first = lines.iter().position(|line| !line.trim().is_empty())?;
    let last = lines.iter().rposition(|line| !line.trim().is_empty())?;
    let text: Vec<&str> = lines[first..=last]
        .iter()
        .map(|line| line.trim_end())
        .collect();
    Some(text.join("\n"))
}

fn without_trailing_quote(lines: &[&str]) -> usize {
    let mut cut = lines.len();
    let mut quoted = false;
    while cut > 0 {
        let line = lines[cut - 1].trim_start();
        if line.starts_with('>') {
            quoted = true;
        } else if !line.is_empty() {
            break;
        }
        cut -= 1;
    }
    if !quoted {
        return lines.len();
    }
    let before = lines[..cut]
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .filter(|&index| is_attribution(lines[index]));
    before.unwrap_or(cut)
}

fn is_attribution(line: &str) -> bool {
    line.trim().ends_with(':')
}

fn without_signature(lines: &[&str]) -> usize {
    lines
        .iter()
        .rposition(|line| line.trim_end() == SIGNATURE_SEPARATOR)
        .unwrap_or(lines.len())
}

#[cfg(test)]
mod tests {
    use super::{MAX_QUOTE_DEPTH, Part, closing, parts, text};

    fn plain(text: &str) -> Part {
        Part::Text {
            text: text.to_string(),
        }
    }

    fn signature(text: &str) -> Part {
        Part::Signature {
            text: text.to_string(),
        }
    }

    fn quote(attribution: Option<&str>, parts: Vec<Part>) -> Part {
        Part::Quote {
            attribution: attribution.map(str::to_string),
            parts,
        }
    }

    #[test]
    fn keeps_a_plain_body_trimmed() {
        assert_eq!(
            text("Hi,\n\nsee attached.\n\nJane\n"),
            "Hi,\n\nsee attached.\n\nJane"
        );
        assert_eq!(text(""), "");
    }

    #[test]
    fn drops_a_trailing_quote_with_its_attribution() {
        let body =
            "Sure, Saturday.\n\nAm 15.09.2026 um 14:32 schrieb Jane Doe:\n> Weekend?\n>\n> Jane\n";
        assert_eq!(text(body), "Sure, Saturday.");
        let body = "Sounds good.\n\nOn Tue, 15 Sep 2026, Jane Doe <jane@example.com> wrote:\n\n> Weekend?\n";
        assert_eq!(text(body), "Sounds good.");
    }

    #[test]
    fn any_language_ends_its_attribution_with_a_colon() {
        let body = "Merci.\n\nLe lundi 15 sept. 2026, Jean a écrit :\n> Bonjour\n";
        assert_eq!(text(body), "Merci.");
        let body = "Here are the numbers:\n> 1\n> 2\n";
        assert_eq!(text(body), body.trim());
    }

    #[test]
    fn drops_the_signature() {
        assert_eq!(text("Ok.\n\n-- \nJane Doe\nExample GmbH\n"), "Ok.");
        assert_eq!(text("Ok.\n--\nSent from my phone\n"), "Ok.");
        let body = "Done.\n\n-- \nJane\n\nOn Monday, Peter wrote:\n> Please do it.\n";
        assert_eq!(text(body), "Done.");
        let body = "Done.\n\nOn Monday, Peter wrote:\n> Please do it.\n\n-- \nJane\n";
        assert_eq!(text(body), "Done.");
    }

    #[test]
    fn keeps_inline_replies_and_a_quote_only_body() {
        let body = "> Saturday?\nYes.\n> Sunday?\nNo.\n";
        assert_eq!(text(body), "> Saturday?\nYes.\n> Sunday?\nNo.");
        let body = "> Forwarded as is.\n> Nothing added.\n";
        assert_eq!(text(body), "> Forwarded as is.\n> Nothing added.");
    }

    #[test]
    fn the_closing_is_the_trailing_quote_and_signature_in_either_order() {
        assert_eq!(
            closing("Hi\n\nAm Mo schrieb Jane:\n> x\n"),
            ("Hi\n\n", "Am Mo schrieb Jane:\n> x\n")
        );
        assert_eq!(
            closing("Hi\n\n-- \nRobin\n\nAm Mo schrieb Jane:\n> x\n"),
            ("Hi\n\n", "-- \nRobin\n\nAm Mo schrieb Jane:\n> x\n")
        );
        assert_eq!(
            closing("Hi\n\nAm Mo schrieb Jane:\n> x\n\n-- \nRobin"),
            ("Hi\n\n", "Am Mo schrieb Jane:\n> x\n\n-- \nRobin")
        );
        assert_eq!(closing("Hi\n> x\nthere"), ("Hi\n> x\nthere", ""));
        assert_eq!(closing("\n \n-- \nRobin"), ("\n \n", "-- \nRobin"));
    }

    #[test]
    fn parts_keep_a_plain_body_as_one_text() {
        assert_eq!(
            parts("Hi,\n\nsee attached.  \n\nJane\n"),
            vec![plain("Hi,\n\nsee attached.\n\nJane")]
        );
        assert!(parts("").is_empty());
        assert!(parts("\n  \n").is_empty());
    }

    #[test]
    fn parts_fold_a_trailing_quote_with_its_attribution() {
        let body =
            "Sure, Saturday.\n\nAm 15.09.2026 um 14:32 schrieb Jane Doe:\n> Weekend?\n>\n> Jane\n";
        assert_eq!(
            parts(body),
            vec![
                plain("Sure, Saturday."),
                quote(
                    Some("Am 15.09.2026 um 14:32 schrieb Jane Doe"),
                    vec![plain("Weekend?\n\nJane")]
                ),
            ]
        );
    }

    #[test]
    fn quotes_nest_only_so_deep() {
        let mut found = parts(&format!("{} deep", ">".repeat(100_000)));
        let mut depth = 0;
        while let Some(Part::Quote { parts, .. }) = found.pop() {
            found = parts;
            depth += 1;
        }
        assert_eq!(depth, MAX_QUOTE_DEPTH);
    }

    #[test]
    fn parts_split_the_signature_and_nest_older_quotes() {
        let body = "Ok.\n\n-- \nPaul Berger\nTel. 1\n\nOn Monday, Jane wrote:\n> Deal?\n> \n> -- \n> Jane\n> \n> Am Sonntag schrieb Paul:\n> > Price?\n";
        assert_eq!(
            parts(body),
            vec![
                plain("Ok."),
                signature("Paul Berger\nTel. 1"),
                quote(
                    Some("On Monday, Jane wrote"),
                    vec![
                        plain("Deal?"),
                        signature("Jane"),
                        quote(Some("Am Sonntag schrieb Paul"), vec![plain("Price?")]),
                    ]
                ),
            ]
        );
    }

    #[test]
    fn parts_alternate_for_inline_replies() {
        assert_eq!(
            parts("> Saturday?\nYes.\n> Sunday?\nNo.\n"),
            vec![
                quote(None, vec![plain("Saturday?")]),
                plain("Yes."),
                quote(None, vec![plain("Sunday?")]),
                plain("No."),
            ]
        );
        assert_eq!(
            parts("> Forwarded as is.\n"),
            vec![quote(None, vec![plain("Forwarded as is.")])]
        );
    }

    #[test]
    fn a_quote_bridges_one_blank_line_but_not_two() {
        assert_eq!(
            parts("> a\n\n> b\n"),
            vec![quote(None, vec![plain("a\n\nb")])]
        );
        assert_eq!(
            parts("> a\n\n\n> b\n"),
            vec![quote(None, vec![plain("a")]), quote(None, vec![plain("b")])]
        );
    }

    #[test]
    fn parts_take_a_colon_line_before_a_quote_as_its_attribution() {
        assert_eq!(
            parts("Le lundi, Jean a écrit :\n\n> Bonjour\n"),
            vec![quote(
                Some("Le lundi, Jean a écrit"),
                vec![plain("Bonjour")]
            )]
        );
        assert_eq!(
            parts("Numbers\n> 1\n> 2\n"),
            vec![plain("Numbers"), quote(None, vec![plain("1\n2")])]
        );
    }
}
