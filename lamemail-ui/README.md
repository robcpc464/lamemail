# lamemail-ui

The UI for a lamemail sync folder: Qt Quick (QML) for the surface, C++
with Qt 6 for the rest, nothing else. It contains no mail logic: everything
that concerns mail runs through the `lamemail` binary from
`../lamemail-cli`, whose JSON outputs are the contract.

## Building

Qt 6 (6.9 or newer, developed on 6.11) with its QML runtime, QtQuick, Controls, Shapes and
Labs Platform, Qt Network, Qt D-Bus, the Wayland plugin and, for the window
and tray icon, the SVG plugins, qmake6,
make and a C++20 compiler (Arch: `qt6-base`, `qt6-declarative`,
`qt6-wayland`, `qt6-svg`, `base-devel`); on omarchy all of that is already
installed.

```sh
mkdir -p build && cd build
qmake6 -r ../lamemail-ui.pro && make -j"$(nproc)"
./tests/tst_lamemail          # the tests
./app/lamemail-ui --dir <sync folder>
```

`lamemail-ui` runs the `lamemail` next to its executable, else the one in
`PATH`.

`./drive/lamemail-ui-drive` is the same window without a display, for
trying the UI where there is none (a sandbox, a build server): Qt's
offscreen platform with the software renderer, driven step by step and
captured as PNG. It needs Qt Test as well; `--help` lists the steps.

```sh
./drive/lamemail-ui-drive --dir <sync folder> --out <folder> "size 1700 900" "key Ctrl+N" "shot compose"
```

## Usage

```sh
lamemail-ui [--dir <path>] [--hidden]
```

`--hidden` starts lamemail in the tray without its window while "Close
to tray" is on; the start at login uses it.

