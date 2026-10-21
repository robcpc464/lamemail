mod ai;
mod attachment;
mod commands;
mod compose;
mod contacts;
mod draft;
mod flag;
mod folder_name;
mod folders;
mod html;
mod http;
mod i18n;
mod imap;
mod journal;
mod keyring;
mod kind;
mod list;
mod mcp;
mod message;
mod moving;
mod net;
mod ollama;
mod output;
mod reply;
mod send;
mod server;
mod settings;
mod show;
mod smtp;
mod store;
mod style;
mod suggest;
mod sync;
#[cfg(test)]
mod testing;
mod thread;
mod update;
mod watch;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{CommandFactory, Parser, Subcommand};
use serde::Serialize;
use tokio::task::JoinSet;

use crate::i18n::{fill, t};
use crate::kind::{Kind, Kinds};
use crate::server::{Access, Server, access};
use crate::settings::{Account, Settings, Smtp};
use crate::style::{Color, Property};

#[derive(Parser)]
#[command(
    name = "lamemail",
    version,
    about = "Mirrors IMAP mailboxes into plain JSON text files"
)]
struct Cli {
    #[arg(
        long,
        global = true,
        env = "LAMEMAIL_DIR",
        value_name = "DIR",
        help = "Sync folder (default: $LAMEMAIL_DIR or ~/.lamemail)"
    )]
    dir: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        env = "LAMEMAIL_DEV",
        value_name = "DIR",
        hide = true,
        help = "Development server: a folder of .eml files per account that stands in for the mail server; no network, no keyring"
    )]
    dev: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[command(about = "Create the sync folder and an empty settings.json")]
    Init,
    #[command(about = "Manage accounts")]
    Account {
        #[command(subcommand)]
        action: AccountCommand,
    },
    #[command(about = "Mirror all (or the given) accounts once")]
    Sync {
        #[arg(value_name = "ID", help = "Account ids; all accounts when omitted")]
        ids: Vec<String>,
        #[arg(long, help = "One JSON object per account with its counts or error")]
        json: bool,
    },
    #[command(
        about = "Keep mirroring all (or the given) accounts: the inbox as soon as the server reports a change (IMAP IDLE), everything every 15 minutes"
    )]
    Watch {
        #[arg(value_name = "ID", help = "Account ids; all accounts when omitted")]
        ids: Vec<String>,
        #[arg(
            long,
            help = "One JSON object per sync with its counts and the mail that arrived, or per error"
        )]
        json: bool,
    },
    #[command(about = "List every message of every account, newest first")]
    List {
        #[arg(long, help = "One JSON object per line instead of a table")]
        json: bool,
        #[arg(
            long,
            value_name = "WORDS",
            help = "Only messages that contain every word, ignoring case, in sender, recipients, subject or text"
        )]
        search: Option<String>,
        #[arg(
            long = "kind",
            value_enum,
            value_name = "KIND",
            help = "Only messages in a folder of this kind, repeated for more (default: every kind)"
        )]
        kinds: Vec<Kind>,
        #[arg(
            long,
            help = "Flagged (starred) messages first, marked as pinned; both parts newest first"
        )]
        flagged_first: bool,
    },
    #[command(about = "Show one message: headers and text, or the whole JSON object")]
    Show {
        #[arg(
            help = "Message file, as printed by `lamemail list --json` (relative to the sync folder)"
        )]
        path: PathBuf,
        #[arg(long, help = "One JSON object instead of text")]
        json: bool,
    },
    #[command(about = "Show the history with the people of a message, replies nested")]
    Thread {
        #[arg(
            help = "Message file, as printed by `lamemail list --json` (relative to the sync folder)"
        )]
        path: PathBuf,
        #[arg(long, help = "One JSON object per message instead of text")]
        json: bool,
    },
    #[command(
        about = "Fetch one or all attachments of a message from the server and print their paths: read-only into $XDG_RUNTIME_DIR/lamemail/attachments, where a file already there is not fetched again, or into a folder of the user's"
    )]
    Attachment {
        #[arg(
            help = "Message file, as printed by `lamemail list --json` (relative to the sync folder)"
        )]
        path: PathBuf,
        #[arg(help = "Index of the attachment, as `show --json` lists it; all when omitted")]
        index: Option<u32>,
        #[arg(
            long,
            value_name = "DIR",
            help = "Save into this folder instead, as normal files; a name taken there gets ` (1)`, ` (2)`, …"
        )]
        to: Option<PathBuf>,
        #[arg(long, help = "One JSON object per file instead of its path")]
        json: bool,
    },
    #[command(
        about = "Write a mail, new from an account, a reply with --reply or a forward with --forward; the text comes from stdin: noted in the journal at once, sent with the next push"
    )]
    Send(Compose),
    #[command(
        about = "Save a mail as a draft into the account's drafts folder, with the same arguments as send; noted in the journal at once, saved with the next push, which deletes the draft's older versions"
    )]
    Draft(Compose),
    #[command(
        about = "Delete a saved draft for good: a version waiting in the journal is dropped, its copies in the drafts folder are expunged with the next push"
    )]
    Discard {
        #[arg(help = "Account id")]
        id: String,
        #[arg(
            value_name = "MESSAGE-ID",
            help = "Message-ID of the draft, as `draft --json` prints it"
        )]
        message_id: String,
        #[arg(
            long,
            help = "One JSON object with the message files noted for deletion"
        )]
        json: bool,
    },
    #[command(
        about = "Mark a message as seen, unseen, flagged, unflagged, answered, forwarded, junk or not junk: noted in the journal at once, on the server with the next push"
    )]
    Flag {
        #[arg(
            help = "Message file, as printed by `lamemail list --json` (relative to the sync folder)"
        )]
        path: PathBuf,
        #[arg(value_enum)]
        change: flag::Change,
    },
    #[command(
        about = "Move a message into another folder of its account: noted in the journal at once, on the server with the next push"
    )]
    Move {
        #[arg(
            help = "Message file, as printed by `lamemail list --json` (relative to the sync folder)"
        )]
        path: PathBuf,
        #[arg(
            help = "Local folder path of the account, or inbox, archive, trash or junk for the account's default folder of that kind; archive goes into the folder of the mail's year below it while the account archives by year; mail moved into a folder that is not synchronized leaves lamemail"
        )]
        folder: String,
        #[arg(long, help = "One JSON object instead of text")]
        json: bool,
    },
    #[command(
        about = "Delete a message in the trash for good: noted in the journal at once, expunged with the next push"
    )]
    Delete {
        #[arg(
            help = "Message file, as printed by `lamemail list --json` (relative to the sync folder)"
        )]
        path: PathBuf,
    },
    #[command(
        about = "Send what the journal holds to the server: changes bundled per folder, then exactly those messages mirrored; then the mail"
    )]
    Push {
        #[arg(
            long,
            help = "One JSON object per account with what was pushed, sent and dropped, or its error"
        )]
        json: bool,
    },
    #[command(about = "List what waits in the journal for the next push")]
    Journal {
        #[arg(long, help = "One JSON object per entry instead of text")]
        json: bool,
    },
    #[command(
        about = "List everyone known from the mail: the people written to first, then everyone else, newest first"
    )]
    Contacts {
        #[arg(long, help = "One JSON object per line instead of a table")]
        json: bool,
    },
    #[command(
        about = "List the kinds of folders and choose how the mail of each kind looks in the feed, for all accounts"
    )]
    Folder {
        #[command(subcommand)]
        action: FolderCommand,
    },
    #[command(
        about = "The AI that drafts text, Claude Code, a model of the local ollama service or a program of your own: choose it, and have it draft; suggest is the only command that hands mail to it"
    )]
    Ai {
        #[command(subcommand)]
        action: AiCommand,
    },
    #[command(
        about = "Serve the mail to AI agents over the Model Context Protocol on stdin and stdout: messages, threads, contacts and accounts to read, drafts to write, nothing else"
    )]
    Mcp,
    #[command(about = "List every command with its usage, arguments and help")]
    Commands {
        #[arg(long, help = "One JSON object per command instead of text")]
        json: bool,
    },
    #[command(about = "Look for a newer version of lamemail")]
    Update {
        #[command(subcommand)]
        action: UpdateCommand,
    },
}

