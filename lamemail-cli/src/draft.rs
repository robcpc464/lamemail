use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::compose::Original;
use crate::i18n::{fill, t};
use crate::journal::{self, Dropped, Outbound, Pushed};
use crate::kind::{self, Kind, Kinds};
use crate::message::{Attachment, MailFile};
use crate::net::Refused;
use crate::send::Files;
use crate::server::Server;
use crate::settings::Settings;
use crate::store::{self, FolderState};
use crate::{output, send, settings, sync};

pub struct Continued {
    message_id: String,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
    source: Source,
}

enum Source {
    Waiting(Vec<Original>),
    Saved {
        path: String,
        uid: u32,
        attachments: Vec<Attachment>,
    },
    Gone,
}

impl Continued {
    pub fn originals(&self, dir: &Path, indices: &[u32]) -> Result<Vec<Original>> {
        let wanted: BTreeSet<u32> = indices.iter().copied().collect();
        let Some(first) = wanted.first() else {
            return Ok(Vec::new());
        };
        let missing = |index: &u32| fill(t().draft_no_attachment, &[&self.message_id, index]);
        let originals = match &self.source {
            Source::Saved {
                path,
                uid,
                attachments,
            } => return Ok(vec![send::original(dir, path, *uid, attachments, indices)?]),
            Source::Waiting(originals) => originals,
            Source::Gone => bail!("{}", missing(first)),
        };
        let count: usize = originals
            .iter()
            .map(|original| original.attachments.len())
            .sum();
        if let Some(index) = wanted.iter().find(|index| **index as usize >= count) {
            bail!("{}", missing(index));
        }
        let mut positions = 0..;
        Ok(originals
            .iter()
            .filter_map(|original| {
                let attachments: Vec<Attachment> = original
                    .attachments
                    .iter()
                    .zip(&mut positions)
                    .filter(|(_, position)| wanted.contains(position))
                    .map(|(attachment, _)| attachment.clone())
                    .collect();
                (!attachments.is_empty()).then(|| Original {
                    attachments,
                    ..original.clone()
                })
            })
            .collect())
    }
}

pub fn folder(dir: &Path, settings: &Settings, account: &str) -> Result<Option<PathBuf>> {
    Ok(Kinds::of(settings.account(account)?)
        .folder(Kind::Drafts)
        .map(|local| dir.join(account).join(local)))
}

pub fn ensure_folder(dir: &Path, settings: &Settings, account: &str) -> Result<()> {
    if folder(dir, settings, account)?.is_none() {
        bail!("{}", kind::no_folder(account, Kind::Drafts));
    }
    Ok(())
}

pub fn check_message_id(message_id: &str) -> Result<()> {
    if message_id.is_empty()
        || message_id.contains(|c: char| c.is_whitespace() || matches!(c, '<' | '>'))
    {
        bail!("{}", fill(t().draft_invalid, &[&message_id]));
    }
    Ok(())
}

#[derive(Serialize)]
struct Discarded<'a> {
    account: &'a str,
    message_id: &'a str,
    paths: Vec<String>,
}

pub fn run_discard(dir: &Path, account: &str, message_id: &str, json: bool) -> Result<()> {
    let settings = settings::load(dir)?;
    settings.account(account)?;
    check_message_id(message_id)?;
    let (paths, uidvalidity) = match target_in(dir, &settings, account)? {
        Some((folder_dir, state)) => {
            let ids = BTreeSet::from([message_id.to_string()]);
            let paths = saved(&folder_dir, &ids)?
                .into_iter()
                .map(|(_, file, _)| store::relative(&file, dir))
                .collect();
            (paths, state.uidvalidity)
        }
        None => (Vec::new(), 0),
    };
    journal::discard_draft(dir, account, message_id, &paths, uidvalidity)?;
    output::stdout(|out| {
        if json {
            output::json_line(
                out,
                &Discarded {
                    account,
                    message_id,
                    paths,
                },
            )
        } else {
            writeln!(out, "{}", fill(t().draft_discarded, &[&message_id]))
        }
    })
}