`lamemail-ui` shows the sync folder as one feed over the full window: every
mail of the folders `lamemail` finds, one row per mail (copies once),
newest first. A row has two lines:
the account tag in the account's colour, then `Sender address →
Recipient address`, with `+n` and a tooltip when there are more
recipients, and the subject below; the time over the attachment,
answered and forwarded icons sits on the right, the star left of them, centred over
both lines. The time column is as wide as the longest date, so the stars
stand in one column. There is no preview: the first line of a mail is
mostly a greeting. Unread mail has a dot in the accent colour at the
top right of its icon and bright, bold text, read mail is dimmed.
There are no folders or trees in the UI and there will not be any. It reads nothing itself: the feed comes from `lamemail list --json`,
`lamemail watch --json` keeps the mirror current while the UI runs,
the sync button runs `lamemail sync --json` (the `lamemail` next to the
executable, else the one in `PATH`), sending runs `lamemail send` and
`lamemail push`, read and star run `lamemail flag`, the contacts come
from `lamemail contacts`, and the folder is watched with inotify so that a
sync from anywhere shows up while the window is open, new mail sliding in
at the top. Toasts speak in icons
where they can: after a sync the number of accounts and, summed over
them, the new, removed and updated messages, each next to its icon (+, −
and a pencil); an error shows the first failed account and how many
failed.

- New mail comes by itself: `lamemail watch --json` starts with the
  UI and ends with it (it gets `SIGTERM` when the UI dies), and
  adding or removing an account or a new password start it again. The
  inbox follows the server at once, everything else every 15 minutes;
  `Ctrl+R` still syncs everything on demand. While any sync runs (the
  button's, `Ctrl+R` or one of `watch`), the sync button in the top bar
  spins and cannot be clicked. When mail arrives while the
  window is not active (`arrived` of `watch --json`: unread, not from
  the user, not in spam or trash), the tray icon (with "Close to tray")
  gets a dot in the accent colour until the window is active again, and a
  desktop notification names the sender and the subject, from three mails
  on one notification ("3 new mails") with the senders. A click on it brings the window up
  and opens the mail. Notifications go over D-Bus to the desktop's
  notification service (`org.freedesktop.Notifications`, mako on
  omarchy); "Notifications" in the general settings turns them off, the
  dot stays. An account `watch` gives up on (the server refuses the
  login, the keyring has no password) is named in a toast; other
  failures are retried by `watch` and only logged
- A quiet status bar at the bottom of the window says what runs: a
  turning sync icon with the accounts being synced, or once the sync
  reports its progress where it stands ("private · inbox 500/3000 ·
  folder 3 of 12"), the outbox icon with
  the number of mails waiting to go out (in red when the last push failed,
  the reason as tooltip), the upload icon with the number of
  changes waiting in the journal (in the accent colour while they are
  pushed), failing accounts in red (the reasons as tooltip), and
  otherwise "Up to date", with the time of the last sync as its tooltip;
  at its right end an envelope with the number of unread mails in the
  feed, in the accent colour. Items
  appear only after 400 ms, so short syncs stay invisible; there is no
  running line under the top bar
- The feed is split by small headers: today, yesterday, older (Heute,
  Gestern, Älter), so new mail stands out; the header of the part being
  scrolled through stays at the top of the list; the mouse wheel moves a
  fixed number of rows per notch (one per 60 px of its setting), and it and
  `PageUp` `PageDown` stop at the start of a row right below the header; starred mail stands above
  them under "Starred" (Markiert), newest first (`lamemail list --json
  --flagged-first`), unless turned off in the settings
- A filter: the feed shows the kinds of folders chosen under "In the
  feed" in the Feed section of the settings (inbox, sent and drafts unless changed,
  `lamemail list --json --kind`). The six kind icons in the top bar show
  one kind alone: a click lights it up in the accent colour and shows only
  that kind, a second click goes back to the chosen kinds; the choice is
  forgotten when the UI closes. The unread count in the status bar follows
  the filter
- Search: the magnifier in the top bar, or `/`, opens a field over the
  full width; the feed follows the typed words (`lamemail list --json
  --search`: every word, in sender, recipients, subject or text) in every
  kind, whatever the filter (its icons are dimmed meanwhile), the
  field counts the hits at its right end, `↓` or `Enter` go into the list,
  `Esc` empties and closes the field
- A row names sender and first recipient; an address that follows a name
  is smaller and dimmer, without angle brackets, so the names stand out
- A row follows the style of its folder's kind (see "Folder styles" in
  `../lamemail-cli/README.md`): the large icon on the left (by default an
  arrow pointing in for the inbox and out for sent mail, like a call
  log) on a tile in the kind's `color` (by default inbox blue, sent green,
  drafts yellow, junk orange, trash cyan, archive magenta; grey while the
  kind is `dim`), so
  the kind of a row shows at a glance, `dense` for one line instead of two, in a smaller font, `dim`,
  `strike` and `italic` for the text, `indent` moves icon and text right
  by 16 px per level, 1 to 3, while the time stays (no kind is indented
  by default).
- A click on a row, or `Enter`, opens the mail as its own view (`lamemail
  show --json`): subject, sender, recipients, time, attachments, then the
  text. Quotes are collapsed into badges with the "… wrote" line; a click
  unfolds the quoted text in place, older quotes inside are badges again.
  Signatures are muted. Selecting text copies it to the clipboard once
  no further click follows (a double or triple click copies its final
  selection), a small "Copied" hint at the end of the selection says so. Web addresses in the text (`http://`, `https://`,
  `www.`, bare or as `Text <url>`) are links in the accent colour that a
  click opens in the browser; the address always stays visible. An
  address of 60 characters or more is shown shortened, its host in full
  and 50 characters in all, then `…`; the pointer on it shows the whole
  address, the click opens it, and the text of the mail keeps it. Copying
  by selecting takes what is shown, so the shortened form. A pipe table
  of the text (a `| … |` line, `|---|` under it, more `| … |` lines) has
  its cells filled with spaces to the width of their column, in the
  monospace font the columns line up, and copying takes them aligned;
  the text of the mail keeps the table as it came. Mail
  addresses are links as well: a click starts a new mail to the address,
  from the account of the mail unless the contacts know a better one. A click
  on an attachment fetches it with `lamemail attachment` into
  `$XDG_RUNTIME_DIR/lamemail/attachments` and opens it through the XDG
  desktop portal: with the default program, else the system's "Open with"
  dialog; without a portal with `xdg-open`. The paperclip pulses
  meanwhile, a failure comes as a toast. Images the mail's HTML embeds
  (`inline`) stand behind one chip with the image icon and their count,
  a click shows them as chips too. The chip with the download
  arrow after the others saves all of those into the download folder of the
  settings (`lamemail attachment --to`), never asking for a folder; a
  name already there gets ` (1)`, the toast counts the files and names
  the folder. The reply area is always open and has the focus:
  from about 1300 px window width it sits right of the mail, below that
  under it. Dragging the border between them sets the width or the height,
  kept in `~/.config/lamemail/lamemail-ui.conf`. Drafts are kept per mail
  while the app runs. `Esc` or the arrow leads back
