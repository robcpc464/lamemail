use std::collections::BTreeMap;

use anyhow::Result;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::i18n::{fill, t};
use crate::kind::Kind;

pub const MAX_INDENT: u8 = 3;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Icon {
    Incoming,
    Outgoing,
    Drafts,
    Junk,
    Trash,
    Archive,
    #[default]
    Mail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Color {
    Blue,
    Green,
    Magenta,
    Cyan,
    Yellow,
    Orange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Property {
    Icon,
    Dense,
    Dim,
    Strike,
    Italic,
    Indent,
    Color,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Style {
    pub icon: Icon,
    pub dense: bool,
    pub dim: bool,
    pub strike: bool,
    pub italic: bool,
    pub indent: u8,
    pub color: Option<Color>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Overrides {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<Icon>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dense: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dim: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indent: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
}

impl Icon {
    pub fn as_str(self) -> &'static str {
        match self {
            Icon::Incoming => "incoming",
            Icon::Outgoing => "outgoing",
            Icon::Drafts => "drafts",
            Icon::Junk => "junk",
            Icon::Trash => "trash",
            Icon::Archive => "archive",
            Icon::Mail => "mail",
        }
    }
}

impl Color {
    pub fn as_str(self) -> &'static str {
        match self {
            Color::Blue => "blue",
            Color::Green => "green",
            Color::Magenta => "magenta",
            Color::Cyan => "cyan",
            Color::Yellow => "yellow",
            Color::Orange => "orange",
        }
    }

    pub fn nth(position: usize) -> Color {
        let all = Color::value_variants();
        all[position % all.len()]
    }

    pub fn unused(taken: &[Color]) -> Color {
        Color::value_variants()
            .iter()
            .copied()
            .find(|color| !taken.contains(color))
            .unwrap_or_else(|| Color::nth(taken.len()))
    }
}

impl Property {
    pub fn as_str(self) -> &'static str {
        match self {
            Property::Icon => "icon",
            Property::Dense => "dense",
            Property::Dim => "dim",
            Property::Strike => "strike",
            Property::Italic => "italic",
            Property::Indent => "indent",
            Property::Color => "color",
        }
    }
}

impl Style {
    fn apply(mut self, set: &Overrides) -> Self {
        self.icon = set.icon.unwrap_or(self.icon);
        self.dense = set.dense.unwrap_or(self.dense);
        self.dim = set.dim.unwrap_or(self.dim);
        self.strike = set.strike.unwrap_or(self.strike);
        self.italic = set.italic.unwrap_or(self.italic);
        self.indent = set.indent.unwrap_or(self.indent);
        self.color = set.color.or(self.color);
        self
    }

    pub fn flags(&self) -> String {
        let indent = format!("{}={}", Property::Indent.as_str(), self.indent);
        [
            (self.dense, Property::Dense.as_str()),
            (self.dim, Property::Dim.as_str()),
            (self.strike, Property::Strike.as_str()),
            (self.italic, Property::Italic.as_str()),
            (self.indent > 0, indent.as_str()),
        ]
        .into_iter()
        .filter(|(on, _)| *on)
        .map(|(_, name)| name)
        .collect::<Vec<_>>()
        .join(" ")
    }
}

impl Overrides {
    pub fn is_empty(&self) -> bool {
        *self == Overrides::default()
    }

    pub fn set(&mut self, property: Property, value: &str) -> Result<()> {
        let invalid = || invalid(property, value);
        match property {
            Property::Icon => {
                self.icon = Some(Icon::from_str(value, true).map_err(|_| invalid())?);
            }
            Property::Indent => {
                let level = value.parse().ok().filter(|level| *level <= MAX_INDENT);
                self.indent = Some(level.ok_or_else(invalid)?);
            }
            Property::Color => {
                self.color = Some(Color::from_str(value, true).map_err(|_| invalid())?);
            }
            _ => {
                *self.flag(property) = Some(match value {
                    "true" => true,
                    "false" => false,
                    _ => return Err(invalid()),
                });
            }
        }
        Ok(())
    }

    pub fn clear(&mut self, property: Property) {
        match property {
            Property::Icon => self.icon = None,
            Property::Indent => self.indent = None,
            Property::Color => self.color = None,
            _ => *self.flag(property) = None,
        }
    }

    fn flag(&mut self, property: Property) -> &mut Option<bool> {
        match property {
            Property::Dense => &mut self.dense,
            Property::Dim => &mut self.dim,
            Property::Strike => &mut self.strike,
            Property::Italic => &mut self.italic,
            Property::Icon | Property::Indent | Property::Color => unreachable!("not a flag"),
        }
    }
}

fn invalid(property: Property, value: &str) -> anyhow::Error {
    let allowed = match property {
        Property::Icon => Icon::value_variants()
            .iter()
            .map(|icon| icon.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        Property::Indent => (0..=MAX_INDENT)
            .map(|level| level.to_string())
            .collect::<Vec<_>>()
            .join(", "),
        Property::Color => Color::value_variants()
            .iter()
            .map(|color| color.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        _ => "true, false".to_string(),
    };
    anyhow::anyhow!(
        "{}",
        fill(t().invalid_value, &[&value, &property.as_str(), &allowed])
    )
}

pub fn resolve(kind: Option<Kind>, set: &BTreeMap<Kind, Overrides>) -> Style {
    let style = default_for(kind);
    match kind.and_then(|kind| set.get(&kind)) {
        Some(overrides) => style.apply(overrides),
        None => style,
    }
}

pub fn default_for(kind: Option<Kind>) -> Style {
    let plain = Style::default();
    match kind {
        None => plain,
        Some(Kind::Inbox) => Style {
            icon: Icon::Incoming,
            color: Some(Color::Blue),
            ..plain
        },
        Some(Kind::Sent) => Style {
            icon: Icon::Outgoing,
            color: Some(Color::Green),
            ..plain
        },
        Some(Kind::Drafts) => Style {
            icon: Icon::Drafts,
            italic: true,
            color: Some(Color::Yellow),
            ..plain
        },
        Some(Kind::Junk) => Style {
            icon: Icon::Junk,
            dense: true,
            dim: true,
            color: Some(Color::Orange),
            ..plain
        },
        Some(Kind::Trash) => Style {
            icon: Icon::Trash,
            dim: true,
            strike: true,
            color: Some(Color::Cyan),
            ..plain
        },
        Some(Kind::Archive) => Style {
            icon: Icon::Archive,
            color: Some(Color::Magenta),
            ..plain
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_its_look_and_a_folder_without_one_is_plain() {
        assert_eq!(default_for(Some(Kind::Inbox)).icon, Icon::Incoming);
        assert_eq!(default_for(Some(Kind::Inbox)).flags(), "");
        assert_eq!(default_for(Some(Kind::Sent)).icon, Icon::Outgoing);
        assert_eq!(default_for(Some(Kind::Sent)).flags(), "");
        assert_eq!(default_for(Some(Kind::Drafts)).flags(), "italic");
        assert_eq!(default_for(Some(Kind::Junk)).flags(), "dense dim");
        assert_eq!(default_for(Some(Kind::Trash)).flags(), "dim strike");
        assert_eq!(default_for(Some(Kind::Archive)).icon, Icon::Archive);
        assert_eq!(default_for(Some(Kind::Archive)).flags(), "");
        assert_eq!(default_for(None).icon, Icon::Mail);
        assert_eq!(default_for(None).flags(), "");
        assert_eq!(default_for(Some(Kind::Inbox)).color, Some(Color::Blue));
        assert_eq!(default_for(Some(Kind::Sent)).color, Some(Color::Green));
        assert_eq!(default_for(None).color, None);
    }

    #[test]
    fn overrides_apply_on_top_of_the_defaults_of_their_kind() {
        let mut junk = Overrides::default();
        junk.set(Property::Dense, "false").unwrap();
        junk.set(Property::Icon, "mail").unwrap();
        let set = BTreeMap::from([(Kind::Junk, junk)]);
        let style = resolve(Some(Kind::Junk), &set);
        assert_eq!(style.icon, Icon::Mail);
        assert!(!style.dense && style.dim);
        assert_eq!(
            resolve(Some(Kind::Trash), &set),
            default_for(Some(Kind::Trash))
        );
        assert_eq!(resolve(None, &set), default_for(None));
    }

    #[test]
    fn values_are_checked_and_cleared() {
        let mut set = Overrides::default();
        assert!(set.set(Property::Dim, "maybe").is_err());
        assert!(set.set(Property::Icon, "star").is_err());
        assert!(set.set(Property::Icon, "inbox").is_err());
        assert!(set.set(Property::Indent, "true").is_err());
        assert!(set.set(Property::Indent, "4").is_err());
        assert!(set.set(Property::Color, "red").is_err());
        set.set(Property::Color, "orange").unwrap();
        assert_eq!(
            resolve(Some(Kind::Inbox), &BTreeMap::from([(Kind::Inbox, set)])).color,
            Some(Color::Orange)
        );
        set.clear(Property::Color);
        set.set(Property::Indent, "2").unwrap();
        assert_eq!(set.indent, Some(2));
        assert_eq!(
            resolve(Some(Kind::Inbox), &BTreeMap::from([(Kind::Inbox, set)])).flags(),
            "indent=2"
        );
        set.clear(Property::Indent);
        set.set(Property::Italic, "true").unwrap();
        assert!(!set.is_empty());
        set.clear(Property::Italic);
        assert!(set.is_empty());
        assert_eq!(
            serde_json::to_string(&Overrides {
                dense: Some(false),
                ..Overrides::default()
            })
            .unwrap(),
            r#"{"dense":false}"#
        );
    }
}
