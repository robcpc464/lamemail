use std::collections::HashSet;
use std::convert::Infallible;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use futures::channel::mpsc::{self, UnboundedSender};
use jiff::{SignedDuration, Timestamp};
use serde::Serialize;
use tokio::task::JoinSet;
use tokio::time::Instant;

use crate::i18n::{fill, t};
use crate::imap::{self, CredentialError};
use crate::kind::Kind;
use crate::message::{self, Addr, MailFile};
use crate::server::{self, Server};
use crate::settings::{self, Account};
use crate::show::IdOnly;
use crate::sync::{self, Counts, Progress, Report, Scope};
use crate::{list, output, show, store};

const IDLE_LIMIT: Duration = Duration::from_secs(5 * 60);
const FULL_EVERY: Duration = Duration::from_secs(15 * 60);
const FIRST_RETRY: Duration = Duration::from_secs(30);
const LAST_RETRY: Duration = Duration::from_secs(5 * 60);
const CLOCK_SKEW: SignedDuration = SignedDuration::from_mins(5);

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Line {
    Syncing {
        account: String,
        syncing: bool,
    },
    Synced {
        account: String,
        #[serde(flatten)]
        counts: Counts,
        arrived: Vec<Arrival>,
    },
    Failed {
        account: String,
        error: String,
        retry: Option<u64>,
    },
    Progress(Progress),
}

#[derive(Debug, Serialize)]
pub struct Arrival {
    pub path: String,
    pub from: Vec<Addr>,
    pub subject: String,
}

pub async fn run(dir: &Path, dev: Option<&Path>, ids: Vec<String>, json: bool) -> Result<()> {
    let accounts = settings::load(dir)?.chosen(ids)?;
    let (lines, mut received) = mpsc::unbounded();
    let mut watches = JoinSet::new();
    for (id, account) in accounts {
        watches.spawn(Watch::new(dir, dev, id, account, lines.clone()).run());
    }
    drop(lines);
    while let Some(line) = received.next().await {
        output::stdout(|out| {
            if json {
                output::json_line(out, &line)
            } else if let Some(text) = text(&line) {
                writeln!(out, "{text}")
            } else {
                Ok(())
            }
        })?;
    }
    bail!("{}", t().watch_ended)
}

fn text(line: &Line) -> Option<String> {
    let text = match line {
        Line::Syncing { .. } | Line::Progress(_) => return None,
        Line::Synced {
            account,
            counts,
            arrived,
        } => {
            let mut text = fill(
                t().folder_summary,
                &[account, &counts.new, &counts.removed, &counts.updated],
            );
            for mail in arrived {
                text.push('\n');
                text.push_str(&fill(
                    t().watch_arrived,
                    &[account, &show::people(&mail.from), &mail.subject],
                ));
            }
            text
        }
        Line::Failed {
            account,
            error,
            retry: Some(seconds),
        } => fill(t().watch_retry, &[account, error, seconds]),
        Line::Failed {
            account,
            error,
            retry: None,
        } => fill(t().watch_stopped, &[account, error]),
    };
    Some(text)
}

struct Watch {
    dir: PathBuf,
    dev: Option<PathBuf>,
    id: String,
    account: Account,
    lines: UnboundedSender<Line>,
    last_sync: Option<Timestamp>,
    retry: Duration,
}

impl Watch {
    fn new(
        dir: &Path,
        dev: Option<&Path>,
        id: String,
        account: Account,
        lines: UnboundedSender<Line>,
    ) -> Watch {
        Watch {
            dir: dir.to_path_buf(),
            dev: dev.map(Path::to_path_buf),
            id,
            account,
            lines,
            last_sync: None,
            retry: FIRST_RETRY,
        }
    }

    async fn run(mut self) {
        loop {
            let Err(err) = self.session().await;
            let stopped = err.is::<CredentialError>();
            self.send(Line::Failed {
                account: self.id.clone(),
                error: format!("{err:#}"),
                retry: (!stopped).then_some(self.retry.as_secs()),
            });
            if stopped {
                return;
            }
            tokio::time::sleep(self.retry).await;
            self.retry = (self.retry * 2).min(LAST_RETRY);
        }
    }

    async fn session(&mut self) -> Result<Infallible> {
        let access = server::access(self.dev.as_deref(), &self.id)?;
        self.sync_folder_free().await?;
        let mut server = Server::connect(&self.account, access).await?;
        self.sync(&mut server, Scope::All).await?;
        self.retry = FIRST_RETRY;
        let mut full = Instant::now();
        loop {
            let due = FULL_EVERY.saturating_sub(full.elapsed());
            let (next, changed) = server
                .wait_for_change(imap::INBOX, IDLE_LIMIT.min(due))
                .await?;
            server = next;
            if full.elapsed() >= FULL_EVERY {
                self.sync(&mut server, Scope::All).await?;
                full = Instant::now();
            } else if changed {
                self.sync(&mut server, Scope::Inbox).await?;
            }
        }
    }