- Read and star: opening an unread mail marks it read (`lamemail flag
  <path> seen`, noted in the journal at once). In the feed `Ctrl+U` switches the
  current row between read and unread and `Ctrl+D` stars it or takes the star
  away (`\Flagged`, the star of other clients); a row shows its star on
  hover, and a click on it does the same. The bar under the mail in the
  detail view has the star and a read button with two states like the row: an
  envelope while the mail is read, with an open dot in the accent colour
  while it is unread; a click switches it and the view stays. Every icon
  button explains itself in a tooltip, with its key where one exists.
  The row changes at once, the commands run one after another,
  and a list read while they run is not merged but read again after them.
  Ten seconds after the journal last changed, `lamemail push --json` sends
  it to the server, bundled and without loading the server with every
  click (a fixed value from experience, not a setting); the status bar shows the waiting entries with the upload icon,
  and a change the server refused (the mail is gone) comes as a toast
- Archive, trash and back: what a mail offers follows the `kind` of its
  folder from `lamemail`. Mail in the inbox or sent gets archive
  and trash, in the archive back to the inbox and trash, in
  junk back to the inbox ("not spam") and trash, in the trash back to the
  inbox only, a draft trash only. `Ctrl+E` does the first (archive or back to
  the inbox), `Ctrl+Delete` the trash, where the mail offers it (`lamemail move
  --json <path> archive|trash|inbox`, noted in the journal at once); the
  bar under the mail in the detail view has the same buttons and goes
  back to the feed. They always act on the mail on the left, also beside a draft of
  a reply, and the toast names it by its subject. The row stays and takes
  the style of its new folder; after a delete the selection goes on to
  the next row. In the trash, `Ctrl+Delete` and a button with the trash
  icon in that bar delete the mail for good (`lamemail delete <path>`), held for
  five seconds with "Undo" as a reply is, then pushed at once.
  A toast offers "Undo" for five seconds (a click, `Ctrl+Z`, or `Esc` in
  the feed), which moves the mail back to where it was; moves made while
  it shows join it, it counts them by target and starts its five seconds
  again, and undo takes all of them back; a mail moved back by hand leaves
  it. The push comes only after the ten seconds, and the UI holds its
  own pushes while the undo is offered, so an undo meets the moves still
  in the journal, and a move back drops the entry. Undo always takes back
  what the toast shows: a held reply or the moves, whichever came last.
  A row whose mail is on its way spins its icon: it cannot be opened while
  a reply to it or its delete is held or its move can be undone, and it opens while a
  move of it or a reply to it waits for the push
- The composer, the same for a reply and a new mail: one line with the
  account tag, the sender and the recipients, `Sender → Recipient` as in
  the feed, then a card with the subject as its first line and the text.
  Sender and recipients are framed. A click on the sender edits the part
  before the `@` in place, for an alias; the domain belongs to the
  account (an account without an own address yet takes a whole one). `Esc`
  in it or leaving it empty brings back the one `lamemail` proposes.
  Recipients stand in the order they were added; Cc and Bcc carry a tag
  at the start of their chip (`CC`, `BCC`). A click on one switches it
  To → Cc → Bcc in place, its cross removes it. After them comes the
  field for the next one, always marked by its placeholder, also where it
  wraps into a line of its own. Typing a name or an address suggests the
  known contacts (`lamemail contacts --json`, the rows of the new mail
  view) under the field, or above it when there is no room: `↑` `↓`
  choose, `Enter` or `Tab` takes one as To, a click too, `Ctrl`+click as
  Cc, `Esc` drops what was typed. Addresses that are not known are added
  with `Enter` or a comma, `Backspace` removes the last recipient, `Enter`
  in an empty field goes on to the subject, and from there to the text
