# Changelog

What changed in each version of lamemail, newest first.

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
