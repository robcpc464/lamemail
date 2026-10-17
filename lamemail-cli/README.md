# lamemail-cli

`lamemail` mirrors IMAP mailboxes into a folder of plain JSON text files
and owns every mail action: sync, send, move, flag, drafts, attachments
and the AI. The folder is the mailbox, and any tool that reads text can
read it. The UI in [`../lamemail-ui`](../lamemail-ui) runs `lamemail` for
everything that concerns mail.

- an exact mirror, server to disk: what disappears on the server
  disappears on disk, and syncing never marks anything as read
- text only: HTML becomes text, attachments are listed and fetched only
  on demand, raw messages are never stored
- changes are noted in a journal at once, offline too, and `push` sends
  them to the server
- passwords live only in the Secret Service keyring, through `secret-tool`

[`FORMAT.md`](FORMAT.md) documents the sync folder, how every command
behaves and what it prints with `--json`.

## Building

Rust is installed into the project folder, nothing is touched outside it:

```sh
./setup                      # once: installs Rust into .toolchain/
./cargo build --release      # -> target/release/lamemail
```

`rm -rf .toolchain target` removes it again. The binary needs glibc and,
for passwords, `secret-tool` (package `libsecret`) with a running Secret
Service.

## Usage

The sync folder is the only configuration: `--dir <path>`, else
`$LAMEMAIL_DIR`, else `~/.lamemail`. `lamemail <command> --help` explains
each command.

```
lamemail init                                      create the folder and an empty settings.json
lamemail account add <id> --host <host> --user <login> [--port 993] [--smtp-host <host>] [--smtp-port 465]
lamemail account smtp <id> [--host <host>] [--port 465] [--copy <true|false>]
lamemail account color <id> <color>                blue, green, magenta, cyan, yellow or orange for the account's tag
lamemail account signature <id>                    the account's signature from stdin, empty removes it
lamemail account context <id> [<dir>]              the account's context for the AI, a folder of text files; none removes it
lamemail account folder <id> <folder> <kind>       an assigned folder's kind (or other), [--sync <bool>] [--default]
lamemail account archive <id> --by-year <bool>     archived mail into a folder per year below the archive, or not
lamemail account list [--json]
lamemail account test <id>                         log in (IMAP, and SMTP when set) and list the server folders
lamemail account password <id>                     replace the password in the keyring
lamemail account remove <id> [--purge]             --purge also deletes the synced mail
lamemail sync [<id>...] [--json]                   mirror all (or the given) accounts once
lamemail watch [<id>...] [--json]                  keep mirroring them: the inbox at once (IMAP IDLE), all every 15 minutes
lamemail list [--json] [--search <words>] [--kind <kind>...] [--flagged-first]
                                                   one line per mail of the folders found, newest first
lamemail show <path> [--json]                      one message: headers and text, or the whole JSON object
lamemail thread <path> [--json]                    the history with the people of a message, replies nested
lamemail attachment <path> [<index>] [--to <dir>] [--json]
                                                   fetch one or all attachments, print their paths
lamemail send [<id>] [--reply <path>] [--forward <path>] [--all] [--from <address>] [--to <address>...]
              [--cc <address>...] [--bcc <address>...] [--subject <text>] [--draft <message-id>]
              [--attachment [<path>:]<index>...] [--file <path>...] [--json]
                                                   a new mail, a reply or a forward, its text read from stdin: noted at once
lamemail draft …                                   the same, saved as a draft into the drafts folder
lamemail discard <id> <message-id> [--json]        delete a saved draft for good with the next push
lamemail flag <path> <change>                      seen, unseen, flagged, unflagged, answered, forwarded, junk or notjunk:
                                                   noted in the journal at once
lamemail push [--json]                             send the journal to the server, bundled, and the mail it holds
lamemail move <path> <folder> [--json]             into another folder of the account (or archive, trash, junk, inbox)
lamemail delete <path>                             a message in the trash, for good with the next push
lamemail journal [--json]                          what waits in the journal for the next push
lamemail contacts [--json]                         everyone known from the mail, the people written to first
lamemail folder list [--json]                      the kinds of folders with their style, summed over all accounts
lamemail folder set <kind> <property> <value>      icon, dense, dim, strike, italic, indent or color for a kind
lamemail folder reset <kind> [<property>]          back to the defaults of that kind
lamemail commands [--json]                         every command with its usage, arguments and help
lamemail update check [--json]                     is there a newer version (asks GitHub, downloads nothing)
lamemail mcp                                       serve the mail to AI agents (Model Context Protocol, stdio)
lamemail ai set claude-code                        Claude Code answers for the AI, without tools
lamemail ai set ollama <model>                     or a model of the local ollama service
lamemail ai set custom [<command>]                 or a program of your own, run without a shell
lamemail ai show [--json]                          the AI set, and the known programs and ollama models
lamemail ai remove                                 no mail goes to an AI any more
lamemail ai context-limit <chars>                  how much of the context folder the AI gets: 10000, 20000, 50000 or 100000
lamemail ai suggest <path> [--to <address>...] [--cc <address>...] [--before <text>] [--after <text>] [--no-questions] [--json]
                                                   the whole text of a reply drafted or revised by the AI, the instruction from stdin
```

`account add` and `account password` hand the password to `secret-tool`,
which asks for it on the terminal or takes it from stdin. Sending needs an
SMTP server per account (`account add --smtp-host` or `account smtp`); it
logs in like IMAP and never sends without TLS.

```sh
lamemail init
lamemail account add private --host imap.example.com --user me@example.com --smtp-host smtp.example.com
lamemail sync
echo "Thanks, see you Tuesday." | lamemail send --reply private/inbox/2026-09-15_1432-offer-for-project-4711.json
lamemail push
```

## The sync folder

```
~/.lamemail/
├── settings.json              accounts, folders, styles, the AI
├── .journal.json              changes and mail not pushed yet
└── private/                   one directory per account
    ├── inbox/
    │   ├── .folder.json       sync state of this folder
    │   └── 2026-09-15_1432-offer-for-project-4711.json
    └── archive/
        └── 2025/              server folders become subdirectories
```

Each message is one JSON file: headers, flags, the text and the list of
attachments. Messages are printed in English, or in German when the
locale starts with `de`.