#[derive(Subcommand)]
enum UpdateCommand {
    #[command(
        about = "Ask GitHub for the newest release and compare it with this version; nothing is downloaded"
    )]
    Check {
        #[arg(long, help = "One JSON object instead of text")]
        json: bool,
    },
}

#[derive(Subcommand)]
enum FolderCommand {
    #[command(about = "Show every kind of folder with its style, summed over all accounts")]
    List {
        #[arg(long, help = "One JSON object per line instead of a table")]
        json: bool,
    },
    #[command(about = "Set one property of the style of a kind")]
    Set {
        #[arg(value_enum)]
        kind: Kind,
        #[arg(value_enum)]
        property: Property,
        #[arg(
            help = "An icon name for icon, a colour name for color, 0 to 3 for indent, else true or false"
        )]
        value: String,
    },
    #[command(about = "Remove one or all properties of a kind so its defaults apply again")]
    Reset {
        #[arg(value_enum)]
        kind: Kind,
        #[arg(value_enum)]
        property: Option<Property>,
    },
}

#[derive(clap::Args)]
#[command(
    group(clap::ArgGroup::new("original").args(["reply", "forward"]))
)]
struct Compose {
    #[arg(
        required_unless_present = "original",
        conflicts_with = "original",
        help = "Account id of a new mail"
    )]
    id: Option<String>,
    #[arg(
        long,
        value_name = "PATH",
        help = "Message file to answer, as printed by `lamemail list --json`; once sent, the original gets \\Answered"
    )]
    reply: Option<PathBuf>,
    #[arg(
        long,
        value_name = "PATH",
        help = "Message file to forward, as for --reply, under `Fwd: `; once sent, the original gets $Forwarded"
    )]
    forward: Option<PathBuf>,
    #[arg(
        long,
        requires = "reply",
        conflicts_with = "cc",
        help = "Reply to all: the other recipients get a copy"
    )]
    all: bool,
    #[arg(
        long,
        value_name = "ADDRESS",
        help = "Sender, `address` or `Name <address>`, any alias; default: the own address the message was sent to, else the account's"
    )]
    from: Option<String>,
    #[arg(
        long,
        value_name = "ADDRESS",
        help = "Recipient, `address` or `Name <address>`, with --reply instead of the sender of the message; repeat for more; a mail to send needs one"
    )]
    to: Vec<String>,
    #[arg(
        long,
        value_name = "ADDRESS",
        help = "Recipient of a copy, with --reply instead of those of `--all`; repeat for more"
    )]
    cc: Vec<String>,
    #[arg(
        long,
        value_name = "ADDRESS",
        help = "Hidden recipient of a copy; repeat for more"
    )]
    bcc: Vec<String>,
    #[arg(
        long,
        help = "Subject, with --reply instead of `Re: ` and the original subject"
    )]
    subject: Option<String>,
    #[arg(
        long = "draft",
        value_name = "MESSAGE-ID",
        help = "Continue the saved draft with this Message-ID: the mail keeps it, without --reply or --forward it keeps the draft's thread, and the push deletes the draft once the new version is saved or the mail is sent"
    )]
    continues: Option<String>,
    #[arg(
        long = "attachment",
        value_name = "PATH:INDEX",
        help = "Attachment to send along, its `index` in `show --json`: of the message file at `path`, any of the account, or as a bare index of the --draft, whose path changes with every save while its Message-ID stays; repeat for more"
    )]
    attachments: Vec<send::Pick>,
    #[arg(
        long = "file",
        value_name = "PATH",
        help = "File to attach, read when the push sends the mail; repeat for more"
    )]
    files: Vec<PathBuf>,
    #[arg(long, help = "One JSON object instead of text")]
    json: bool,
}

