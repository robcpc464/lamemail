use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

struct Mailbox {
    base: PathBuf,
    dir: PathBuf,
    dev: PathBuf,
}

impl Mailbox {
    fn new() -> Mailbox {
        let base = std::env::temp_dir().join(format!("lamemail-mcp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let mailbox = Mailbox {
            dir: base.join("mirror"),
            dev: base.join("server"),
            base,
        };
        for folder in ["INBOX", "Drafts"] {
            let folder = mailbox.dev.join("private").join(folder);
            fs::create_dir_all(&folder).unwrap();
            fs::write(folder.join(".uidvalidity"), "1").unwrap();
        }
        fs::write(mailbox.dev.join("private/Drafts/.special"), "drafts").unwrap();
        fs::write(
            mailbox.dev.join("private/INBOX/1,S.eml"),
            include_bytes!("fixtures/plain.eml"),
        )
        .unwrap();
        mailbox.run(&["init"], "");
        let added = mailbox.run(
            &[
                "account",
                "add",
                "private",
                "--host",
                "imap.example.com",
                "--user",
                "me@example.com",
            ],
            "",
        );
        assert!(
            added.contains("no folder for sent, junk, trash, archive"),
            "{added}"
        );
        mailbox.run(&["sync"], "");
        mailbox
    }

    fn lamemail(&self, arguments: &[&str], input: &str) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_lamemail"))
            .arg("--dir")
            .arg(&self.dir)
            .arg("--dev")
            .arg(&self.dev)
            .args(arguments)
            .env("LANG", "C")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }

    fn run(&self, arguments: &[&str], input: &str) -> String {
        let output = self.lamemail(arguments, input);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    fn serve(&self, requests: &[Value]) -> Vec<Value> {
        let input: String = requests
            .iter()
            .map(|request| format!("{request}\n"))
            .collect();
        self.run(&["mcp"], &input)
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}

fn call(id: u32, name: &str, arguments: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    })
}

fn text(reply: &Value) -> &str {
    reply["result"]["content"][0]["text"].as_str().unwrap()
}

#[test]
fn agents_read_the_mail_and_write_drafts_through_the_lamemail_commands() {
    let mailbox = Mailbox::new();
    let listed: Value = serde_json::from_str(mailbox.run(&["list", "--json"], "").trim()).unwrap();
    let path = listed["path"].as_str().unwrap();

    let replies = mailbox.serve(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        call(2, "list_messages", json!({"search": "offer"})),
        call(3, "list_messages", json!({"search": "nothing like this"})),
        call(4, "show_thread", json!({"path": path})),
        call(5, "list_contacts", json!({"search": "JANE"})),
        call(6, "create_draft", json!({"reply_to": path, "text": "Thanks, Tuesday works."})),
        call(7, "show_message", json!({"path": "private/inbox/nothing-9.json"})),
        call(
            8,
            "create_draft",
            json!({"forward": path, "to": ["tom@example.com"], "attachments": [0]}),
        ),
    ]);
    assert_eq!(replies.len(), 8);
    assert_eq!(replies[0]["result"]["capabilities"]["tools"], json!({}));
    let hit: Value = serde_json::from_str(text(&replies[1])).unwrap();
    assert_eq!(hit["subject"], "Offer for project");
    assert_eq!(text(&replies[2]), "");
    assert!(text(&replies[3]).contains("Offer for project"));
    assert!(text(&replies[4]).contains("jane@example.com"));
    let draft: Value = serde_json::from_str(text(&replies[5])).unwrap();
    assert_eq!(draft["action"], "draft");
    assert_eq!(draft["subject"], "Re: Offer for project");
    assert_eq!(replies[6]["result"]["isError"], true);
    assert_eq!(replies[7]["result"]["isError"], true);
    assert!(text(&replies[7]).contains(&format!("{path} has no attachment 0")));

    let journal = mailbox.run(&["journal", "--json"], "");
    assert!(journal.contains("Thanks, Tuesday works."));
    mailbox.run(&["push"], "");
    let drafts: Vec<_> = fs::read_dir(mailbox.dev.join("private/Drafts"))
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "eml"))
        .collect();
    assert_eq!(drafts.len(), 1);
    fs::remove_dir_all(&mailbox.base).unwrap();
}