pub fn continued(dir: &Path, account: &str, message_id: &str) -> Result<Continued> {
    if let Some(waiting) = journal::waiting_draft(dir, account, message_id)? {
        return Ok(Continued {
            message_id: message_id.to_string(),
            in_reply_to: waiting.in_reply_to,
            references: waiting.references,
            source: Source::Waiting(waiting.originals),
        });
    }
    let newest = match target(dir, account)? {
        Some((folder_dir, _)) => {
            saved(&folder_dir, &BTreeSet::from([message_id.to_string()]))?.pop()
        }
        None => None,
    };
    Ok(match newest {
        Some((_, file, mail)) => Continued {
            message_id: message_id.to_string(),
            in_reply_to: mail.in_reply_to,
            references: mail.references,
            source: Source::Saved {
                path: store::relative(&file, dir),
                uid: mail.uid,
                attachments: mail.attachments,
            },
        },
        None => Continued {
            message_id: message_id.to_string(),
            in_reply_to: None,
            references: Vec::new(),
            source: Source::Gone,
        },
    })
}

pub async fn push(
    server: &mut Server,
    dir: &Path,
    id: &str,
    drafts: Vec<Outbound>,
    files: &mut Files,
    pushed: &mut Pushed,
) -> Result<()> {
    let Some((folder_dir, state)) = target(dir, id)? else {
        return drop_all(dir, drafts, t().push_no_target, pushed);
    };
    let next = match server.next_uid(&state.imap_name).await {
        Ok(next) => next,
        Err(err) if err.is::<Refused>() => {
            return drop_all(dir, drafts, &format!("{err:#}"), pushed);
        }
        Err(err) => return Err(err),
    };
    let mut saved_ids = BTreeSet::new();
    let mut discarded = BTreeSet::new();
    for draft in drafts {
        let attached = files.remove(&draft.id).unwrap_or_else(|| Ok(Vec::new()));
        let appended = match attached {
            Err(reason) => Err(fill(t().draft_missing, &[&reason])),
            Ok(attached) => {
                let message = draft.mail.copy(&attached)?;
                match server.append_draft(&state.imap_name, &message).await {
                    Ok(()) => Ok(()),
                    Err(err) if err.is::<Refused>() => {
                        Err(fill(t().draft_refused, &[&format!("{err:#}")]))
                    }
                    Err(err) => return Err(err),
                }
            }
        };
        let waited = journal::forget_mail(dir, draft.id)?;
        match appended {
            Ok(()) => {
                pushed.pushed += 1;
                let newer = journal::waiting_draft(dir, id, &draft.mail.message_id)?.is_some();
                if !waited && !newer {
                    discarded.insert(draft.mail.message_id.clone());
                }
                saved_ids.insert(draft.mail.message_id);
            }
            Err(reason) => pushed.dropped.push(dropped(draft, reason)),
        }
    }
    if saved_ids.is_empty() || next.uid_validity != state.uidvalidity {
        return Ok(());
    }
    sync::fetch_new(server, &folder_dir, id, &state.imap_name, next.uid).await?;
    let copies = saved(&folder_dir, &saved_ids)?;
    let arrived: BTreeSet<&str> = copies
        .iter()
        .filter(|(uid, _, _)| *uid >= next.uid)
        .filter_map(|(_, _, mail)| mail.message_id.as_deref())
        .collect();
    let gone = copies
        .iter()
        .filter(|(uid, _, mail)| {
            let Some(message_id) = mail.message_id.as_deref() else {
                return false;
            };
            discarded.contains(message_id) || (*uid < next.uid && arrived.contains(message_id))
        })
        .map(|(uid, file, _)| (*uid, file.clone()))
        .collect();
    expunge(server, &state, gone).await
}

