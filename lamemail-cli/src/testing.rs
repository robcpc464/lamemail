use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use crate::journal::{self, Pushed};
use crate::kind::{Assigned, Kind};
use crate::server::{Access, Server};
use crate::settings::{self, Account, Settings};
use crate::store;

pub struct Fixture {
    pub base: PathBuf,
    pub dev: PathBuf,
    pub dir: PathBuf,
    pub config: Settings,
    pub account: Account,
}

impl Fixture {
    pub fn new(name: &str) -> Fixture {
        let base = std::env::temp_dir().join(format!("lamemail-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let dev = base.join("server");
        let dir = base.join("mirror");
        dev_folder(&dev, "INBOX", None);
        dev_folder(&dev, "Sent", Some("sent"));
        fs::write(
            dev.join("private/INBOX/1,S.eml"),
            include_bytes!("../tests/fixtures/plain.eml"),
        )
        .unwrap();
        settings::init(&dir).unwrap();
        let mut fixture = Fixture {
            config: settings::load(&dir).unwrap(),
            account: Account::new(
                "imap.example.com".to_string(),
                993,
                "me@example.com".to_string(),
            ),
            base,
            dev,
            dir,
        };
        fixture.save();
        fixture
    }

    pub fn save(&mut self) {
        let saved = settings::load(&self.dir).unwrap();
        if let Some(synced) = saved.accounts.get("private") {
            self.account.folders = synced.folders.clone();
        }
        self.config
            .accounts
            .insert("private".to_string(), self.account.clone());
        settings::save(&self.dir, &self.config).unwrap();
    }

    pub async fn sync(&self) {
        crate::sync::sync_account(
            &self.dir,
            "private",
            &self.account,
            Access::Dev(self.dev.join("private")),
            &crate::sync::Report::Quiet,
        )
        .await
        .unwrap();
    }

    pub async fn push(&self) -> Pushed {
        let access = Access::Dev(self.dev.join("private"));
        let mut server = Server::connect(&self.account, access).await.unwrap();
        let pushed = journal::push_account(&mut server, &self.dir, "private")
            .await
            .unwrap();
        server.logout().await;
        pushed
    }

    pub fn settings(&self) -> Settings {
        settings::load(&self.dir).unwrap()
    }

    pub fn marked_folder(&self, name: &str, special: &str) -> PathBuf {
        dev_folder(&self.dev, name, Some(special))
    }
}

fn dev_folder(dev: &Path, name: &str, special: Option<&str>) -> PathBuf {
    let folder = dev.join("private").join(name);
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join(".uidvalidity"), "1").unwrap();
    if let Some(special) = special {
        fs::write(folder.join(".special"), special).unwrap();
    }
    folder
}

pub fn assigned(kinds: &[(&str, Kind)]) -> BTreeMap<String, Assigned> {
    kinds
        .iter()
        .map(|(name, kind)| {
            (
                name.to_string(),
                Assigned {
                    kind: Some(*kind),
                    sync: true,
                    default: false,
                },
            )
        })
        .collect()
}

pub fn write_message(dir: &Path, rel: &str, from: &str, to: &[&str], in_reply_to: Option<&str>) {
    let path = dir.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let state = path.parent().unwrap().join(store::STATE_FILE);
    if !state.exists() {
        fs::write(
            &state,
            r#"{"imap_name":"X","uidvalidity":1,"last_sync":null}"#,
        )
        .unwrap();
    }
    let name = path.file_name().unwrap().to_str().unwrap();
    let uid = store::uid_from_name(name).unwrap();
    let received = format!(
        "{}T{}:{}:00+02:00",
        &name[..10],
        &name[11..13],
        &name[13..15]
    );
    let person = |text: &&str| match text.split_once(" <") {
        Some((name, address)) => {
            serde_json::json!({"name": name, "address": address.trim_end_matches('>')})
        }
        None => serde_json::json!({"name": null, "address": text}),
    };
    let to: Vec<_> = to.iter().map(person).collect();
    let json = serde_json::json!({
        "format": 1, "account": rel.split('/').next(), "folder": "X", "uid": uid,
        "flags": {"seen": true, "answered": false, "flagged": false, "draft": false, "keywords": []},
        "received": received, "date": null, "size": 1,
        "message_id": format!("m{uid}@x"), "in_reply_to": in_reply_to, "references": [],
        "from": [person(&from)], "reply_to": [], "to": to,
        "cc": [], "bcc": [], "subject": "x", "body": "hi", "body_source": "text", "attachments": []
    });
    fs::write(&path, json.to_string()).unwrap();
}

pub struct Served {
    pub path: String,
    pub body: String,
}

pub fn http_server(
    answer: impl Fn(&str) -> (u16, String) + Send + 'static,
) -> (String, Receiver<Served>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    let (served, received) = mpsc::channel();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let path = request_line
                .split(' ')
                .nth(1)
                .unwrap_or_default()
                .to_string();
            let mut length = 0;
            loop {
                let mut header = String::new();
                reader.read_line(&mut header).unwrap();
                if header.trim().is_empty() {
                    break;
                }
                if let Some(value) = header.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let (status, reply) = answer(&path);
            write!(
                stream,
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{reply}",
                reply.len()
            )
            .unwrap();
            let _ = served.send(Served {
                path,
                body: String::from_utf8(body).unwrap(),
            });
        }
    });
    (address, received)
}

pub fn closed_address() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().to_string()
}
