use std::net::SocketAddr;

use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

use crate::net::{self, Refused, timed};

const IMPLICIT_TLS_PORT: u16 = 465;
const MAX_LINE: u64 = 4096;
const MAX_LINES: usize = 100;

pub struct Login<'a> {
    pub host: &'a str,
    pub port: u16,
    pub username: &'a str,
    pub password: &'a str,
}

pub struct Envelope<'a> {
    pub from: &'a str,
    pub recipients: &'a [String],
    pub message: &'a [u8],
}

struct Reply {
    code: u16,
    lines: Vec<String>,
}

struct Client<S> {
    stream: BufReader<S>,
}

pub async fn check(login: &Login<'_>) -> Result<()> {
    let mut client = open(login).await?;
    client.quit().await;
    Ok(())
}

pub async fn send(login: &Login<'_>, envelope: &Envelope<'_>) -> Result<()> {
    let mut client = open(login).await?;
    let result = client.deliver(envelope).await;
    client.quit().await;
    result
}

pub fn is_address(address: &str) -> bool {
    address.contains('@')
        && !address
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '<' || c == '>')
}

async fn open(login: &Login<'_>) -> Result<Client<TlsStream<TcpStream>>> {
    let tcp = net::tcp(login.host, login.port).await?;
    let name = ehlo_name(tcp.local_addr().context("local address")?);
    let mut client = if login.port == IMPLICIT_TLS_PORT {
        let mut client = Client::new(net::tls(login.host, tcp).await?);
        client.greeting().await?;
        client
    } else {
        let mut plain = Client::new(tcp);
        plain.greeting().await?;
        if !plain
            .ehlo(&name)
            .await?
            .iter()
            .any(|line| line == "STARTTLS")
        {
            bail!(
                "{} offers no STARTTLS; lamemail sends only over TLS (port {IMPLICIT_TLS_PORT} or STARTTLS)",
                login.host
            );
        }
        plain.command("STARTTLS", "STARTTLS", 2).await?;
        Client::new(net::tls(login.host, plain.into_inner()?).await?)
    };
    let capabilities = client.ehlo(&name).await?;
    client
        .authenticate(login, &capabilities)
        .await
        .with_context(|| format!("SMTP login as {} failed", login.username))?;
    Ok(client)
}

fn ehlo_name(local: SocketAddr) -> String {
    match local {
        SocketAddr::V4(address) => format!("[{}]", address.ip()),
        SocketAddr::V6(address) => format!("[IPv6:{}]", address.ip()),
    }
}

fn mechanisms(capabilities: &[String]) -> Vec<&str> {
    capabilities
        .iter()
        .filter_map(|line| {
            line.strip_prefix("AUTH ")
                .or_else(|| line.strip_prefix("AUTH="))
        })
        .flat_map(str::split_whitespace)
        .collect()
}

fn dot_stuffed(message: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(message.len() + 8);
    let mut line_start = true;
    for &byte in message {
        if line_start && byte == b'.' {
            out.push(b'.');
        }
        out.push(byte);
        line_start = byte == b'\n';
    }
    if !out.ends_with(b"\r\n") {
        out.extend_from_slice(b"\r\n");
    }
    out.extend_from_slice(b".\r\n");
    out
}

fn parse_line(line: &str) -> Option<(u16, bool, &str)> {
    let code = line.get(..3)?;
    if !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let code = code.parse().ok()?;
    match line.as_bytes().get(3) {
        None => Some((code, false, "")),
        Some(b' ') => Some((code, false, &line[4..])),
        Some(b'-') => Some((code, true, &line[4..])),
        Some(_) => None,
    }
}

