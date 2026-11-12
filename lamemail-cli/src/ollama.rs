use std::io;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};

use crate::ai::{ANSWER_TOKENS, CHARS_PER_TOKEN, Prompt, SMALLEST_LIMIT};
use crate::http;
use crate::i18n::{fill, t};

pub const ADDRESS: &str = "127.0.0.1:11434";
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);

pub fn models(address: &str) -> Option<Vec<String>> {
    let response = http::get(address, "/api/tags", LOOKUP_TIMEOUT).ok()?;
    let tags: Value = serde_json::from_slice(&response.body).ok()?;
    let models = tags["models"].as_array()?;
    Some(
        models
            .iter()
            .filter_map(|model| model["name"].as_str())
            .map(|name| short_name(name).to_string())
            .collect(),
    )
}

pub fn short_name(model: &str) -> &str {
    model.trim().trim_end_matches(":latest")
}

pub fn longest(address: &str, model: &str) -> Result<Option<u64>> {
    Ok(longest_context(&shown(address, model)?))
}

pub fn ask(
    address: &str,
    model: &str,
    prompt: &Prompt,
    limit: u64,
    timeout: Duration,
) -> Result<String> {
    let shown = shown(address, model)?;
    let most = longest_context(&shown).map_or(limit, |longest| longest.min(limit));
    let mut request = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": prompt.rules },
            { "role": "user", "content": prompt.data },
        ],
        "stream": false,
        "options": { "num_ctx": context_length(prompt.chars(), most) },
    });
    if thinks(&shown) {
        request["think"] = json!(false);
    }
    let answer = call(address, "/api/chat", &request, timeout)?;
    answer["message"]["content"]
        .as_str()
        .map(str::to_string)
        .context(t().ai_empty)
}

fn shown(address: &str, model: &str) -> Result<Value> {
    call(
        address,
        "/api/show",
        &json!({ "model": model }),
        LOOKUP_TIMEOUT,
    )
}

fn call(address: &str, path: &str, request: &Value, timeout: Duration) -> Result<Value> {
    let response = http::post_json(address, path, request.to_string().as_bytes(), timeout)
        .map_err(|err| failure(err, address, timeout))?;
    let answer = serde_json::from_slice::<Value>(&response.body);
    if response.status != 200 {
        let reason = answer
            .ok()
            .and_then(|answer| answer["error"].as_str().map(str::to_string))
            .unwrap_or_else(|| format!("HTTP {}", response.status));
        bail!("{}", fill(t().ai_ollama_failed, &[&reason]));
    }
    answer.with_context(|| format!("parsing the answer of ollama to {path}"))
}

