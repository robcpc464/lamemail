# Changelog

What changed in each version of lamemail, newest first.

## Unreleased

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
