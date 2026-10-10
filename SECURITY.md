# Security

lamemail reads what strangers send: IMAP responses, MIME, HTML and, with
an AI switched on, text that may try to steer a model. A flaw there
matters, so reports are welcome.

## Reporting a vulnerability

Please do not open a public issue. Use GitHub's private reporting
instead: the "Security" tab of this repository, then "Report a
vulnerability". The report stays between you and the maintainer until a
fix is out.

Helpful in a report:

- the commit you tried
- what you did and what happened, as exact as you can
- a message that shows it

lamemail is a one-person project, so an answer can take a while. Pull
requests are not accepted, for security fixes either: a description is
enough, the fix is written here.

## Supported versions

Only the latest commit of the default branch. There are no releases yet.

## What counts

- memory safety, a crash or a hang caused by a message or by a server's
  answer
- a message that writes outside the sync folder, or an attachment that
  lands outside its folder, becomes executable or is opened unasked
- passwords or mail reaching anything but the account's own servers and
  the AI the user switched on
- a message that makes the AI, or an agent connected through `lamemail
  mcp`, do more than write text: run a program, read files, send mail
- a link or an attachment in the UI that opens something other than what
  it shows
