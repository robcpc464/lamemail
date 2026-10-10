use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Serialize;

use crate::i18n::{fill, t};
use crate::list::{self, Located};
use crate::message::{self, Attachment, MailFile};
use crate::server::{self, Server};
use crate::settings::Settings;
use crate::store;
use crate::{output, settings};

const RUNTIME_DIR: &str = "XDG_RUNTIME_DIR";
const ATTACHMENTS_DIR: &str = "lamemail/attachments";
const MAX_NAME_BYTES: usize = 200;
const READ_ONLY: u32 = 0o400;

#[derive(Debug, Serialize)]
struct Saved {
    #[serde(flatten)]
    attachment: Attachment,
    path: PathBuf,
}

enum Place {
    Runtime(PathBuf),
    Folder(PathBuf),
}

pub async fn run(
    dir: &Path,
    dev: Option<&Path>,
    path: &Path,
    index: Option<u32>,
    to: Option<PathBuf>,
    json: bool,
) -> Result<()> {
    let place = match to {
        Some(folder) => Place::Folder(folder),
        None => Place::Runtime(runtime_base()?),
    };
    let saved = save(dir, dev, path, index, &place).await?;
    output::stdout(|out| {
        saved.iter().try_for_each(|saved| {
            if json {
                output::json_line(out, saved)
            } else {
                writeln!(out, "{}", saved.path.display())
            }
        })
    })
}

fn runtime_base() -> Result<PathBuf> {
    let runtime = std::env::var_os(RUNTIME_DIR)
        .filter(|value| !value.is_empty())
        .context(t().attachment_no_runtime_dir)?;
    Ok(PathBuf::from(runtime).join(ATTACHMENTS_DIR))
}

async fn save(
    dir: &Path,
    dev: Option<&Path>,
    path: &Path,
    index: Option<u32>,
    place: &Place,
) -> Result<Vec<Saved>> {
    let settings = settings::load(dir)?;
    let located = list::locate_one(dir, &settings, path)?;
    let file: MailFile = store::read(&located.file)?;
    let chosen = match index {
        Some(index) => vec![
            file.attachments
                .into_iter()
                .find(|attachment| attachment.index == index)
                .with_context(|| missing(&located, index))?,
        ],
        None => file.attachments,
    };
    let kept: Vec<Option<PathBuf>> = chosen
        .iter()
        .map(|attachment| place.kept(&located, attachment))
        .collect();
    let raw = if kept.iter().any(Option::is_none) {
        fetch(&settings, dev, &located).await?
    } else {
        Vec::new()
    };
    chosen
        .into_iter()
        .zip(kept)
        .map(|(attachment, kept)| {
            let path = match kept {
                Some(path) => path,
                None => {
                    let content = message::attachment_content(&raw, attachment.index)
                        .with_context(|| missing(&located, attachment.index))?;
                    place.write(&located, &attachment, &content)?
                }
            };
            Ok(Saved { attachment, path })
        })
        .collect()
}

fn missing(located: &Located, index: u32) -> String {
    fill(t().attachment_none, &[&located.path, &index])
}

impl Place {
    fn kept(&self, located: &Located, attachment: &Attachment) -> Option<PathBuf> {
        match self {
            Place::Runtime(base) => {
                Some(runtime_file(base, located, attachment)).filter(|path| path.exists())
            }
            Place::Folder(_) => None,
        }
    }

    fn write(&self, located: &Located, attachment: &Attachment, content: &[u8]) -> Result<PathBuf> {
        let target = match self {
            Place::Runtime(base) => runtime_file(base, located, attachment),
            Place::Folder(folder) => free_name(folder, &file_name(attachment)),
        };
        let folder = target.parent().context("attachment without folder")?;
        fs::create_dir_all(folder).with_context(|| format!("creating {}", folder.display()))?;
        store::write_bytes_atomic(&target, content)?;
        if let Place::Runtime(_) = self {
            fs::set_permissions(&target, Permissions::from_mode(READ_ONLY))
                .with_context(|| format!("protecting {}", target.display()))?;
        }
        Ok(target)
    }
}

fn runtime_file(base: &Path, located: &Located, attachment: &Attachment) -> PathBuf {
    base.join(Path::new(&located.path).with_extension(""))
        .join(attachment.index.to_string())
        .join(file_name(attachment))
}

fn free_name(folder: &Path, name: &str) -> PathBuf {
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() => (stem, format!(".{extension}")),
        _ => (name, String::new()),
    };
    std::iter::once(name.to_string())
        .chain((1..).map(|number: u64| format!("{stem} ({number}){extension}")))
        .map(|candidate| folder.join(candidate))
        .find(|path| !path.exists())
        .unwrap_or_else(|| folder.join(name))
}

async fn fetch(settings: &Settings, dev: Option<&Path>, located: &Located) -> Result<Vec<u8>> {
    let state = store::state_of(&located.file)?;
    let account = settings.account(&located.account)?;
    let mut server = Server::connect(account, server::access(dev, &located.account)?).await?;
    let raw = server
        .fetch_message(&state.imap_name, state.uidvalidity, located.uid)
        .await;
    server.logout().await;
    raw
}

