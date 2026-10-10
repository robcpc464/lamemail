use std::path::Path;

use anyhow::Result;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::i18n::{fill, t};
use crate::journal::{self, Action};
use crate::message::{self, Flags};
use crate::{list, settings, store};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Change {
    Seen,
    Unseen,
    Flagged,
    Unflagged,
    Answered,
    Forwarded,
    Junk,
    #[value(name = "notjunk")]
    NotJunk,
}

impl Change {
    pub const ALL: [Change; 8] = [
        Change::Seen,
        Change::Unseen,
        Change::Flagged,
        Change::Unflagged,
        Change::Answered,
        Change::Forwarded,
        Change::Junk,
        Change::NotJunk,
    ];

    pub fn queries(self) -> &'static [&'static str] {
        match self {
            Change::Seen => &["+FLAGS.SILENT (\\Seen)"],
            Change::Unseen => &["-FLAGS.SILENT (\\Seen)"],
            Change::Flagged => &["+FLAGS.SILENT (\\Flagged)"],
            Change::Unflagged => &["-FLAGS.SILENT (\\Flagged)"],
            Change::Answered => &["+FLAGS.SILENT (\\Answered)"],
            Change::Forwarded => &["+FLAGS.SILENT ($Forwarded)"],
            Change::Junk => &["+FLAGS.SILENT ($Junk)", "-FLAGS.SILENT ($NotJunk)"],
            Change::NotJunk => &["+FLAGS.SILENT ($NotJunk)", "-FLAGS.SILENT ($Junk)"],
        }
    }

    pub fn apply(self, flags: &mut Flags) {
        match self {
            Change::Seen | Change::Unseen => flags.seen = self == Change::Seen,
            Change::Flagged | Change::Unflagged => flags.flagged = self == Change::Flagged,
            Change::Answered => flags.answered = true,
            Change::Forwarded => flags.set(message::FORWARDED, true),
            Change::Junk | Change::NotJunk => {
                flags.set(message::JUNK, self == Change::Junk);
                flags.set(message::NOT_JUNK, self == Change::NotJunk);
            }
        }
    }

    pub fn is_keyword(self) -> bool {
        matches!(self, Change::Forwarded | Change::Junk | Change::NotJunk)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Change::Seen => "seen",
            Change::Unseen => "unseen",
            Change::Flagged => "flagged",
            Change::Unflagged => "unflagged",
            Change::Answered => "answered",
            Change::Forwarded => "forwarded",
            Change::Junk => "junk",
            Change::NotJunk => "notjunk",
        }
    }

    pub fn same_flag(self, other: Change) -> bool {
        self.flag() == other.flag()
    }

    fn flag(self) -> &'static str {
        match self {
            Change::Seen | Change::Unseen => "\\Seen",
            Change::Flagged | Change::Unflagged => "\\Flagged",
            Change::Answered => "\\Answered",
            Change::Forwarded => message::FORWARDED,
            Change::Junk | Change::NotJunk => message::JUNK,
        }
    }

    fn done(self) -> &'static str {
        match self {
            Change::Seen => t().flag_seen,
            Change::Unseen => t().flag_unseen,
            Change::Flagged => t().flag_flagged,
            Change::Unflagged => t().flag_unflagged,
            Change::Answered => t().flag_answered,
            Change::Forwarded => t().flag_forwarded,
            Change::Junk => t().flag_junk,
            Change::NotJunk => t().flag_not_junk,
        }
    }
}

pub fn run(dir: &Path, path: &Path, change: Change) -> Result<()> {
    let settings = settings::load(dir)?;
    let located = list::locate_one(dir, &settings, path)?;
    store::read_flags(&located.file)?;
    let state = store::state_of(&located.file)?;
    journal::record(
        dir,
        &located.path,
        state.uidvalidity,
        Action::Flag { change },
    )?;
    println!("{}", fill(change.done(), &[&located.path]));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_sets_or_clears_one_flag() {
        let mut flags = Flags {
            answered: true,
            ..Flags::default()
        };
        Change::Flagged.apply(&mut flags);
        Change::Seen.apply(&mut flags);
        Change::Unseen.apply(&mut flags);
        assert!(flags.flagged && flags.answered && !flags.seen);
        assert_eq!(Change::Unflagged.queries(), ["-FLAGS.SILENT (\\Flagged)"]);
        assert!(Change::Seen.same_flag(Change::Unseen));
        assert!(!Change::Seen.same_flag(Change::Flagged));
        assert!(Change::Junk.same_flag(Change::NotJunk));
        assert!(!Change::Answered.same_flag(Change::Forwarded));
    }

    #[test]
    fn keywords_mark_forwarded_and_junk_mail() {
        let mut flags = Flags::default();
        Change::Answered.apply(&mut flags);
        Change::Forwarded.apply(&mut flags);
        Change::Forwarded.apply(&mut flags);
        Change::NotJunk.apply(&mut flags);
        Change::Junk.apply(&mut flags);
        assert!(flags.answered);
        assert_eq!(flags.keywords, ["$Forwarded", "$Junk"]);
        assert!(flags.has("$forwarded") && !flags.has("$NotJunk"));
        assert!(Change::Junk.is_keyword() && !Change::Answered.is_keyword());
        assert_eq!(
            Change::Junk.queries(),
            ["+FLAGS.SILENT ($Junk)", "-FLAGS.SILENT ($NotJunk)"]
        );
    }
}
