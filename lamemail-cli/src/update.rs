use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::http;
use crate::i18n::{fill, t};
use crate::output;

const API_HOST: &str = "api.github.com";
const GITHUB: &str = "https://github.com/";
const TIMEOUT: Duration = Duration::from_secs(10);
const DEV_RELEASE: &str = ".releases/latest.json";
const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
    body: Option<String>,
}

#[derive(Serialize)]
struct Checked {
    current: &'static str,
    latest: String,
    newer: bool,
    url: String,
    notes: String,
}

pub fn check(dev: Option<&Path>, json: bool) -> Result<()> {
    let release = latest(dev)?;
    let latest = release.tag_name.trim_start_matches('v').to_string();
    let checked = Checked {
        current: CURRENT,
        newer: is_newer(&latest, CURRENT),
        latest,
        url: release.html_url,
        notes: release.body.unwrap_or_default(),
    };
    output::stdout(|out| {
        if json {
            return output::json_line(out, &checked);
        }
        let line = if checked.newer {
            fill(
                t().update_available,
                &[&checked.latest, &checked.current, &checked.url],
            )
        } else {
            fill(t().update_newest, &[&checked.current])
        };
        writeln!(out, "{line}")
    })
}

fn latest(dev: Option<&Path>) -> Result<Release> {
    let body = match dev {
        Some(dev) => {
            let path = dev.join(DEV_RELEASE);
            std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?
        }
        None => {
            let path = format!(
                "/repos/{}/releases/latest",
                env!("CARGO_PKG_REPOSITORY").trim_start_matches(GITHUB)
            );
            let response = http::get_secure(API_HOST, &path, TIMEOUT)?;
            if response.status != 200 {
                bail!("GitHub answered {} to {path}", response.status);
            }
            response.body
        }
    };
    serde_json::from_slice(&body).context("reading the latest release")
}

fn is_newer(latest: &str, current: &str) -> bool {
    numbers(latest) > numbers(current)
}

fn numbers(version: &str) -> Vec<u64> {
    version
        .split('.')
        .map(|part| {
            part.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::Fixture;

    #[test]
    fn versions_compare_by_their_numbers() {
        assert!(is_newer("0.10.0", "0.9.3"));
        assert!(is_newer("0.4.1", "0.4.0"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.4.0", "0.4.0"));
        assert!(!is_newer("0.3.9", "0.4.0"));
        assert!(!is_newer("0.4.0-rc1", "0.4.0"));
    }

    #[test]
    fn the_development_server_holds_the_latest_release() {
        let fixture = Fixture::new("update");
        assert!(latest(Some(&fixture.dev)).is_err());
        let file = fixture.dev.join(DEV_RELEASE);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(
            &file,
            r#"{"tag_name":"v9.1.0","html_url":"https://github.com/x/y/releases/tag/v9.1.0","body":"- New"}"#,
        )
        .unwrap();
        let release = latest(Some(&fixture.dev)).unwrap();
        assert_eq!(release.tag_name, "v9.1.0");
        assert_eq!(release.body.as_deref(), Some("- New"));
    }
}