pub fn file_name(attachment: &Attachment) -> String {
    let name = attachment.name.as_deref().unwrap_or_default();
    let last = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let visible: String = last.chars().filter(|c| !c.is_control()).collect();
    let start = (visible.len().saturating_sub(MAX_NAME_BYTES)..=visible.len())
        .find(|at| visible.is_char_boundary(*at))
        .unwrap_or_default();
    let clean = visible[start..].trim().trim_start_matches('.');
    if clean.is_empty() {
        format!("attachment-{}", attachment.index)
    } else {
        clean.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Fixture;

    #[tokio::test]
    async fn an_attachment_is_fetched_once_into_a_read_only_file() {
        let fixture = Fixture::new("attachment");
        let inbox = fixture.dev.join("private/INBOX");
        fs::write(
            inbox.join("2,S.eml"),
            include_bytes!("../tests/fixtures/multipart-attachment.eml"),
        )
        .unwrap();
        fixture.sync().await;
        let files = store::index(&fixture.dir.join("private/inbox")).unwrap();
        let path = PathBuf::from(store::relative(&files[&2], &fixture.dir));
        let base = fixture.base.join("runtime");
        let place = Place::Runtime(base.clone());
        let save = async |index| {
            save(&fixture.dir, Some(&fixture.dev), &path, Some(index), &place)
                .await
                .map(|mut saved| saved.remove(0))
        };

        let saved = save(0).await.unwrap();
        assert_eq!(
            saved.path,
            base.join(path.with_extension("")).join("0/offer.pdf")
        );
        assert_eq!(fs::read(&saved.path).unwrap(), b"%PDF-1.4 fake");
        let mode = fs::metadata(&saved.path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, READ_ONLY);
        let json = serde_json::to_value(&saved).unwrap();
        assert_eq!(json["name"], "offer.pdf");
        assert_eq!(json["path"], saved.path.to_str().unwrap());

        fs::remove_file(inbox.join("2,S.eml")).unwrap();
        assert_eq!(save(0).await.unwrap().path, saved.path);
        let none = fill(t().attachment_none, &[&path.display(), &1]);
        assert_eq!(save(1).await.unwrap_err().to_string(), none);
        fs::remove_file(&saved.path).unwrap();
        assert_eq!(save(0).await.unwrap_err().to_string(), t().message_gone);
        fs::write(inbox.join(".uidvalidity"), "2").unwrap();
        assert_eq!(save(0).await.unwrap_err().to_string(), t().folder_changed);
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[tokio::test]
    async fn all_attachments_go_into_a_folder_under_free_names() {
        let fixture = Fixture::new("attachment-folder");
        fs::write(
            fixture.dev.join("private/INBOX/2,S.eml"),
            "Subject: offers\r\nMIME-Version: 1.0\r\n\
             Content-Type: multipart/mixed; boundary=\"b\"\r\n\r\n\
             --b\r\nContent-Type: text/plain\r\n\r\nTwo offers.\r\n\
             --b\r\nContent-Type: application/pdf\r\n\
             Content-Disposition: attachment; filename=\"offer.pdf\"\r\n\r\none\r\n\
             --b\r\nContent-Type: application/pdf\r\n\
             Content-Disposition: attachment; filename=\"offer.pdf\"\r\n\r\ntwo\r\n--b--\r\n",
        )
        .unwrap();
        fixture.sync().await;
        let files = store::index(&fixture.dir.join("private/inbox")).unwrap();
        let path = PathBuf::from(store::relative(&files[&2], &fixture.dir));
        let downloads = fixture.base.join("downloads");
        let place = Place::Folder(downloads.clone());
        let save = async || {
            save(&fixture.dir, Some(&fixture.dev), &path, None, &place)
                .await
                .unwrap()
                .into_iter()
                .map(|saved| saved.path)
                .collect::<Vec<_>>()
        };

        let first = save().await;
        assert_eq!(
            first,
            [downloads.join("offer.pdf"), downloads.join("offer (1).pdf")]
        );
        assert_eq!(fs::read(&first[0]).unwrap(), b"one");
        assert_eq!(fs::read(&first[1]).unwrap(), b"two");
        let mode = fs::metadata(&first[0]).unwrap().permissions().mode();
        assert_ne!(mode & 0o200, 0);
        assert_eq!(
            save().await,
            [
                downloads.join("offer (2).pdf"),
                downloads.join("offer (3).pdf")
            ]
        );
        assert_eq!(free_name(&downloads, "README"), downloads.join("README"));
        fs::write(downloads.join("README"), "").unwrap();
        assert_eq!(
            free_name(&downloads, "README"),
            downloads.join("README (1)")
        );
        fs::remove_dir_all(&fixture.base).unwrap();
    }

    #[test]
    fn a_file_name_is_the_last_visible_part_of_the_attachment_name() {
        let named = |name: Option<&str>| {
            file_name(&Attachment {
                index: 3,
                name: name.map(str::to_string),
                content_type: "application/pdf".to_string(),
                size: 1,
            })
        };
        assert_eq!(named(Some("offer.pdf")), "offer.pdf");
        assert_eq!(named(Some("../../.bashrc")), "bashrc");
        assert_eq!(named(Some("C:\\Users\\jane\\plan.pdf")), "plan.pdf");
        assert_eq!(named(Some(" a\u{0}b\nc.txt ")), "abc.txt");
        assert_eq!(named(Some("..")), "attachment-3");
        assert_eq!(named(None), "attachment-3");
        let kept = named(Some(&format!("{}.pdf", "ä".repeat(150))));
        assert!(kept.len() <= MAX_NAME_BYTES && kept.ends_with("ää.pdf"));
    }
}