- Sending: `Ctrl+Enter` or the send button at the bottom right of the
  composer (its tooltip names the key). A placeholder in square brackets
  left in the text, not in a quoted line, holds the first send back with
  a warning; sending the same text again sends it. The reply
  starts with the sender, the recipient and the subject from `show --json`
  (`answer_from`, `answer_to`, `answer_subject`), and all of it can be
  changed; when the mail had other recipients, an icon chip at the end of
  its "to" line ("Reply all") adds them as Cc
  (`answer_cc`) and takes them away again. A chip at the start of the
  subject line, "Reply" or "Forward", switches the subject between
  `answer_subject` and `forward_subject`; a subject changed by hand keeps
  its words and gets `Re: ` or `Fwd: ` in place of the prefix it has, or
  had before it was deleted. A forward drops the reply's recipient and gets
  the mail's attachments, switching back brings the recipients as they
  were; the text stays as it is, the draft keeps the choice. The view returns to the feed and a toast
  counts down five seconds with "Undo" (or `Ctrl+Z`, or `Esc` in the feed),
  which opens the mail again with the draft. Then `lamemail send --json
  --reply` notes it in the journal with `--from` (only when changed), `--to`,
  `--cc`, `--bcc` and `--subject`, the text on stdin, the draft is gone,
  and `lamemail push --json` sends it at once, without waiting the ten
  seconds of read and star. The toast says who got it, and the copy from
  the sent folder is in the feed, fetched by the push. A mail that cannot
  be noted keeps its draft and shows the error. Offline, or when the
  server cannot be reached, the mail waits in the journal (the outbox
  icon in the status bar, a toast with the reason) and goes out with the
  next push, also the one of `lamemail watch` before it syncs. A mail
  the SMTP server refuses (an unknown recipient) comes back as the draft
  of its mail or of the new mail view, unless a new draft is there, and
  the toast gives the server's words; a failed copy in the sent folder is
  a warning, the mail is out. Closing the window notes and pushes what is
  waiting first and waits for the push
