use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use anyhow::{Context, Result, bail};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const LINE_BYTES: u64 = 8 * 1024;
const HEADERS: usize = 100;
const BODY_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, PartialEq)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

pub fn get(address: &str, path: &str, timeout: Duration) -> Result<Response> {
    exchange(address, "GET", path, None, timeout)
}

pub fn post_json(address: &str, path: &str, body: &[u8], timeout: Duration) -> Result<Response> {
    exchange(address, "POST", path, Some(body), timeout)
}

fn exchange(
    address: &str,
    method: &str,
    path: &str,
    body: Option<&[u8]>,
    timeout: Duration,
) -> Result<Response> {
    let target: SocketAddr = address
        .parse()
        .with_context(|| format!("parsing the address {address}"))?;
    let mut stream = TcpStream::connect_timeout(&target, CONNECT_TIMEOUT)
        .with_context(|| format!("connecting to {address}"))?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    send(&mut stream, address, method, path, body)
        .with_context(|| format!("sending {method} {path}"))?;
    receive(BufReader::new(stream))
        .with_context(|| format!("reading the answer to {method} {path}"))
}

fn send(
    out: &mut impl Write,
    host: &str,
    method: &str,
    path: &str,
    body: Option<&[u8]>,
) -> io::Result<()> {
    let mut head = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nAccept: application/json\r\n"
    );
    if let Some(body) = body {
        head += &format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        );
    }
    head += "\r\n";
    out.write_all(head.as_bytes())?;
    out.write_all(body.unwrap_or_default())?;
    out.flush()
}

fn receive(mut reader: impl BufRead) -> Result<Response> {
    let status_line = line(&mut reader)?;
    let status = status_line
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .with_context(|| format!("not an HTTP status line: {status_line}"))?;
    let mut length = None;
    let mut chunked = false;
    for _ in 0..HEADERS {
        let header = line(&mut reader)?;
        if header.is_empty() {
            let body = if chunked {
                dechunk(reader)?
            } else {
                read_body(reader, length)?
            };
            return Ok(Response { status, body });
        }
        let Some((name, value)) = header.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            length = Some(value.parse::<u64>().context("reading Content-Length")?);
        } else if name.eq_ignore_ascii_case("transfer-encoding") {
            chunked = value.eq_ignore_ascii_case("chunked");
        }
    }
    bail!("more than {HEADERS} header lines")
}

fn line(reader: &mut impl BufRead) -> Result<String> {
    let mut line = String::new();
    reader.by_ref().take(LINE_BYTES).read_line(&mut line)?;
    match line.strip_suffix('\n') {
        Some(line) => Ok(line.strip_suffix('\r').unwrap_or(line).to_string()),
        None => bail!("the answer ended early or has a line longer than {LINE_BYTES} bytes"),
    }
}

fn read_body(reader: impl Read, length: Option<u64>) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    reader
        .take(length.unwrap_or(u64::MAX).min(BODY_BYTES + 1))
        .read_to_end(&mut body)?;
    let read = body.len() as u64;
    if read > BODY_BYTES {
        bail!("the answer is larger than {BODY_BYTES} bytes");
    }
    if let Some(length) = length
        && read < length
    {
        bail!("the answer ended after {read} of {length} bytes");
    }
    Ok(body)
}

fn dechunk(mut reader: impl BufRead) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    loop {
        let size_line = line(&mut reader)?;
        let size = size_line.split(';').next().unwrap_or_default().trim();
        let size = u64::from_str_radix(size, 16)
            .with_context(|| format!("not a chunk size: {size_line}"))?;
        if size == 0 {
            return Ok(body);
        }
        if body.len() as u64 + size > BODY_BYTES {
            bail!("the answer is larger than {BODY_BYTES} bytes");
        }
        let start = body.len();
        body.resize(start + size as usize, 0);
        reader.read_exact(&mut body[start..])?;
        if !line(&mut reader)?.is_empty() {
            bail!("a chunk is longer than its size");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn received(raw: &str) -> Result<Response> {
        receive(raw.as_bytes())
    }

    #[test]
    fn a_request_names_its_host_closes_and_carries_its_body() {
        let mut out = Vec::new();
        send(
            &mut out,
            "127.0.0.1:11434",
            "POST",
            "/api/chat",
            Some(b"{}"),
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "POST /api/chat HTTP/1.1\r\nHost: 127.0.0.1:11434\r\nConnection: close\r\n\
             Accept: application/json\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}"
        );
    }

    #[test]
    fn an_answer_is_read_by_its_length_by_chunks_or_to_its_end() {
        let ok = |body: &str| Response {
            status: 200,
            body: body.as_bytes().to_vec(),
        };
        assert_eq!(
            received("HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello and more").unwrap(),
            ok("hello")
        );
        assert_eq!(
            received(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5;x=1\r\nhello\r\n6\r\n world\r\n0\r\n\r\n"
            )
            .unwrap(),
            ok("hello world")
        );
        assert_eq!(
            received("HTTP/1.0 404 Not Found\nX: y\n\n{\"error\":\"no\"}").unwrap(),
            Response {
                status: 404,
                body: b"{\"error\":\"no\"}".to_vec()
            }
        );
    }

    #[test]
    fn a_broken_answer_is_an_error() {
        assert!(received("").is_err());
        assert!(received("hello\r\n\r\n").is_err());
        assert!(received("HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nshort").is_err());
        assert!(received("HTTP/1.1 200 OK\r\nContent-Length: x\r\n\r\n").is_err());
        assert!(received("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n").is_err());
        assert!(
            received(
                "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nhello\r\n0\r\n\r\n"
            )
            .is_err()
        );
        assert!(
            received(&format!(
                "HTTP/1.1 200 OK\r\nX: {}\r\n\r\n",
                "a".repeat(9000)
            ))
            .is_err()
        );
        assert!(
            received(&format!(
                "HTTP/1.1 200 OK\r\n{}\r\n",
                "X: y\r\n".repeat(101)
            ))
            .is_err()
        );
    }
}