    async fn sync_folder_free(&self) -> Result<()> {
        self.lock().await.map(drop)
    }

    async fn lock(&self) -> Result<File> {
        let dir = self.dir.clone();
        tokio::task::spawn_blocking(move || store::wait_for_lock(&dir))
            .await
            .context("waiting for the sync folder")?
    }

    async fn sync(&mut self, server: &mut Server, scope: Scope) -> Result<()> {
        let started = Timestamp::now();
        let _lock = self.lock().await?;
        self.send(Line::Syncing {
            account: self.id.clone(),
            syncing: true,
        });
        let lines = self.lines.clone();
        let report = Report::Progress(Arc::new(move |progress| {
            let _ = lines.unbounded_send(Line::Progress(progress));
        }));
        let synced = sync::sync_folders(server, &self.dir, &self.id, scope, &report).await?;
        let arrived = match self.last_sync.replace(started) {
            Some(since) => arrivals(&self.dir, &self.id, &synced.added, since)?,
            None => Vec::new(),
        };
        self.send(Line::Synced {
            account: self.id.clone(),
            counts: synced.counts,
            arrived,
        });
        Ok(())
    }

    fn send(&self, line: Line) {
        let _ = self.lines.unbounded_send(line);
    }
}

fn arrivals(dir: &Path, id: &str, added: &[PathBuf], since: Timestamp) -> Result<Vec<Arrival>> {
    if added.is_empty() {
        return Ok(Vec::new());
    }
    let settings = &settings::load(dir)?;
    let mine = show::identities(dir, settings, id)?.addresses;
    let oldest = since - CLOCK_SKEW;
    let mut fresh = Vec::new();
    for file in added {
        let located = list::locate_one(dir, settings, Path::new(&store::relative(file, dir)))?;
        let kind = located.kind;
        if matches!(kind, None | Some(Kind::Junk | Kind::Trash)) {
            continue;
        }
        let Some(mail) = store::read_or_skip::<MailFile>(file) else {
            continue;
        };
        let recent = mail
            .received
            .parse::<Timestamp>()
            .is_ok_and(|received| received >= oldest);
        if recent && !mail.flags.seen && !show::is_own(&mail, kind, &mine) {
            fresh.push((located.path, kind, mail));
        }
    }
    if fresh.is_empty() {
        return Ok(Vec::new());
    }
    let added: HashSet<String> = added
        .iter()
        .map(|file| store::relative(file, dir))
        .collect();
    let from = store::stamp(&message::local_zoned(oldest.as_second()));
    let known: HashSet<String> = list::locate_account(dir, settings, id)?
        .into_iter()
        .filter(|located| located.time >= from && !added.contains(&located.path))
        .filter_map(|located| store::read_or_skip::<IdOnly>(&located.file)?.message_id)
        .collect();
    Ok(list::one_copy(fresh, |(_, kind, mail)| {
        Some((mail.message_id.clone()?, *kind))
    })
    .into_iter()
    .filter(|(_, _, mail)| {
        mail.message_id
            .as_ref()
            .is_none_or(|id| !known.contains(id))
    })
    .map(|(path, _, mail)| Arrival {
        path,
        from: mail.from,
        subject: mail.subject,
    })
    .collect())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::SystemTime;

    use futures::channel::mpsc::UnboundedReceiver;

    use super::*;
    use crate::server::Access;
    use crate::testing::Fixture;

    const PLAIN: &[u8] = include_bytes!("../tests/fixtures/plain.eml");
    const PATIENCE: Duration = Duration::from_secs(10);

    fn start(fixture: &Fixture) -> (tokio::task::JoinHandle<()>, UnboundedReceiver<Line>) {
        let (sender, lines) = mpsc::unbounded();
        let watch = Watch::new(
            &fixture.dir,
            Some(&fixture.dev),
            "private".to_string(),
            fixture.account.clone(),
            sender,
        );
        (tokio::spawn(watch.run()), lines)
    }

    async fn next(lines: &mut UnboundedReceiver<Line>) -> Line {
        tokio::time::timeout(PATIENCE, lines.next())
            .await
            .expect("no line in time")
            .expect("the watch ended")
    }

    async fn synced(lines: &mut UnboundedReceiver<Line>) -> (Counts, Vec<Arrival>) {
        let started = next(lines).await;
        assert!(
            matches!(started, Line::Syncing { syncing: true, .. }),
            "{started:?}"
        );
        loop {
            match next(lines).await {
                Line::Progress(_) => {}
                Line::Synced {
                    counts, arrived, ..
                } => return (counts, arrived),
                failed => panic!("{failed:?}"),
            }
        }
    }

    fn mail(message_id: &str) -> String {
        String::from_utf8_lossy(PLAIN).replace("a1b2c3@example.com", message_id)
    }

    #[tokio::test]
    async fn a_new_mail_is_announced_once_and_a_copy_of_a_known_one_never() {
        let fixture = Fixture::new("watch-copies");
        fixture.sync().await;
        let archive = fixture.marked_folder("Archive", "archive");
        fs::write(archive.join("1.eml"), PLAIN).unwrap();
        fs::write(archive.join("2.eml"), mail("copy@example.com")).unwrap();
        fs::write(
            fixture.dev.join("private/INBOX/2.eml"),
            mail("copy@example.com"),
        )
        .unwrap();
        let synced = sync::sync_account(
            &fixture.dir,
            "private",
            &fixture.account,
            Access::Dev(fixture.dev.join("private")),
            &Report::Quiet,
        )
        .await
        .unwrap();
        assert_eq!(synced.added.len(), 3);
        let since = Timestamp::now() - SignedDuration::from_hours(1);
        let arrived = arrivals(&fixture.dir, "private", &synced.added, since).unwrap();
        assert_eq!(arrived.len(), 1);
        assert!(arrived[0].path.starts_with("private/inbox/"));
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[tokio::test]
    async fn the_inbox_is_mirrored_as_soon_as_it_changes_and_new_unread_mail_is_reported() {
        let fixture = Fixture::new("watch");
        let (watch, mut lines) = start(&fixture);
        let (counts, arrived) = synced(&mut lines).await;
        assert_eq!(counts.new, 1);
        assert!(arrived.is_empty());

        let inbox = fixture.dev.join("private/INBOX");
        let moved = fixture.base.join("4.eml");
        fs::write(&moved, mail("old@example.com")).unwrap();
        fs::File::options()
            .write(true)
            .open(&moved)
            .unwrap()
            .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000))
            .unwrap();
        fs::write(
            fixture.dev.join("private/Sent/5.eml"),
            mail("sent@example.com"),
        )
        .unwrap();
        fs::write(inbox.join("2.eml"), mail("new@example.com")).unwrap();
        fs::write(inbox.join("3,S.eml"), mail("read@example.com")).unwrap();
        fs::rename(&moved, inbox.join("4.eml")).unwrap();

        let mut new = 0;
        let mut arrived = Vec::new();
        while new < 3 {
            let (counts, more) = synced(&mut lines).await;
            new += counts.new;
            arrived.extend(more);
        }
        watch.abort();
        assert_eq!(arrived.len(), 1);
        assert!(arrived[0].path.starts_with("private/inbox/"));
        assert!(arrived[0].path.ends_with("-offer-for-project-2.json"));
        assert_eq!(arrived[0].subject, "Offer for project");
        assert_eq!(
            arrived[0].from[0].address.as_deref(),
            Some("jane@example.com")
        );
        assert!(
            store::index(&fixture.dir.join("private/sent"))
                .unwrap()
                .is_empty()
        );
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[tokio::test]
    async fn a_failure_is_reported_with_the_next_try() {
        let fixture = Fixture::new("watch-failure");
        fs::remove_dir_all(fixture.dev.join("private")).unwrap();
        let (watch, mut lines) = start(&fixture);
        let line = next(&mut lines).await;
        watch.abort();
        assert!(matches!(
            line,
            Line::Failed {
                retry: Some(30),
                ..
            }
        ));
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[test]
    fn lines_carry_the_counts_and_arrivals_or_the_error() {
        let synced = Line::Synced {
            account: "private".to_string(),
            counts: Counts {
                new: 1,
                removed: 0,
                updated: 2,
            },
            arrived: vec![Arrival {
                path: "private/inbox/2026-09-15_1432-offer-for-project-4711.json".to_string(),
                from: vec![Addr {
                    name: Some("Jane Doe".to_string()),
                    address: Some("jane@example.com".to_string()),
                }],
                subject: "Offer for project".to_string(),
            }],
        };
        assert_eq!(
            serde_json::to_value(&synced).unwrap(),
            serde_json::json!({
                "account": "private", "new": 1, "removed": 0, "updated": 2,
                "arrived": [{
                    "path": "private/inbox/2026-09-15_1432-offer-for-project-4711.json",
                    "from": [{ "name": "Jane Doe", "address": "jane@example.com" }],
                    "subject": "Offer for project"
                }]
            })
        );
        let syncing = Line::Syncing {
            account: "private".to_string(),
            syncing: true,
        };
        assert_eq!(
            serde_json::to_value(&syncing).unwrap(),
            serde_json::json!({ "account": "private", "syncing": true })
        );
        let stopped = Line::Failed {
            account: "work".to_string(),
            error: "login as work@example.com failed: wrong password".to_string(),
            retry: None,
        };
        assert_eq!(
            serde_json::to_value(&stopped).unwrap(),
            serde_json::json!({
                "account": "work",
                "error": "login as work@example.com failed: wrong password",
                "retry": null
            })
        );
    }
}