pub async fn remove(server: &mut Server, dir: &Path, id: &str, message_id: &str) -> Result<()> {
    let Some((folder_dir, state)) = target(dir, id)? else {
        return Ok(());
    };
    let copies = saved(&folder_dir, &BTreeSet::from([message_id.to_string()]))?
        .into_iter()
        .map(|(uid, file, _)| (uid, file))
        .collect();
    expunge(server, &state, copies).await
}

fn target(dir: &Path, id: &str) -> Result<Option<(PathBuf, FolderState)>> {
    target_in(dir, &settings::load(dir)?, id)
}

fn target_in(dir: &Path, settings: &Settings, id: &str) -> Result<Option<(PathBuf, FolderState)>> {
    let Some(folder_dir) = folder(dir, settings, id)? else {
        return Ok(None);
    };
    Ok(store::read_state(&folder_dir)?.map(|state| (folder_dir, state)))
}

async fn expunge(
    server: &mut Server,
    state: &FolderState,
    copies: Vec<(u32, PathBuf)>,
) -> Result<()> {
    if copies.is_empty() {
        return Ok(());
    }
    let uids: Vec<u32> = copies.iter().map(|(uid, _)| *uid).collect();
    let removed = async {
        if server.select(&state.imap_name).await?.uid_validity != state.uidvalidity {
            return Ok(false);
        }
        server.expunge(&uids).await?;
        Ok::<_, anyhow::Error>(true)
    }
    .await;
    match removed {
        Ok(true) => {}
        Ok(false) => return Ok(()),
        Err(err) if err.is::<Refused>() => return Ok(()),
        Err(err) => return Err(err),
    }
    for (_, file) in copies {
        fs::remove_file(&file).with_context(|| format!("removing {}", file.display()))?;
    }
    Ok(())
}

fn saved(
    folder_dir: &Path,
    message_ids: &BTreeSet<String>,
) -> Result<Vec<(u32, PathBuf, MailFile)>> {
    Ok(store::index(folder_dir)?
        .into_iter()
        .filter_map(|(uid, file)| {
            let mail: MailFile = store::read(&file).ok()?;
            let known = mail
                .message_id
                .as_ref()
                .is_some_and(|message_id| message_ids.contains(message_id));
            known.then_some((uid, file, mail))
        })
        .collect())
}

fn dropped(draft: Outbound, reason: String) -> Dropped {
    Dropped {
        path: draft.path,
        message_id: Some(draft.mail.message_id),
        reason,
    }
}