- Attachments: the paperclip at the bottom left of the composer opens the file dialog
  (Qt's, native through the platform theme), and files dropped onto the
  composer are attached too. They stand as chips at the bottom of the
  card, a click removes one; `lamemail send` gets them as `--file`, those
  of a message as `--attachment <path>:<index>` and those of the draft it
  continues as `--attachment <index>`, and the push reads them when it
  sends.
- Drafts live on the server, in the account's drafts folder, so another
  device can continue them. What the composer holds stays in memory while
  the UI runs; the floppy disk left of the send button, or `Ctrl+S`, saves it with
  `lamemail draft --json` and pushes at once, and the composer keeps the
  draft's `message_id` for the next save and for sending (`--draft`), which
  replace and delete the saved version. A row of the drafts folder opens
  in the new mail view with the draft's people, subject, text and
  attachments; the draft of a reply opens in the detail view of the mail
  it answers (`parent` of `show --json`), its reply area holding the
  draft, so quote, AI, the chips and the answered flag work as in any
  reply. While the composer holds a saved draft, the trash icon left of
  the floppy disk deletes it for good (`lamemail discard`) after the same
  five seconds of undo as sending. The composer's buttons are icons,
  their labels stand in the tooltips.
- Commands: `/` at the start of a line or after a space in the text opens
  a menu at the cursor, filtered by the letters that follow; `↑` `↓`
  choose, `Enter` or `Tab` runs, `Esc` closes it. In a reply `/quote`
  (`/zitat` in German) inserts the mail quoted, `quote` of `show --json`,
  folded: the "… wrote" line stays with a chip that unfolds and folds the
  `>` lines, which stay part of the text. Nothing is quoted otherwise, the
  text is the whole mail. `/signature` (`/signatur`) inserts the account's
  signature under a `-- ` line, grey while the account has none,
  `/attach` (`/anhang`) opens the file dialog, and `/date` (`/datum`)
  inserts today's date, short, medium or long. In a reply `/ai <instruction>` (`/ki` in German)
  has the AI write the reply: `lamemail ai suggest --json` with the
  instruction on stdin, the composer's To and Cc and the text before and
  after the command line, which marks the place; the AI answers with the
  whole reply, which replaces the text in one step (`Ctrl+Z` brings it
  back). The exchange runs in a panel at the bottom of the composer's
  card: while the AI thinks, a pulsing sparkle and a running line, `Esc`
  stops it (the whole process group, the AI program included); a question
  of the AI with a field for the answer (`Enter` sends it, "Go on without
  an answer" lets it write with a placeholder), at most three; afterwards
  what the AI got, one line that opens into the context files, the
  searches with their hits and the earlier mails. The text is read-only
  meanwhile; a failure comes as a toast and leaves the line to try again. Without an AI set, and in a new mail,
  the entry is grey with a hint
- New mail: the pen in the top bar or `Ctrl+N` opens its own view, split
  like the detail view (the same border, kept separately). On the left
  every contact from `lamemail contacts --json`, the people written to
  first, each with the tag of the account last used with it and the time
  of the newest mail; the search above filters by name and address, words
  in any order, `↑` `↓` move and `Enter` adds the highlighted contact. A
  click adds to To, `Ctrl`+click (or `Ctrl+Enter` in the search) to Cc;
  added contacts get a check, and a click on one of them removes it
  again. On the right the composer; its sender is the
  account's (`from` of `account list --json`). The account follows the
  first To until its tag is clicked, which switches to the next account,
  or the sender is edited. `Ctrl+Enter` or the send button starts the same five
  seconds of undo as a reply; `lamemail send --json` notes it and the push
  sends it, as for a reply
- Settings (gear icon or `Ctrl+,`, back with `Esc`): sections in a
  sidebar (`↑` `↓` switch, the last one is remembered; below 900 px the
  sidebar shrinks to its icons), each with a title, a hint and its rows;
  the version of lamemail stands at the bottom of the sidebar, and a
  click on it opens "Info":
  - General: the theme (System, the omarchy theme; Light or Dark,
    lamemail's own palettes), pixels per mouse wheel notch (120 unless changed), for the
    feed, the mail text and the folder table; a notch starts a flick that
    comes to rest that far down, a touchpad scrolls by its own pixels.
    "Close to tray" (off unless changed): the icon sits in the tray, and
    closing the window only hides it, so lamemail keeps running; a click
    on the icon shows or hides the window, its menu opens or quits, and
    `Ctrl+Q` quits. Without a tray the row says so and closing quits.
    The tray is `SystemTrayIcon` of `Qt.labs.platform`, which Qt serves
    over D-Bus as a StatusNotifierItem (the omarchy shell, KDE and Waybar
    show those) without Qt Widgets; where no tray is found Qt prints that
    it would need Qt Widgets, which lamemail leaves out.
    "Autostart" (off unless switched on) writes `lamemail-ui.desktop`
    into `~/.config/autostart`, which the desktop starts at login (on
    Omarchy through uwsm); off removes it. The entry starts the running
    program with `--hidden`, the AppImage where it lies, and the window
    shows unless "Close to tray" is on and a tray is there.
    "Notifications" (on unless changed) announces new mail on the desktop
    while the window is in the background.
    "Check for updates" (off unless switched on) asks once a day, through
    `lamemail update check`, whether a newer version is out; then a dot
    sits on the gear, the version at the bottom of the sidebar names the
    new one, and "Info" links to its release, which says how to install
    it. Nothing is downloaded.
    "Shadows" (on unless changed) puts a soft shadow under menus, tooltips
    and messages; it needs the Qt module `QtQuick.Effects` (part of
    `qt6-declarative` on Arch, `qml6-module-qtquick-effects` on Debian and
    Ubuntu), without it the row is grey and says so, and everything else
    works.
    "Starred on top" (on unless changed) keeps starred mail in its own
    section above today.
    "Download folder" is where "Download all" saves attachments; empty
    means the system's download folder (`~/Downloads` or what
    `user-dirs.dirs` names), `~` and paths relative to the home work
  - Accounts: every account from `lamemail account list --json` with its
    login, IMAP and SMTP server, and a warning when the keyring has no
    password or cannot be reached; "Test" logs in and shows the result in
    the row. "Edit" opens the tag colour (a row of theme colours, `lamemail
    account color`), the SMTP server, a new password right under it, the
    copy to the sent folder (switched right away), the signature
    (`lamemail account signature`), "Archive by year" (on or off, switched
    right away by `lamemail account archive <id> --by-year`), the default
    folders (every folder of the account from `folders`, by its local
    path, with its kind as a chip
    that opens the kinds, "Other" among them, a "Default" pill lit on the
    folder `kinds` names and a "Sync" pill; the kinds without a folder in
    red above; every change runs `lamemail account folder` and, but for a
    new default, syncs the account; the default folder of a kind is where
    archive, delete and back move mail) and removing (with or without the
    synced mail, after a second click). "Add account" asks for short
    name, IMAP server, login, password and SMTP server; empty ports mean
    the defaults of `lamemail`. The password goes on stdin to `lamemail
    account add` and from there into the keyring, and the new account is
    synced right away
  - AI: first a warning on data protection (mail sent to an AI service
    on the internet goes to its provider, which as a rule needs the
    consent of the people in it; confidentiality notices; possible legal
    consequences; a local model such as Ollama keeps mail on the
    computer), then two parts under headings of their own. "Model": every
    AI program and ollama model lamemail knows (`lamemail ai show --json`),
    the ollama models first, then Claude Code next to the custom command,
    one row each with the link to get it and below the name the command
    line it runs (Claude Code) or a description, grey with "Not found"
    when it is missing, "Not running" for ollama when its service does not
    answer, the others with a switch: on sets it (`lamemail ai set
    claude-code` or `ai set ollama <model>`), off removes it (`ai
    remove`); at most one is on, and none until the user switches one on.
    Below, "Custom command" holds the command line of the user's own
    (`custom` of `ai show --json`), kept while another is on, and always a
    switch: on runs `ai set custom <command>` with the line in the field,
    off `ai remove`, dimmed while the field is empty; "Save" applies a
    changed line while it is on; its note stands under the whole row. "Context": first
    "Length", how many characters of the files the AI gets at most, the
    steps of `context_limits` as pills (`lamemail ai context-limit`), then
    per account its folder of text files (with `~` for the home)
    or "No folder", "Choose folder" opens the system's folder dialog
    (`lamemail account context <id> <dir>`), "Remove" drops it; the
    section scrolls
  - Feed: first "In the feed", the kinds the feed shows as icon pills
    (several, at least one), then one row per kind of folder (inbox, sent,
    drafts, junk, trash, archive), summed over all accounts, each with its icon as a chip (the
    icon's name as tooltip, a click opens the icons to choose from), its
    colour as a chip (a click opens the theme colours as swatches, those of
    the account tags) and
    the flags (dense, dim, struck, italic) as pills, then "indented" with
    its level, which a click steps through 1, 2, 3 and off. Every click
    runs `lamemail folder set <kind>`; values set explicitly get an accent
    border, the arrow at the end of a row resets the kind to its defaults
  - CLI: every command of `lamemail` with its usage, arguments and help,
    read from `lamemail commands --json`
  - Info: the version (and a newer one with its release, see "Check for
    updates"), links to the source code and to the issues on
    GitHub, the licence (GPL-3.0-or-later, its text at gnu.org), the
    software used (Qt, the Lucide icons, the Rust libraries) and a
    notice on warranty and liability

  What only concerns the UI (the mouse wheel, the tray, notifications, the download folder, the kinds in the feed, the detail layout) is kept in
  `~/.config/lamemail/lamemail-ui.conf`; what concerns mail (folder styles)
  goes through `lamemail`
- One instance per sync folder: a second `lamemail-ui` on the same
  folder shows the window of the running one and ends (a local socket
  `lamemail-ui-<hash of the folder>` in `$XDG_RUNTIME_DIR`; the Wayland
  activation token of the second start is handed over, so the window
  may take the focus)
- Keys: no action on a single key. `↑` `↓` move, `Enter` opens, `Esc`
  closes, `PageUp` `PageDown` scroll a page, `Home` `End` jump, `Ctrl+U`
  read or unread, `Ctrl+D` star, `Ctrl+E` archive or back to the inbox,
  `Ctrl+Delete` trash, in the trash for good,
  `/` search, `Ctrl+N` new mail, `Ctrl+Enter` sends, `Ctrl+S` saves a draft,
  `Ctrl+Z`, or `Esc` in the feed, takes it back while the toast counts
  down, `Ctrl+R` or `F5` sync, `Ctrl+,` settings, `Ctrl+Q` quit
- Colours follow the current omarchy theme
  (`~/.local/state/omarchy/current/theme/colors.toml`), live on `omarchy
  theme set`, unless Light or Dark is chosen under General; without omarchy
  the built-in dark palette is used. The top bar, the page headers and
  the status bar stand apart in the theme's darker background. Buttons,
  chips, tags and toasts have rounded corners, buttons with only an icon
  are squares. The font
  is the `monospace` fontconfig alias
- Icons are [Lucide](https://lucide.dev) paths drawn by Qt Quick Shapes
  (`qml/Icon.qml`, ISC licence in `LICENSE-lucide`), so no icon font
  has to be installed anywhere
- Messages are English, or German when the locale starts with `de`
- The icon is `lamemail.svg`, a plain envelope (2026-10-08), white
  lines only. The menu entry names it (`Icon=lamemail`), and the app
  sets it as its window icon from its resources (drawn by Qt SVG's icon
  plugin; without it the window has no icon). Wayland compositors mostly
  take the icon from the menu entry that matches the app id
  `lamemail-ui`; Hyprland does not show window icons
- Installing: `./dev install` in the repository root builds both
  programs and puts `lamemail` and `lamemail-ui` into `~/.local/bin`,
  `lamemail-ui.desktop` into `~/.local/share/applications/` (the app
  menu) and `lamemail.svg` into
  `~/.local/share/icons/hicolor/scalable/apps/`; `./dev uninstall`
  removes exactly these four files and the entry of "Autostart"