impl Compose {
    fn run(self, dir: &Path, draft: bool) -> Result<()> {
        let mut choice = compose::choice(
            self.from.as_deref(),
            &self.to,
            &self.cc,
            &self.bcc,
            self.subject,
        )?;
        choice.files = self.files;
        choice.draft = draft;
        let request = send::Request {
            continues: self.continues.as_deref(),
            attachments: &self.attachments,
        };
        let answer = send::Answer {
            all: self.all,
            forward: self.forward.is_some(),
        };
        match (self.reply.or(self.forward), self.id) {
            (Some(path), _) => send::run_reply(dir, &path, &choice, &answer, &request, self.json),
            (None, Some(id)) => send::run_new(dir, &id, &choice, &request, self.json),
            (None, None) => bail!("an account id, --reply or --forward is needed"),
        }
    }
}

#[derive(Subcommand)]
enum AiChoice {
    #[command(
        about = "Claude Code on your subscription, run without tools or MCP servers: claude -p --tools \"\" --strict-mcp-config --output-format text"
    )]
    ClaudeCode,
    #[command(
        about = "A model of the ollama service on this computer (127.0.0.1:11434), as `ai show` lists it"
    )]
    Ollama {
        #[arg(help = "The model, e.g. gemma4:12b")]
        model: String,
    },
    #[command(
        about = "A program of your own that gets the prompt on stdin and prints the answer, run without a shell; it is kept when another is chosen"
    )]
    Custom {
        #[arg(
            help = "Program and arguments as one argument, quoted as in a shell, e.g. 'llm -m mistral' (default: the own command kept)"
        )]
        command: Option<String>,
    },
}

