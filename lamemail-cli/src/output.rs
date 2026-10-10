use std::io::{self, BufWriter, ErrorKind, Write};

use anyhow::{Context, Result};
use serde::Serialize;

pub fn stdout(write: impl FnOnce(&mut dyn Write) -> io::Result<()>) -> Result<()> {
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    match write(&mut out).and_then(|()| out.flush()) {
        Err(err) if err.kind() == ErrorKind::BrokenPipe => Ok(()),
        other => other.context("writing stdout"),
    }
}

pub fn json_line<T: Serialize>(out: &mut dyn Write, value: &T) -> io::Result<()> {
    let text = serde_json::to_string(value).map_err(io::Error::other)?;
    writeln!(out, "{text}")
}