impl std::fmt::Display for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.code, self.lines.join(" ").trim())
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> Client<S> {
    fn new(stream: S) -> Client<S> {
        Client {
            stream: BufReader::new(stream),
        }
    }

    fn into_inner(self) -> Result<S> {
        if !self.stream.buffer().is_empty() {
            bail!("STARTTLS: the server sent data before the TLS handshake");
        }
        Ok(self.stream.into_inner())
    }

    async fn greeting(&mut self) -> Result<()> {
        self.expect("greeting", 2).await.map(drop)
    }

    async fn ehlo(&mut self, name: &str) -> Result<Vec<String>> {
        let reply = self.command(&format!("EHLO {name}"), "EHLO", 2).await?;
        Ok(reply
            .lines
            .into_iter()
            .skip(1)
            .map(|line| line.trim().to_ascii_uppercase())
            .collect())
    }

    async fn authenticate(&mut self, login: &Login<'_>, capabilities: &[String]) -> Result<()> {
        let offered = mechanisms(capabilities);
        if offered.contains(&"PLAIN") {
            let token = STANDARD.encode(format!("\0{}\0{}", login.username, login.password));
            self.command(&format!("AUTH PLAIN {token}"), "AUTH PLAIN", 2)
                .await?;
        } else if offered.contains(&"LOGIN") {
            self.command("AUTH LOGIN", "AUTH LOGIN", 3).await?;
            self.command(&STANDARD.encode(login.username), "AUTH LOGIN", 3)
                .await?;
            self.command(&STANDARD.encode(login.password), "AUTH LOGIN", 2)
                .await?;
        } else {
            bail!(
                "{} offers no password login (AUTH PLAIN or LOGIN)",
                login.host
            );
        }
        Ok(())
    }

    async fn deliver(&mut self, envelope: &Envelope<'_>) -> Result<()> {
        for address in
            std::iter::once(envelope.from).chain(envelope.recipients.iter().map(String::as_str))
        {
            if !is_address(address) {
                bail!("`{address}` is not a mail address");
            }
        }
        self.transact(&format!("MAIL FROM:<{}>", envelope.from), "MAIL FROM", 2)
            .await?;
        for recipient in envelope.recipients {
            self.transact(&format!("RCPT TO:<{recipient}>"), "RCPT TO", 2)
                .await
                .with_context(|| recipient.clone())?;
        }
        self.transact("DATA", "DATA", 3).await?;
        self.write("DATA", &dot_stuffed(envelope.message)).await?;
        self.accepted("DATA", 2).await
    }

    async fn transact(&mut self, line: &str, what: &str, class: u16) -> Result<()> {
        self.write(what, format!("{line}\r\n").as_bytes()).await?;
        self.accepted(what, class).await
    }

    async fn accepted(&mut self, what: &str, class: u16) -> Result<()> {
        let reply = self.reply(what).await?;
        match reply.code / 100 {
            got if got == class => Ok(()),
            5 => Err(Refused(format!("{what}: {reply}")).into()),
            _ => bail!("{what}: {reply}"),
        }
    }

    async fn quit(&mut self) {
        let _ = self.command("QUIT", "QUIT", 2).await;
    }

    async fn command(&mut self, line: &str, what: &str, class: u16) -> Result<Reply> {
        self.write(what, format!("{line}\r\n").as_bytes()).await?;
        self.expect(what, class).await
    }

    async fn expect(&mut self, what: &str, class: u16) -> Result<Reply> {
        let reply = self.reply(what).await?;
        if reply.code / 100 != class {
            bail!("{what}: {reply}");
        }
        Ok(reply)
    }

    async fn write(&mut self, what: &str, bytes: &[u8]) -> Result<()> {
        let stream = self.stream.get_mut();
        timed(what, async {
            stream.write_all(bytes).await?;
            stream.flush().await
        })
        .await?
        .with_context(|| what.to_string())
    }

    async fn reply(&mut self, what: &str) -> Result<Reply> {
        let mut lines = Vec::new();
        loop {
            let mut raw = Vec::new();
            timed(
                what,
                (&mut self.stream)
                    .take(MAX_LINE)
                    .read_until(b'\n', &mut raw),
            )
            .await?
            .with_context(|| what.to_string())?;
            if !raw.ends_with(b"\n") {
                bail!("{what}: the server closed the connection or sent an overlong line");
            }
            let text = String::from_utf8_lossy(&raw);
            let line = text.trim_end_matches(['\r', '\n']);
            let (code, more, rest) =
                parse_line(line).with_context(|| format!("{what}: unexpected reply `{line}`"))?;
            lines.push(rest.to_string());
            if !more {
                return Ok(Reply { code, lines });
            }
            if lines.len() >= MAX_LINES {
                bail!("{what}: the reply has more than {MAX_LINES} lines");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{DuplexStream, duplex};

    struct Server {
        stream: BufReader<DuplexStream>,
    }

    impl Server {
        async fn expect(&mut self, line: &str) {
            let mut got = String::new();
            self.stream.read_line(&mut got).await.unwrap();
            assert_eq!(got, format!("{line}\r\n"));
        }

        async fn say(&mut self, text: &str) {
            self.stream
                .get_mut()
                .write_all(text.as_bytes())
                .await
                .unwrap();
        }
    }

    fn pair() -> (Client<DuplexStream>, Server) {
        let (client, server) = duplex(64 * 1024);
        (
            Client::new(client),
            Server {
                stream: BufReader::new(server),
            },
        )
    }

    fn login() -> Login<'static> {
        Login {
            host: "smtp.example.com",
            port: 465,
            username: "me@example.com",
            password: "secret",
        }
    }

    #[tokio::test]
    async fn logs_in_with_plain_and_delivers_dot_stuffed() {
        let (mut client, mut server) = pair();
        let recipients = vec![
            "jane@example.com".to_string(),
            "paul@example.com".to_string(),
        ];
        let script = async move {
            server.say("220 smtp.example.com ready\r\n").await;
            server.expect("EHLO [127.0.0.1]").await;
            server
                .say("250-smtp.example.com\r\n250-SIZE 1000\r\n250 auth plain login\r\n")
                .await;
            server
                .expect(&format!(
                    "AUTH PLAIN {}",
                    STANDARD.encode("\0me@example.com\0secret")
                ))
                .await;
            server.say("235 2.7.0 ok\r\n").await;
            server.expect("MAIL FROM:<me@example.com>").await;
            server.say("250 ok\r\n").await;
            server.expect("RCPT TO:<jane@example.com>").await;
            server.say("250 ok\r\n").await;
            server.expect("RCPT TO:<paul@example.com>").await;
            server.say("251 will forward\r\n").await;
            server.expect("DATA").await;
            server.say("354 go ahead\r\n").await;
            server.expect("Subject: x").await;
            server.expect("").await;
            server.expect("..hidden").await;
            server.expect("end").await;
            server.expect(".").await;
            server.say("250 queued\r\n").await;
            server.expect("QUIT").await;
            server.say("221 bye\r\n").await;
        };
        let session = async move {
            client.greeting().await.unwrap();
            let capabilities = client.ehlo("[127.0.0.1]").await.unwrap();
            assert_eq!(capabilities, vec!["SIZE 1000", "AUTH PLAIN LOGIN"]);
            client.authenticate(&login(), &capabilities).await.unwrap();
            client
                .deliver(&Envelope {
                    from: "me@example.com",
                    recipients: &recipients,
                    message: b"Subject: x\r\n\r\n.hidden\r\nend",
                })
                .await
                .unwrap();
            client.quit().await;
        };
        tokio::join!(script, session);
    }

    #[tokio::test]
    async fn falls_back_to_login_and_reports_the_refusal() {
        let (mut client, mut server) = pair();
        let script = async move {
            server.expect("AUTH LOGIN").await;
            server.say("334 VXNlcm5hbWU6\r\n").await;
            server.expect(&STANDARD.encode("me@example.com")).await;
            server.say("334 UGFzc3dvcmQ6\r\n").await;
            server.expect(&STANDARD.encode("secret")).await;
            server
                .say("535-5.7.8 Username and Password not accepted.\r\n535 5.7.8 Learn more\r\n")
                .await;
        };
        let session = async move {
            let capabilities = vec!["AUTH=LOGIN".to_string()];
            client
                .authenticate(&login(), &capabilities)
                .await
                .unwrap_err()
                .to_string()
        };
        let ((), error) = tokio::join!(script, session);
        assert_eq!(
            error,
            "AUTH LOGIN: 535 5.7.8 Username and Password not accepted. 5.7.8 Learn more"
        );
    }

    #[tokio::test]
    async fn a_permanent_refusal_of_the_mail_is_told_apart_from_a_passing_one() {
        let (mut client, mut server) = pair();
        let recipients = vec!["nobody@example.com".to_string()];
        let script = async move {
            server.expect("MAIL FROM:<me@example.com>").await;
            server.say("250 ok\r\n").await;
            server.expect("RCPT TO:<nobody@example.com>").await;
            server.say("550 5.1.1 no such user\r\n").await;
            server.expect("MAIL FROM:<me@example.com>").await;
            server.say("451 4.3.0 try again later\r\n").await;
        };
        let session = async move {
            let envelope = Envelope {
                from: "me@example.com",
                recipients: &recipients,
                message: b"x",
            };
            let refused = client.deliver(&envelope).await.unwrap_err();
            let later = client.deliver(&envelope).await.unwrap_err();
            (refused, later)
        };
        let ((), (refused, later)) = tokio::join!(script, session);
        assert!(refused.is::<Refused>());
        assert_eq!(
            format!("{refused:#}"),
            "nobody@example.com: RCPT TO: 550 5.1.1 no such user"
        );
        assert!(!later.is::<Refused>());
    }

    #[tokio::test]
    async fn refuses_addresses_that_could_inject_commands() {
        let (mut client, _server) = pair();
        let recipients = vec!["jane@example.com>\r\nRCPT TO:<x@example.com".to_string()];
        let error = client
            .deliver(&Envelope {
                from: "me@example.com",
                recipients: &recipients,
                message: b"x",
            })
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("is not a mail address"),
            "{error}"
        );
    }

    #[test]
    fn replies_are_three_digits_and_a_separator() {
        assert_eq!(parse_line("250 ok"), Some((250, false, "ok")));
        assert_eq!(parse_line("250-SIZE 1"), Some((250, true, "SIZE 1")));
        assert_eq!(parse_line("354"), Some((354, false, "")));
        assert_eq!(parse_line("25 ok"), None);
        assert_eq!(parse_line("250:ok"), None);
        assert_eq!(parse_line("abc ok"), None);
        assert_eq!(
            mechanisms(&["AUTH PLAIN XOAUTH2".to_string(), "AUTH=LOGIN".to_string()]),
            vec!["PLAIN", "XOAUTH2", "LOGIN"]
        );
        assert_eq!(ehlo_name("192.0.2.1:5000".parse().unwrap()), "[192.0.2.1]");
        assert_eq!(
            ehlo_name("[2001:db8::1]:5000".parse().unwrap()),
            "[IPv6:2001:db8::1]"
        );
    }

    #[test]
    fn a_leading_dot_is_doubled_and_the_end_marked() {
        assert_eq!(dot_stuffed(b".a\r\nb\r\n.\r\n"), b"..a\r\nb\r\n..\r\n.\r\n");
        assert_eq!(dot_stuffed(b"a"), b"a\r\n.\r\n");
    }
}