#[derive(Subcommand)]
enum AiCommand {
    #[command(about = "Choose what answers for the AI; mail goes to it only when asked")]
    Set {
        #[command(subcommand)]
        choice: AiChoice,
    },
    #[command(
        about = "Show what answers for the AI, the own command kept, and the known programs and ollama models"
    )]
    Show {
        #[arg(long, help = "One JSON object instead of text")]
        json: bool,
    },
    #[command(about = "Remove the program; no mail goes to an AI any more")]
    Remove,
    #[command(
        about = "Set how many characters of the context folder's files the AI gets at most, the same for every account"
    )]
    ContextLimit {
        #[arg(
            value_name = "CHARS",
            help = "10000, 20000 (the default), 50000 or 100000; large values only help a model with a large context"
        )]
        chars: usize,
    },
    #[command(
        about = "Draft or revise the text of a new mail or a reply with the AI that is set: the instruction comes from stdin, the whole text goes to stdout; nothing is noted or sent"
    )]
    Suggest {
        #[arg(
            required_unless_present = "reply",
            conflicts_with = "reply",
            help = "Account id of a new mail"
        )]
        id: Option<String>,
        #[arg(
            long,
            value_name = "PATH",
            help = "Message file to answer, as printed by `lamemail list --json` (relative to the sync folder)"
        )]
        reply: Option<PathBuf>,
        #[arg(
            long,
            value_name = "ADDRESS",
            help = "Who the mail goes to, `address` or `Name <address>`, repeated for more (a reply's default: answer_to of show --json)"
        )]
        to: Vec<String>,
        #[arg(
            long,
            value_name = "ADDRESS",
            help = "Who gets a copy of the mail, repeated for more"
        )]
        cc: Vec<String>,
        #[arg(long, value_name = "TEXT", help = "The subject of the mail")]
        subject: Option<String>,
        #[arg(
            long,
            value_name = "TEXT",
            help = "The text of the mail begun so far, up to the place of the instruction; with it or --after the AI revises the mail and answers with all of it"
        )]
        before: Option<String>,
        #[arg(
            long,
            value_name = "TEXT",
            help = "The text of the mail begun so far, from the place of the instruction on; a quote and a signature at its end stay as they are, the AI never sees them"
        )]
        after: Option<String>,
        #[arg(
            long,
            help = "Never ask back: write the mail now, with a placeholder in square brackets for a fact still missing"
        )]
        no_questions: bool,
        #[arg(
            long,
            help = "One JSON object with the account, the path and the text instead of the text"
        )]
        json: bool,
    },
}

