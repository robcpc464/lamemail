use std::io::ErrorKind;
use std::process::{Command, Output, Stdio};

use anyhow::{Context, Result, anyhow, bail};

use crate::i18n::{fill, t};

const TOOL: &str = "secret-tool";

fn attributes(id: &str) -> [&str; 4] {
    ["application", "lamemail", "account", id]
}

fn run(action: &str, id: &str) -> Result<Output> {
    Command::new(TOOL)
        .arg(action)
        .args(attributes(id))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(missing)
}

fn missing(err: std::io::Error) -> anyhow::Error {
    if err.kind() == ErrorKind::NotFound {
        anyhow!("{}", t().secret_tool_missing)
    } else {
        anyhow::Error::new(err).context(format!("running {TOOL}"))
    }
}

fn failed(output: &Output) -> anyhow::Error {
    anyhow!(
        "{}: {}",
        fill(t().secret_tool_failed, &[&output.status]),
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

pub fn store(id: &str) -> Result<()> {
    let status = Command::new(TOOL)
        .args(["store", "--label", &format!("lamemail {id}")])
        .args(attributes(id))
        .status()
        .map_err(missing)?;
    if !status.success() {
        bail!("{}", fill(t().secret_tool_failed, &[&status]));
    }
    Ok(())
}

pub fn lookup(id: &str) -> Result<Option<String>> {
    let output = run("lookup", id)?;
    if output.status.success() {
        return Ok(Some(
            String::from_utf8(output.stdout).context("password is not valid UTF-8")?,
        ));
    }
    if output.stderr.is_empty() {
        return Ok(None);
    }
    Err(failed(&output))
}

pub fn clear(id: &str) -> Result<()> {
    let output = run("clear", id)?;
    if output.status.success() || output.stderr.is_empty() {
        return Ok(());
    }
    Err(failed(&output))
}