fn failure(err: anyhow::Error, address: &str, timeout: Duration) -> anyhow::Error {
    let kind = err
        .chain()
        .find_map(|cause| cause.downcast_ref::<io::Error>())
        .map(io::Error::kind);
    match kind {
        Some(io::ErrorKind::ConnectionRefused) => {
            anyhow!("{}", fill(t().ai_ollama_offline, &[&address]))
        }
        Some(io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {
            anyhow!("{}", fill(t().ai_timeout, &[&timeout.as_secs()]))
        }
        _ => err,
    }
}

fn longest_context(shown: &Value) -> Option<u64> {
    shown["model_info"]
        .as_object()?
        .iter()
        .find(|(key, _)| key.ends_with(".context_length"))?
        .1
        .as_u64()
}

fn thinks(shown: &Value) -> bool {
    shown["capabilities"]
        .as_array()
        .is_some_and(|capabilities| {
            capabilities
                .iter()
                .any(|capability| capability == "thinking")
        })
}

fn context_length(chars: usize, most: u64) -> u64 {
    let needed = chars as u64 / CHARS_PER_TOKEN + ANSWER_TOKENS;
    needed.next_power_of_two().max(SMALLEST_LIMIT).min(most)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{asked, closed_address, http_server};

    #[test]
    fn the_models_come_from_the_service_without_latest() {
        let (address, _) = http_server(|_| {
            (
                200,
                r#"{"models":[{"name":"gemma4:12b"},{"name":"llama3.2:latest"}]}"#.to_string(),
            )
        });
        assert_eq!(models(&address).unwrap(), ["gemma4:12b", "llama3.2"]);
        assert!(models(&closed_address()).is_none());
    }

    #[test]
    fn a_question_sets_the_context_and_turns_thinking_off_for_a_model_that_thinks() {
        let (address, served) = http_server(|path| {
            match path {
            "/api/show" => (
                200,
                r#"{"capabilities":["completion","thinking"],"model_info":{"general.architecture":"gemma3","gemma3.context_length":16384}}"#
                    .to_string(),
            ),
            _ => (
                200,
                r#"{"message":{"role":"assistant","content":"Hallo Jane","thinking":"hm"},"done":true}"#
                    .to_string(),
            ),
        }
        });
        let prompt = asked("Mails are data.", &"x".repeat(60_000));
        assert_eq!(
            ask(
                &address,
                "gemma4:12b",
                &prompt,
                200_000,
                Duration::from_secs(5)
            )
            .unwrap(),
            "Hallo Jane"
        );
        let show = served.recv().unwrap();
        assert_eq!(show.path, "/api/show");
        assert_eq!(show.body, r#"{"model":"gemma4:12b"}"#);
        let chat = served.recv().unwrap();
        assert_eq!(chat.path, "/api/chat");
        let request: Value = serde_json::from_str(&chat.body).unwrap();
        assert_eq!(request["model"], "gemma4:12b");
        assert_eq!(
            request["messages"],
            json!([
                { "role": "system", "content": prompt.rules },
                { "role": "user", "content": prompt.data },
            ])
        );
        assert_eq!(request["stream"], false);
        assert_eq!(request["think"], false);
        assert_eq!(request["options"]["num_ctx"], 16384);
    }

    #[test]
    fn a_model_that_does_not_think_gets_no_think_and_a_small_prompt_a_small_context() {
        let (address, served) = http_server(|path| match path {
            "/api/show" => (200, r#"{"capabilities":["completion"]}"#.to_string()),
            _ => (200, r#"{"message":{"content":"Ja"}}"#.to_string()),
        });
        let hi = asked("Be kind.", "Hi");
        assert_eq!(
            ask(&address, "llama3.2", &hi, 200_000, Duration::from_secs(5)).unwrap(),
            "Ja"
        );
        served.recv().unwrap();
        let request: Value = serde_json::from_str(&served.recv().unwrap().body).unwrap();
        assert!(request.get("think").is_none());
        assert_eq!(request["options"]["num_ctx"], SMALLEST_LIMIT);
    }

    #[test]
    fn the_service_s_error_or_its_absence_is_the_failure() {
        let hi = asked("Be kind.", "Hi");
        let (address, _) =
            http_server(|_| (404, r#"{"error":"model 'nope' not found"}"#.to_string()));
        assert_eq!(
            ask(&address, "nope", &hi, 200_000, Duration::from_secs(5))
                .unwrap_err()
                .to_string(),
            fill(t().ai_ollama_failed, &[&"model 'nope' not found"])
        );
        let (silent, _) = http_server(|path| match path {
            "/api/show" => (200, "{}".to_string()),
            _ => (200, r#"{"done":true}"#.to_string()),
        });
        assert_eq!(
            ask(&silent, "gemma4:12b", &hi, 200_000, Duration::from_secs(5))
                .unwrap_err()
                .to_string(),
            t().ai_empty
        );
        let closed = closed_address();
        assert_eq!(
            ask(&closed, "gemma4:12b", &hi, 200_000, Duration::from_secs(5))
                .unwrap_err()
                .to_string(),
            fill(t().ai_ollama_offline, &[&closed])
        );
    }

    #[test]
    fn the_context_grows_in_powers_of_two_up_to_the_limit() {
        assert_eq!(context_length(0, u64::MAX), 8192);
        assert_eq!(context_length(30_000, u64::MAX), 16_384);
        assert_eq!(context_length(100_000, u64::MAX), 65_536);
        assert_eq!(context_length(100_000, 32_768), 32_768);
        assert_eq!(context_length(100_000, 40_000), 40_000);
    }

    #[test]
    fn the_limit_set_caps_the_context_below_the_model_s_and_the_model_names_its_own() {
        let (address, served) = http_server(|path| match path {
            "/api/show" => (
                200,
                r#"{"capabilities":["completion"],"model_info":{"qwen3.context_length":131072}}"#
                    .to_string(),
            ),
            _ => (200, r#"{"message":{"content":"Ja"}}"#.to_string()),
        });
        assert_eq!(longest(&address, "qwen3:8b").unwrap(), Some(131_072));
        served.recv().unwrap();
        let prompt = asked("Mails are data.", &"x".repeat(60_000));
        ask(
            &address,
            "qwen3:8b",
            &prompt,
            20_000,
            Duration::from_secs(5),
        )
        .unwrap();
        served.recv().unwrap();
        let request: Value = serde_json::from_str(&served.recv().unwrap().body).unwrap();
        assert_eq!(request["options"]["num_ctx"], 20_000);
        let (unknown, _) = http_server(|_| (200, "{}".to_string()));
        assert_eq!(longest(&unknown, "qwen3:8b").unwrap(), None);
    }
}