#[derive(Subcommand)]
enum AccountCommand {
    #[command(
        about = "Add an account; secret-tool asks for the password and keeps it in the keyring"
    )]
    Add {
        #[arg(help = "Readable id, e.g. private or business (a-z, 0-9, -, _)")]
        id: String,
        #[arg(long, help = "IMAP server host name")]
        host: String,
        #[arg(long = "user", value_name = "LOGIN", help = "IMAP login name")]
        username: String,
        #[arg(long, default_value_t = 993, help = "IMAPS port")]
        port: u16,
        #[arg(long, value_name = "HOST", help = "SMTP server host name, for sending")]
        smtp_host: Option<String>,
        #[arg(
            long,
            value_name = "PORT",
            default_value_t = settings::SMTP_PORT,
            help = "SMTP port: 465 is TLS from the start, any other port uses STARTTLS"
        )]
        smtp_port: u16,
    },
    #[command(
        about = "Set the SMTP server of an account; it uses the same login and password as IMAP"
    )]
    Smtp {
        #[arg(help = "Account id")]
        id: String,
        #[arg(long, help = "SMTP server host name (default: the one set)")]
        host: Option<String>,
        #[arg(
            long,
            help = "SMTP port: 465 is TLS from the start, any other port uses STARTTLS (default: the one set, else 465)"
        )]
        port: Option<u16>,
        #[arg(
            long,
            value_name = "BOOL",
            help = "Save a copy of sent mail in the sent folder (default: the one set, else false for Gmail and Microsoft, which keep it themselves, and true for the rest)"
        )]
        copy: Option<bool>,
    },
    #[command(about = "Set the colour of the account's tag in the feed")]
    Color {
        #[arg(help = "Account id")]
        id: String,
        #[arg(value_enum)]
        color: Color,
    },
    #[command(
        about = "Set the account's signature from stdin, inserted by lamemail-ui's /signature command under a `-- ` line; empty input removes it"
    )]
    Signature {
        #[arg(help = "Account id")]
        id: String,
    },
    #[command(
        about = "Set the account's context for the AI to refer to: a folder whose text files, such as a profile or a price list, the AI gets with every mail of the account, read anew each time"
    )]
    Context {
        #[arg(help = "Account id")]
        id: String,
        #[arg(
            value_name = "DIR",
            help = "The folder; its files directly in it, not in subfolders (none: the context is removed)"
        )]
        folder: Option<PathBuf>,
    },
    #[command(
        about = "Choose where archived mail goes: a folder per year of its arrival below the archive, created by the push, or the archive itself"
    )]
    Archive {
        #[arg(help = "Account id")]
        id: String,
        #[arg(
            long,
            value_name = "BOOL",
            action = clap::ArgAction::Set,
            required = true,
            help = "A folder per year (default: true, false for Gmail, whose archive is All Mail)"
        )]
        by_year: bool,
    },
    #[command(
        about = "Give a folder of the account its kind; only folders with a kind and sync are synchronized and found, the default folder of a kind is where mail is moved"
    )]
    Folder {
        #[arg(help = "Account id")]
        id: String,
        #[arg(
            help = "Folder, as the local path below the account (`[gmail]/all mail`), assigned by `account add` or a sync"
        )]
        folder: String,
        #[arg(value_parser = kind::names(), help = "Its kind; other is never synchronized")]
        kind: String,
        #[arg(
            long,
            value_name = "BOOL",
            help = "Synchronize it or not (default: as before for the same kind, else unless other)"
        )]
        sync: Option<bool>,
        #[arg(long, help = "Make it the default folder of its kind")]
        default: bool,
    },
    #[command(about = "List configured accounts")]
    List {
        #[arg(long, help = "One JSON object per line instead of text")]
        json: bool,
    },
    #[command(about = "Log in and list the server folders")]
    Test {
        #[arg(help = "Account id")]
        id: String,
    },
    #[command(about = "Replace the password in the keyring (secret-tool asks for it)")]
    Password {
        #[arg(help = "Account id")]
        id: String,
    },
    #[command(about = "Remove an account from settings.json and the keyring")]
    Remove {
        #[arg(help = "Account id")]
        id: String,
        #[arg(long, help = "Also delete the account's synced mail")]
        purge: bool,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("lamemail: {err:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<()> {
    let dir = settings::sync_dir(cli.dir)?;
    if cli.dev.is_some() && dir == settings::sync_dir(None)? {
        bail!("{}", fill(t().dev_default_dir, &[&dir.display()]));
    }
    match cli.command {
        Command::Init => {
            let path = dir.join(settings::SETTINGS_FILE);
            let template = if settings::init(&dir)? {
                t().init_done
            } else {
                t().init_exists
            };
            println!("{}", fill(template, &[&path.display()]));
            Ok(())
        }
        Command::Account { action } => account(&dir, cli.dev.as_deref(), action).await,
        Command::Sync { ids, json } => sync(&dir, cli.dev.as_deref(), ids, json).await,
        Command::Watch { ids, json } => watch::run(&dir, cli.dev.as_deref(), ids, json).await,
        Command::List {
            json,
            search,
            kinds,
            flagged_first,
        } => list::run(
            &dir,
            json,
            search.as_deref().unwrap_or_default(),
            &kinds,
            flagged_first,
        ),
        Command::Show { path, json } => show::run(&dir, &path, json),
        Command::Thread { path, json } => thread::run(&dir, &path, json),
        Command::Attachment {
            path,
            index,
            to,
            json,
        } => attachment::run(&dir, cli.dev.as_deref(), &path, index, to, json).await,
        Command::Send(compose) => compose.run(&dir, false),
        Command::Draft(compose) => compose.run(&dir, true),
        Command::Discard {
            id,
            message_id,
            json,
        } => draft::run_discard(&dir, &id, &message_id, json),
        Command::Flag { path, change } => flag::run(&dir, &path, change),
        Command::Move { path, folder, json } => moving::run(&dir, &path, &folder, json),
        Command::Delete { path } => moving::delete(&dir, &path),
        Command::Push { json } => journal::run_push(&dir, cli.dev.as_deref(), json).await,
        Command::Journal { json } => journal::run_list(&dir, json),
        Command::Contacts { json } => contacts::run(&dir, json),
        Command::Folder { action } => folder(&dir, action),
        Command::Ai { action } => match action {
            AiCommand::Set { choice } => match choice {
                AiChoice::ClaudeCode => ai::set_claude_code(&dir),
                AiChoice::Ollama { model } => ai::set_ollama(&dir, &model),
                AiChoice::Custom { command } => ai::set_custom(&dir, command.as_deref()),
            },
            AiCommand::Show { json } => ai::show(&dir, json),
            AiCommand::Remove => ai::remove(&dir),
            AiCommand::ContextLimit { chars } => ai::set_context_limit(&dir, chars),
            AiCommand::Suggest {
                id,
                reply,
                to,
                cc,
                subject,
                before,
                after,
                no_questions,
                json,
            } => {
                let target = match (&reply, &id) {
                    (Some(path), _) => suggest::Target::Reply(path),
                    (None, Some(id)) => suggest::Target::New(id),
                    (None, None) => bail!("an account id or --reply is needed"),
                };
                let request = suggest::Request {
                    to,
                    cc,
                    subject: subject.unwrap_or_default(),
                    before: before.unwrap_or_default(),
                    after: after.unwrap_or_default(),
                    no_questions,
                };
                suggest::run(&dir, &target, &request, json)
            }
        },
        Command::Mcp => mcp::run(&dir, cli.dev.as_deref()),
        Command::Commands { json } => commands::run(&Cli::command(), json),
        Command::Update {
            action: UpdateCommand::Check { json },
        } => update::check(cli.dev.as_deref(), json),
    }
}

async fn account(dir: &Path, dev: Option<&Path>, action: AccountCommand) -> Result<()> {
    let mut settings = settings::load(dir)?;
    match action {
        AccountCommand::Add {
            id,
            host,
            username,
            port,
            smtp_host,
            smtp_port,
        } => {
            settings::validate_id(&id)?;
            if settings.accounts.contains_key(&id) {
                bail!("{}", fill(t().account_exists, &[&id]));
            }
            let mut account = Account {
                smtp: smtp_host.map(|smtp_host| Smtp::new(smtp_host, smtp_port, None)),
                ..Account::new(host, port, username)
            };
            if dev.is_none() {
                keyring::store(&id)?;
            }
            match check(&account, access(dev, &id)?).await {
                Ok(checked) => {
                    checked.report();
                    kind::assign(&mut account.folders, &checked.folders);
                }
                Err(err) => {
                    if dev.is_none() {
                        keyring::clear(&id)?;
                    }
                    return Err(err);
                }
            }
            let note = Kinds::of(&account).note(&id);
            settings.add(id.clone(), account);
            settings::save(dir, &settings)?;
            println!("{}", fill(t().account_added, &[&id]));
            if let Some(note) = note {
                println!("{note}");
            }
        }
        AccountCommand::List { json: true } => {
            let lines = settings
                .accounts
                .iter()
                .zip(password_states(dev, &settings))
                .map(|((id, account), password)| AccountLine {
                    id,
                    account: effective(&settings, id, account),
                    from: sender_of(dir, &settings, id),
                    password,
                    kinds: Kinds::of(account),
                })
                .collect::<Vec<AccountLine>>();
            output::stdout(|out| {
                lines
                    .iter()
                    .try_for_each(|line| output::json_line(out, line))
            })?;
        }
        AccountCommand::List { json: false } => {
            if settings.accounts.is_empty() {
                println!("{}", t().no_accounts);
            }
            for ((id, account), password) in settings
                .accounts
                .iter()
                .zip(password_states(dev, &settings))
            {
                let note = match password {
                    Password::Unused => t().dev_server,
                    Password::Stored => t().password_present,
                    Password::Missing => t().password_missing,
                    Password::Unknown => t().password_unknown,
                };
                println!(
                    "{id}: {}@{}:{} ({note})",
                    account.username, account.host, account.port
                );
                match &account.smtp {
                    Some(smtp) => {
                        println!("  SMTP {}:{}, {}", smtp.host, smtp.port, copy_note(smtp))
                    }
                    None => println!("  {}", t().smtp_none),
                }
                let signature = if account.signature.is_some() {
                    t().signature_present
                } else {
                    t().signature_none
                };
                println!("  {signature}");
                match &account.context_folder {
                    Some(folder) => {
                        println!("  {}", fill(t().context_folder, &[&folder.display()]))
                    }
                    None => println!("  {}", t().context_none),
                }
                println!("  {}", archive_note(account));
            }
        }
        AccountCommand::Smtp {
            id,
            host,
            port,
            copy,
        } => {
            let account = settings.account_mut(&id)?;
            let current = account.smtp.as_ref();
            let smtp = settings::changed_smtp(current, host, port, copy)
                .with_context(|| fill(t().smtp_host_needed, &[&id]))?;
            if current.is_none_or(|current| current.host != smtp.host || current.port != smtp.port)
            {
                check_smtp(&smtp, &account.username, &access(dev, &id)?).await?;
            }
            println!(
                "{}",
                fill(
                    t().smtp_set,
                    &[&id, &smtp.host, &smtp.port, &copy_note(&smtp)]
                )
            );
            account.smtp = Some(smtp);
            settings::save(dir, &settings)?;
        }
        AccountCommand::Color { id, color } => {
            settings.account_mut(&id)?.color = Some(color);
            settings::save(dir, &settings)?;
            println!("{}", fill(t().color_set, &[&id, &color.as_str()]));
        }
        AccountCommand::Signature { id } => {
            let account = settings.account_mut(&id)?;
            account.signature = settings::signature(&send::read_text()?);
            let template = if account.signature.is_some() {
                t().signature_set
            } else {
                t().signature_removed
            };
            settings::save(dir, &settings)?;
            println!("{}", fill(template, &[&id]));
        }
        AccountCommand::Context { id, folder } => {
            let folder = folder
                .map(|folder| settings::context_folder(&folder))
                .transpose()?;
            let account = settings.account_mut(&id)?;
            let report = match &folder {
                Some(folder) => fill(t().context_set, &[&id, &folder.display()]),
                None => fill(t().context_removed, &[&id]),
            };
            account.context_folder = folder;
            settings::save(dir, &settings)?;
            println!("{report}");
        }
        AccountCommand::Archive { id, by_year } => {
            let account = settings.account_mut(&id)?;
            account.archive_by_year = Some(by_year);
            let note = archive_note(account);
            settings::save(dir, &settings)?;
            println!("{}", fill(t().archive_set, &[&id, &note]));
        }
        AccountCommand::Folder {
            id,
            folder,
            kind,
            sync,
            default,
        } => {
            let kind = kind::parse(&kind).map_err(anyhow::Error::msg)?;
            let name = folder_name::normalize(&folder);
            let account = settings.account_mut(&id)?;
            let assigned = kind::reassign(&mut account.folders, &name, kind, sync, default)?;
            settings::save(dir, &settings)?;
            let mut notes = vec![if assigned.sync {
                t().synced
            } else {
                t().not_synced
            }];
            if assigned.default {
                notes.push(t().folder_default);
            }
            println!(
                "{}",
                fill(
                    t().folder_assigned,
                    &[
                        &id,
                        &name,
                        &kind.map_or(kind::OTHER, Kind::as_str),
                        &notes.join(", ")
                    ]
                )
            );
        }
        AccountCommand::Test { id } => {
            let account = settings.account(&id)?;
            let checked = check(account, access(dev, &id)?).await?;
            for folder in &checked.folders {
                let marker = if folder.selectable { " " } else { "-" };
                println!("{marker} {}", folder_name::decode_utf7(&folder.name));
            }
            checked.report();
        }
        AccountCommand::Password { id } => {
            let account = settings.account(&id)?;
            if dev.is_some() {
                println!("{}", t().dev_no_password);
                return Ok(());
            }
            keyring::store(&id)?;
            check(account, access(dev, &id)?).await?.report();
            println!("{}", fill(t().password_updated, &[&id]));
        }
        AccountCommand::Remove { id, purge } => {
            settings.account(&id)?;
            settings.accounts.remove(&id);
            settings::save(dir, &settings)?;
            if dev.is_none() {
                keyring::clear(&id)?;
            }
            journal::forget_account(dir, &id)?;
            if purge {
                let account_dir = dir.join(&id);
                if account_dir.exists() {
                    std::fs::remove_dir_all(&account_dir)
                        .with_context(|| format!("removing {}", account_dir.display()))?;
                }
            }
            println!("{}", fill(t().account_removed, &[&id]));
        }
    }
    Ok(())
}

fn folder(dir: &Path, action: FolderCommand) -> Result<()> {
    match action {
        FolderCommand::List { json } => return folders::run(dir, json),
        FolderCommand::Set {
            kind,
            property,
            value,
        } => {
            let mut settings = settings::load(dir)?;
            settings
                .kind_styles
                .entry(kind)
                .or_default()
                .set(property, &value)?;
            settings::save(dir, &settings)?;
            println!(
                "{}",
                fill(
                    t().folder_set,
                    &[&kind.as_str(), &property.as_str(), &value]
                )
            );
        }
        FolderCommand::Reset { kind, property } => {
            let mut settings = settings::load(dir)?;
            let Some(set) = settings.kind_styles.get_mut(&kind) else {
                println!("{}", fill(t().folder_unset, &[&kind.as_str()]));
                return Ok(());
            };
            let message = match property {
                Some(property) => {
                    set.clear(property);
                    fill(t().folder_reset, &[&kind.as_str(), &property.as_str()])
                }
                None => {
                    *set = style::Overrides::default();
                    fill(t().folder_reset_all, &[&kind.as_str()])
                }
            };
            if set.is_empty() {
                settings.kind_styles.remove(&kind);
            }
            settings::save(dir, &settings)?;
            println!("{message}");
        }
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(untagged)]
enum SyncLine {
    Done {
        account: String,
        #[serde(flatten)]
        counts: sync::Counts,
    },
    Failed {
        account: String,
        error: String,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Password {
    Stored,
    Missing,
    Unused,
    Unknown,
}

fn password_states(dev: Option<&Path>, settings: &Settings) -> Vec<Password> {
    let mut reachable = dev.is_none();
    settings
        .accounts
        .keys()
        .map(|id| {
            if dev.is_some() {
                return Password::Unused;
            }
            if !reachable {
                return Password::Unknown;
            }
            match keyring::lookup(id) {
                Ok(Some(_)) => Password::Stored,
                Ok(None) => Password::Missing,
                Err(err) => {
                    eprintln!("{id}: {err:#}");
                    reachable = false;
                    Password::Unknown
                }
            }
        })
        .collect()
}

fn sender_of(dir: &Path, settings: &Settings, id: &str) -> Option<message::Addr> {
    match show::identities(dir, settings, id) {
        Ok(me) => compose::account_from(&me),
        Err(err) => {
            eprintln!("{id}: {err:#}");
            None
        }
    }
}

#[derive(Serialize)]
struct AccountLine<'a> {
    id: &'a str,
    #[serde(flatten)]
    account: Account,
    from: Option<message::Addr>,
    password: Password,
    kinds: Kinds,
}

fn effective(settings: &Settings, id: &str, account: &Account) -> Account {
    Account {
        color: Some(settings.color(id)),
        archive_by_year: Some(account.archives_by_year()),
        ..account.clone()
    }
}

fn archive_note(account: &Account) -> &'static str {
    if account.archives_by_year() {
        t().archive_yearly
    } else {
        t().archive_flat
    }
}

fn copy_note(smtp: &Smtp) -> &'static str {
    if smtp.copy_to_sent {
        t().copy_on
    } else {
        t().copy_off
    }
}

struct Checked {
    folders: Vec<imap::Folder>,
    smtp: bool,
}

impl Checked {
    fn report(&self) {
        println!("{}", fill(t().login_ok, &[&self.folders.len()]));
        if self.smtp {
            println!("{}", t().smtp_login_ok);
        }
    }
}

async fn check(account: &Account, access: Access) -> Result<Checked> {
    let mut server = Server::connect(account, access.clone()).await?;
    let folders = server.list_folders().await;
    server.logout().await;
    let folders = folders?;
    let smtp = match &account.smtp {
        Some(smtp) => server::check_smtp(smtp, &account.username, &access).await?,
        None => false,
    };
    Ok(Checked { folders, smtp })
}

async fn check_smtp(smtp: &Smtp, username: &str, access: &Access) -> Result<()> {
    if server::check_smtp(smtp, username, access).await? {
        println!("{}", t().smtp_login_ok);
    }
    Ok(())
}

async fn sync(dir: &Path, dev: Option<&Path>, ids: Vec<String>, json: bool) -> Result<()> {
    let accounts = settings::load(dir)?.chosen(ids)?;
    let _lock = store::wait_for_lock(dir)?;

    let mut jobs = Vec::with_capacity(accounts.len());
    for (id, account) in accounts {
        let access = access(dev, &id)?;
        jobs.push((id, account, access));
    }

    let report = if json {
        sync::Report::Progress(std::sync::Arc::new(|progress| {
            output::stdout(|out| output::json_line(out, &progress)).ok();
        }))
    } else {
        sync::Report::Text
    };
    let mut tasks = JoinSet::new();
    for (id, account, access) in jobs {
        let dir = dir.to_path_buf();
        let report = report.clone();
        tasks.spawn(async move {
            let result = sync::sync_account(&dir, &id, &account, access, &report).await;
            (id, result)
        });
    }

    let mut failed = false;
    while let Some(joined) = tasks.join_next().await {
        let (id, result) = joined.context("sync task panicked")?;
        failed |= result.is_err();
        if json {
            let line = match result {
                Ok(synced) => SyncLine::Done {
                    account: id,
                    counts: synced.counts,
                },
                Err(err) => SyncLine::Failed {
                    account: id,
                    error: format!("{err:#}"),
                },
            };
            output::stdout(|out| output::json_line(out, &line))?;
            continue;
        }
        match result {
            Ok(_) => println!("{}", fill(t().sync_done, &[&id])),
            Err(err) => eprintln!("{}", fill(t().sync_failed, &[&id, &format!("{err:#}")])),
        }
    }
    if failed {
        bail!("{}", t().sync_incomplete);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sync_line_carries_the_counts_or_the_error() {
        let done = SyncLine::Done {
            account: "private".to_string(),
            counts: sync::Counts {
                new: 3,
                removed: 1,
                updated: 0,
            },
        };
        assert_eq!(
            serde_json::to_value(&done).unwrap(),
            serde_json::json!({ "account": "private", "new": 3, "removed": 1, "updated": 0 })
        );
        let failed = SyncLine::Failed {
            account: "work".to_string(),
            error: "login failed".to_string(),
        };
        assert_eq!(
            serde_json::to_value(&failed).unwrap(),
            serde_json::json!({ "account": "work", "error": "login failed" })
        );
    }

    #[test]
    fn an_account_line_carries_the_settings_and_the_password_state() {
        let account = Account {
            smtp: Some(Smtp::new("smtp.example.com".to_string(), 465, None)),
            color: Some(Color::Cyan),
            signature: Some("Robin\nexample.com".to_string()),
            context_folder: Some(PathBuf::from("/home/me/context")),
            folders: testing::assigned(&[("inbox", Kind::Inbox)]),
            ..Account::new(
                "imap.example.com".to_string(),
                993,
                "me@example.com".to_string(),
            )
        };
        let settings = Settings {
            accounts: [("private".to_string(), account.clone())].into(),
            ..Settings::default()
        };
        let line = AccountLine {
            id: "private",
            account: effective(&settings, "private", &account),
            from: Some(message::Addr {
                name: Some("Robin".to_string()),
                address: Some("me@example.com".to_string()),
            }),
            password: Password::Missing,
            kinds: Kinds::of(&account),
        };
        assert_eq!(
            serde_json::to_value(&line).unwrap(),
            serde_json::json!({
                "id": "private",
                "host": "imap.example.com",
                "port": 993,
                "username": "me@example.com",
                "smtp": { "host": "smtp.example.com", "port": 465, "copy_to_sent": true },
                "color": "cyan",
                "signature": "Robin\nexample.com",
                "context_folder": "/home/me/context",
                "archive_by_year": true,
                "folders": { "inbox": { "kind": "inbox", "sync": true } },
                "from": { "name": "Robin", "address": "me@example.com" },
                "password": "missing",
                "kinds": {
                    "inbox": "inbox",
                    "sent": null,
                    "drafts": null,
                    "junk": null,
                    "trash": null,
                    "archive": null
                }
            })
        );
    }
}