fn drop_all(dir: &Path, drafts: Vec<Outbound>, reason: &str, pushed: &mut Pushed) -> Result<()> {
    for draft in drafts {
        journal::forget_mail(dir, draft.id)?;
        pushed.dropped.push(dropped(draft, reason.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::Path;

    use mail_parser::{MessageParser, MimeHeaders};

    use super::*;
    use crate::compose::{self, Choice};
    use crate::journal::Purpose;
    use crate::send::{Answer, NewMail, Pick, Reply, Request};
    use crate::settings::Smtp;
    use crate::testing::Fixture;

    const THREADED: &str = "From: me@example.com\r\nTo: jane@example.com\r\nSubject: Re: Plan\r\nMessage-ID: <d1@example.com>\r\nIn-Reply-To: <p1@example.com>\r\nReferences: <p1@example.com>\r\n\r\nHalf done.\r\n";

    const NOTES: &str = "From: jane@example.com\r\nTo: me@example.com\r\nSubject: Notes\r\nMessage-ID: <n1@example.com>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=b\r\n\r\n--b\r\nContent-Type: text/plain\r\n\r\nSee the notes.\r\n--b\r\nContent-Type: text/plain; name=notes.txt\r\nContent-Disposition: attachment; filename=notes.txt\r\n\r\nday one\r\n--b--\r\n";

    fn choice(to: &[&str], draft: bool) -> Choice {
        let to: Vec<String> = to.iter().map(|to| to.to_string()).collect();
        let mut choice = compose::choice(None, &to, &[], &[], Some("Plan".into())).unwrap();
        choice.draft = draft;
        choice
    }

    fn note(dir: &Path, choice: &Choice, request: &Request, text: &str) -> Outbound {
        NewMail::prepare(dir, "private", choice.draft)
            .unwrap()
            .note(dir, choice, request, text)
            .unwrap()
    }

    fn fixture(name: &str) -> (Fixture, PathBuf) {
        let mut fixture = Fixture::new(name);
        fixture.account.smtp = Some(Smtp::new("smtp.example.com".to_string(), 465, None));
        fixture.save();
        let drafts = fixture.marked_folder("Drafts", "drafts");
        (fixture, drafts)
    }

    fn mirrored(dir: &Path) -> BTreeMap<u32, MailFile> {
        store::index(&dir.join("private/drafts"))
            .unwrap()
            .into_iter()
            .map(|(uid, file)| (uid, store::read(&file).unwrap()))
            .collect()
    }

    #[tokio::test]
    async fn a_draft_is_saved_replaced_by_its_next_version_and_removed_once_sent() {
        let (fixture, drafts) = fixture("draft");
        fixture.sync().await;
        let dir = &fixture.dir;
        let plain = Request {
            continues: None,
            attachments: &[],
        };

        let first = note(dir, &choice(&[], true), &plain, "");
        assert_eq!(first.purpose, Purpose::Draft);
        let pushed = fixture.push().await;
        assert_eq!((pushed.pushed, pushed.dropped.len()), (1, 0));
        assert!(drafts.join("1,SD.eml").is_file());
        let saved = mirrored(dir);
        let id = first.mail.message_id.as_str();
        assert_eq!(saved[&1].message_id.as_deref(), Some(id));
        assert!(saved[&1].flags.draft);

        let continued = Request {
            continues: Some(id),
            attachments: &[],
        };
        note(
            dir,
            &choice(&["jane@example.com"], true),
            &continued,
            "Second",
        );
        let third = note(
            dir,
            &choice(&["jane@example.com"], true),
            &continued,
            "Third",
        );
        assert_eq!(third.mail.message_id, id);
        let waiting = journal::waiting_draft(dir, "private", id).unwrap().unwrap();
        assert_eq!(waiting.body, "Third\n");
        assert_eq!(fixture.push().await.pushed, 1);
        assert!(!drafts.join("1,SD.eml").exists());
        let saved = mirrored(dir);
        assert_eq!(saved.keys().copied().collect::<Vec<_>>(), [2]);
        assert_eq!(saved[&2].body.trim_end(), "Third");
        assert_eq!(saved[&2].to[0].address.as_deref(), Some("jane@example.com"));

        note(
            dir,
            &choice(&["jane@example.com"], false),
            &continued,
            "Final",
        );
        let pushed = fixture.push().await;
        assert_eq!(pushed.sent[0].message_id, id);
        assert!(
            fixture
                .dev
                .join(format!("private/.smtp/{id}.eml"))
                .is_file()
        );
        assert!(!drafts.join("2,SD.eml").exists());
        assert!(mirrored(dir).is_empty());
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[tokio::test]
    async fn a_continued_draft_keeps_its_thread_and_attachments_and_a_waiting_one_is_never_saved() {
        let (fixture, drafts) = fixture("draft-continued");
        fs::write(drafts.join("1,SD.eml"), THREADED).unwrap();
        fs::write(
            drafts.join("2,SD.eml"),
            include_bytes!("../tests/fixtures/multipart-attachment.eml"),
        )
        .unwrap();
        fixture.sync().await;
        let dir = &fixture.dir;

        let threaded = Request {
            continues: Some("d1@example.com"),
            attachments: &[],
        };
        let noted = note(
            dir,
            &choice(&["jane@example.com"], true),
            &threaded,
            "Done.",
        );
        assert_eq!(noted.mail.in_reply_to.as_deref(), Some("p1@example.com"));
        assert_eq!(noted.mail.references, ["p1@example.com"]);
        let attached = Request {
            continues: Some("attach@example.com"),
            attachments: &[Pick::Draft(0)],
        };
        let noted = note(
            dir,
            &choice(&["jane@example.com"], true),
            &attached,
            "See it.",
        );
        let original = &noted.mail.originals[0];
        assert_eq!((original.folder.as_str(), original.uid), ("Drafts", 2));
        let again = note(dir, &choice(&[], true), &attached, "Seen it.");
        assert_eq!(again.mail.originals, noted.mail.originals);
        let beyond = Request {
            attachments: &[Pick::Draft(1)],
            ..attached
        };
        let err = NewMail::prepare(dir, "private", true)
            .unwrap()
            .note(dir, &choice(&[], true), &beyond, "x")
            .err()
            .unwrap();
        assert_eq!(
            err.to_string(),
            fill(t().draft_no_attachment, &[&"attach@example.com", &1])
        );
        let missing = Request {
            continues: Some("gone@example.com"),
            attachments: &[Pick::Draft(0)],
        };
        let err = NewMail::prepare(dir, "private", true)
            .unwrap()
            .note(dir, &choice(&[], true), &missing, "x")
            .err()
            .unwrap();
        assert_eq!(
            err.to_string(),
            fill(t().draft_no_attachment, &[&"gone@example.com", &0])
        );

        let pushed = fixture.push().await;
        assert_eq!((pushed.pushed, pushed.dropped.len()), (2, 0));
        let saved = mirrored(dir);
        assert_eq!(saved.keys().copied().collect::<Vec<_>>(), [3, 4]);
        assert_eq!(saved[&3].in_reply_to.as_deref(), Some("p1@example.com"));
        assert_eq!(saved[&4].attachments[0].name.as_deref(), Some("offer.pdf"));

        let waiting = note(dir, &choice(&[], true), &threaded, "Again.");
        let sent = note(
            dir,
            &choice(&["jane@example.com"], false),
            &threaded,
            "Sent.",
        );
        assert_eq!(waiting.mail.message_id, sent.mail.message_id);
        assert!(
            journal::waiting_draft(dir, "private", "d1@example.com")
                .unwrap()
                .is_none()
        );
        let pushed = fixture.push().await;
        assert_eq!((pushed.pushed, pushed.sent.len()), (1, 1));
        assert_eq!(mirrored(dir).keys().copied().collect::<Vec<_>>(), [4]);
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[tokio::test]
    async fn a_reply_continuing_a_draft_takes_attachments_of_both() {
        let (fixture, drafts) = fixture("draft-reply");
        fs::write(fixture.dev.join("private/INBOX/2,S.eml"), NOTES).unwrap();
        fs::write(
            drafts.join("1,SD.eml"),
            include_bytes!("../tests/fixtures/multipart-attachment.eml"),
        )
        .unwrap();
        fixture.sync().await;
        let dir = &fixture.dir;
        let inbox = store::index(&dir.join("private/inbox")).unwrap();
        let path = store::relative(&inbox[&2], dir);
        let reply = |picks: &[Pick], draft: bool| {
            let request = Request {
                continues: Some("attach@example.com"),
                attachments: picks,
            };
            let answer = Answer {
                all: false,
                forward: false,
            };
            Reply::prepare(dir, Path::new(&path), draft)
                .unwrap()
                .note(
                    dir,
                    &choice(&["jane@example.com"], draft),
                    &answer,
                    &request,
                    "Both.",
                )
                .unwrap()
        };
        let names = |originals: &[Original]| -> Vec<(String, u32, String)> {
            originals
                .iter()
                .flat_map(|original| {
                    original.attachments.iter().map(|attachment| {
                        let name = attachment.name.clone().unwrap_or_default();
                        (original.folder.clone(), original.uid, name)
                    })
                })
                .collect()
        };

        let both = reply(
            &[Pick::Message(path.clone().into(), 0), Pick::Draft(0)],
            true,
        );
        assert_eq!(
            names(&both.mail.originals),
            [
                ("Drafts".to_string(), 1, "offer.pdf".to_string()),
                ("INBOX".to_string(), 2, "notes.txt".to_string())
            ]
        );
        let second = reply(&[Pick::Draft(1)], true);
        assert_eq!(
            names(&second.mail.originals),
            [("INBOX".to_string(), 2, "notes.txt".to_string())]
        );
        let pushed = fixture.push().await;
        assert_eq!((pushed.pushed, pushed.dropped.len()), (1, 0));
        let saved = mirrored(dir);
        assert_eq!(saved[&2].attachments[0].name.as_deref(), Some("notes.txt"));

        let sent = reply(&[Pick::Draft(0)], false);
        assert_eq!(
            names(&sent.mail.originals),
            [("Drafts".to_string(), 2, "notes.txt".to_string())]
        );
        let pushed = fixture.push().await;
        assert_eq!((pushed.pushed, pushed.sent.len()), (2, 1));
        let raw = fs::read(fixture.dev.join("private/.smtp/attach@example.com.eml")).unwrap();
        let parsed = MessageParser::new().parse(&raw).unwrap();
        let attachment = parsed.attachments().next().unwrap();
        assert_eq!(attachment.attachment_name(), Some("notes.txt"));
        assert_eq!(attachment.contents(), b"day one");
        assert!(store::read_flags(&dir.join(&path)).unwrap().answered);
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[tokio::test]
    async fn a_discarded_draft_leaves_the_journal_and_the_drafts_folder_for_good() {
        let (fixture, drafts) = fixture("draft-discard");
        fixture.sync().await;
        let dir = &fixture.dir;
        let plain = Request {
            continues: None,
            attachments: &[],
        };
        let saved = note(dir, &choice(&[], true), &plain, "Keep?");
        let id = saved.mail.message_id.as_str();
        fixture.push().await;
        assert!(drafts.join("1,SD.eml").is_file());
        let continued = Request {
            continues: Some(id),
            attachments: &[],
        };
        note(dir, &choice(&[], true), &continued, "Rather not.");

        run_discard(dir, "private", id, true).unwrap();
        assert!(
            journal::waiting_draft(dir, "private", id)
                .unwrap()
                .is_none()
        );
        let settings = settings::load(dir).unwrap();
        let overlay = journal::overlay(dir, &settings).unwrap();
        let file = store::index(&dir.join("private/drafts")).unwrap()[&1].clone();
        let path = store::relative(&file, dir);
        assert!(overlay.hides(&path));
        assert!(crate::show::open(dir, &settings, Path::new(&path)).is_err());
        crate::flag::run(dir, Path::new(&path), crate::flag::Change::Seen).unwrap();
        assert!(journal::overlay(dir, &settings).unwrap().hides(&path));
        let pushed = fixture.push().await;
        assert_eq!((pushed.pushed, pushed.dropped.len()), (1, 0));
        assert!(!drafts.join("1,SD.eml").exists());
        assert!(mirrored(dir).is_empty());
        let err = run_discard(dir, "private", "a b", false).err().unwrap();
        assert_eq!(err.to_string(), fill(t().draft_invalid, &[&"a b"]));
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[test]
    fn a_draft_needs_a_drafts_folder() {
        let fixture = Fixture::new("draft-folder");
        let err = NewMail::prepare(&fixture.dir, "private", true)
            .err()
            .unwrap();
        assert_eq!(err.to_string(), kind::no_folder("private", Kind::Drafts));
        fs::remove_dir_all(&fixture.base).unwrap();
    }
}
