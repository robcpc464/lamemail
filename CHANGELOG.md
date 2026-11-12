# Changelog

What changed in each version of lamemail, newest first.

## Unreleased

- A message file keeps the whole header block of its mail (`headers`),
  such as the `Received` lines and `Authentication-Results`, so nothing
  of a mail's origin is lost. Mail synced before gets it once the sync
  folder is built again.
- `lamemail ai check <path>` has the AI judge a mail for spam, phishing,
  fraud and malware: it reads the header lines (SPF, DKIM, DMARC, the
  way the mail took, the reply address) as well as the text and its
  links, and says in a few sentences what gives the mail away.
- Claude Code and ollama models get lamemail's rules as their system
  prompt and the mails apart from them as data, so a mail that tries to
  give the AI instructions has a harder time. Claude Code no longer
  answers as a coding assistant when it drafts a mail.

## 0.11.0

- lamemail can be your mail program: a `mailto:` link, in the browser
  or anywhere else, opens a new mail with its recipients, subject and
  text. Make lamemail the default with `xdg-mime default
  lamemail-ui.desktop x-scheme-handler/mailto`. Links in a mail take
  their subject and text along as well.

## 0.10.2

- The AI gets as much as its model takes: Claude Code 200,000 tokens,
  an ollama model its own context window, in place of 20,000 characters
  of the context folder. `lamemail ai context-limit <tokens>` sets a
  smaller window for the AI that is on, such as for a local model on a
  small graphics card; a prompt too long for it is shortened, the mail to
  answer last. In the settings the context window stands under the model,
  the model's own shown while none is set.

## 0.10.1

- Started at login, lamemail stays in the tray again when the tray comes
  up only after it, as with Omarchy 4.5; closing the window hides it there
  again.

## 0.10.0

- The trash can be emptied: with the trash chosen in the top bar,
  "Empty trash" deletes all mail in it for good, with five seconds to
  undo. `lamemail delete` takes several mails at once, or their paths on
  stdin with `--stdin`.

## 0.9.0

- The mouse wheel scrolls by the distance set under General again; with
  Qt 6.12 it had moved only a few pixels a notch.
- A button at the top right of the settings quits lamemail, also while
  it closes to the tray.

## 0.8.0

- `lamemail-ui --open <path>` opens a mail, its path as
  `lamemail list --json` prints it: in the running window, or in a new
  one. Bar plugins and scripts can open a mail this way.

## 0.7.0

- `/ai` (`/ki` in German) works in a new mail too. The AI gets the
  account's context folder, the earlier mails with the recipients and the
  subject. `lamemail ai suggest <account>` drafts a new mail; a reply is
  now `lamemail ai suggest --reply <path>`, and `--subject` passes the
  subject.

## 0.6.0

- A mail can be moved into any folder of its account: the new folder
  button under the mail opens the folders as a tree with a search, and the
  folders lamemail synchronizes carry the sync icon. Mail moved into a
  folder that is not synchronized leaves lamemail. `lamemail move` takes
  every folder as well, and `lamemail account list --json` names each
  folder as the server writes it.

## 0.5.0

- Links in HTML mail show their text in the accent colour, followed by
  the site they lead to, muted (`See the collection (shop.example.com)`);
  the pointer shows the whole address. The text of a mail holds them as
  `[Text](url)`. Mail synced before keeps the old form until the sync
  folder is built again.
- "Check for updates" in the general settings (off unless switched on)
  asks GitHub once a day whether a newer version is out and then shows a
  dot on the gear and a link to the release; nothing is downloaded.
  `lamemail update check` asks on the command line.

## 0.4.0

- Tables in mail become text tables (`| Item | Price |`), and the UI lines
  up their columns; copying keeps them aligned.
- The HTML part of a mail now always wins over its plain-text version,
  which often lacks the quotes, tables and links.
- Mail reads better:
  - replies from the new Outlook show their original as a quote;
  - signatures made with Templafy are set apart;
  - totals in shop receipts and invoices stay on one line
    (`Subtotal · €81,00`);
  - a link's address follows its text instead of standing alone on the
    next line (buttons, cards);
  - logo and icon links without text no longer show bare tracking
    addresses, and an icon beside a link to the same place is left out;
  - more hidden newsletter preview texts are dropped;
  - `</br>` breaks the line, and subjects no longer end in a line break.
- Images a mail embeds (logos, icons) are no longer listed among its
  attachments: they wait behind a chip with their count. The feed's
  paperclip and "Download all" count only real attachments, and the
  signature file of signed mail (`smime.p7s`) is not listed.
- Mail synced before keeps its old text until the sync folder is built
  again.
