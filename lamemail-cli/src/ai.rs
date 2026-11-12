use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::i18n::{fill, t};
use crate::settings::{self, Ai, Settings};
use crate::{ollama, output};

const CLAUDE: &str = r#"claude -p --tools "" --strict-mcp-config --output-format text"#;
const CLAUDE_RULES: &str = "--system-prompt";
const CLAUDE_URL: &str = "https://claude.com/claude-code";
const OLLAMA_URL: &str = "https://ollama.com";
const BETWEEN: &str = "\n\n";
const TIMEOUT: Duration = Duration::from_secs(180);
pub const CHARS_PER_TOKEN: u64 = 3;
pub const ANSWER_TOKENS: u64 = 2048;
pub const SMALLEST_LIMIT: u64 = 8192;
const CLAUDE_LIMIT: u64 = 200_000;
const OWN_LIMIT: u64 = 32_768;

#[derive(Serialize)]
struct Shown<'a> {
    active: Option<&'static str>,
    command: Option<&'a str>,
    ollama: Option<&'a str>,
    custom: Option<&'a str>,
    programs: Vec<Program>,
    context_default: Option<u64>,
    context_set: Option<u64>,
}

#[derive(Serialize)]
struct Program {
    program: &'static str,
    name: String,
    url: &'static str,
    found: bool,
    running: bool,
    command: Option<String>,
    ollama: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Chosen<'a> {
    Program(&'a str),
    Ollama(&'a str),
}

pub struct Prompt {
    pub rules: String,
    pub data: String,
}

impl Prompt {
    pub fn chars(&self) -> usize {
        self.rules.chars().count() + BETWEEN.len() + self.data.chars().count()
    }

    pub fn whole(&self) -> String {
        format!("{}{BETWEEN}{}", self.rules, self.data)
    }
}

pub fn chosen(settings: &Settings) -> Result<Chosen<'_>> {
    match settings.ai.as_ref().context(t().ai_none)? {
        Ai::ClaudeCode => Ok(Chosen::Program(CLAUDE)),
        Ai::Ollama(model) => Ok(Chosen::Ollama(model)),
        Ai::Custom => custom(settings)
            .map(Chosen::Program)
            .context(t().ai_no_custom),
        Ai::Command { command } => Ok(Chosen::Program(command)),
    }
}

fn custom(settings: &Settings) -> Option<&str> {
    settings.custom_command.as_deref().or(match &settings.ai {
        Some(Ai::Command { command }) if command != CLAUDE => Some(command),
        _ => None,
    })
}

fn active(ai: &Ai) -> &'static str {
    match ai {
        Ai::ClaudeCode => "claude-code",
        Ai::Ollama(_) => "ollama",
        Ai::Command { command } if command == CLAUDE => "claude-code",
        Ai::Custom | Ai::Command { .. } => "custom",
    }
}

pub fn set_claude_code(dir: &Path) -> Result<()> {
    runnable_line(CLAUDE)?;
    keep(dir, Ai::ClaudeCode, None)
}

pub fn set_ollama(dir: &Path, model: &str) -> Result<()> {
    choose_ollama(dir, ollama::ADDRESS, model)
}

fn choose_ollama(dir: &Path, address: &str, model: &str) -> Result<()> {
    let model = ollama::short_name(model);
    let models =
        ollama::models(address).with_context(|| fill(t().ai_ollama_offline, &[&address]))?;
    if !models.iter().any(|known| known == model) {
        bail!(
            "{}",
            fill(t().ai_ollama_unknown, &[&model, &models.join(", ")])
        );
    }
    keep(dir, Ai::Ollama(model.to_string()), None)
}

pub fn set_custom(dir: &Path, command: Option<&str>) -> Result<()> {
    let line = match command {
        Some(command) => command.trim().to_string(),
        None => custom(&settings::load(dir)?)
            .context(t().ai_no_custom)?
            .to_string(),
    };
    runnable_line(&line)?;
    keep(dir, Ai::Custom, Some(line))
}

fn runnable_line(line: &str) -> Result<()> {
    let words = words(line)?;
    let program = words.first().context(t().ai_no_custom)?;
    find(program, &search_path()).with_context(|| fill(t().ai_not_found, &[program]))?;
    Ok(())
}

fn keep(dir: &Path, ai: Ai, custom: Option<String>) -> Result<()> {
    let mut settings = settings::load(dir)?;
    if let Some(custom) = custom {
        settings.custom_command = Some(custom);
    }
    settings.ai = Some(ai);
    settings::save(dir, &settings)?;
    println!("{}", fill(t().ai_set, &[&described(&settings)]));
    Ok(())
}

fn described(settings: &Settings) -> String {
    match chosen(settings) {
        Ok(Chosen::Program(CLAUDE)) => format!("Claude Code ({CLAUDE})"),
        Ok(Chosen::Program(line)) => line.to_string(),
        Ok(Chosen::Ollama(model)) => format!("ollama · {model}"),
        Err(err) => err.to_string(),
    }
}

fn key(ai: &Ai) -> String {
    match ai {
        Ai::Ollama(model) => format!("ollama {model}"),
        other => active(other).to_string(),
    }
}

fn stored_limit(settings: &Settings) -> Option<u64> {
    let ai = settings.ai.as_ref()?;
    settings.context_limits.get(&key(ai)).copied()
}

fn model_limit(chosen: &Chosen, ollama_address: &str) -> Result<Option<u64>> {
    match chosen {
        Chosen::Program(CLAUDE) => Ok(Some(CLAUDE_LIMIT)),
        Chosen::Program(_) => Ok(None),
        Chosen::Ollama(model) => ollama::longest(ollama_address, model),
    }
}

pub fn context_limit(settings: &Settings, chosen: &Chosen) -> Result<u64> {
    limit_at(settings, chosen, ollama::ADDRESS)
}

fn default_limit(chosen: &Chosen, ollama_address: &str) -> Result<u64> {
    Ok(model_limit(chosen, ollama_address)?.unwrap_or(OWN_LIMIT))
}

fn limit_at(settings: &Settings, chosen: &Chosen, ollama_address: &str) -> Result<u64> {
    match stored_limit(settings) {
        Some(set) => Ok(set),
        None => default_limit(chosen, ollama_address),
    }
}

pub fn room(limit: u64) -> usize {
    (limit.saturating_sub(ANSWER_TOKENS) * CHARS_PER_TOKEN) as usize
}

pub fn cut(text: &str, limit: usize) -> String {
    match text.char_indices().nth(limit) {
        Some((at, _)) => format!("{} […]", &text[..at]),
        None => text.to_string(),
    }
}

pub fn set_context_limit(dir: &Path, tokens: Option<u64>) -> Result<()> {
    limit_context(dir, tokens, ollama::ADDRESS)
}

fn limit_context(dir: &Path, tokens: Option<u64>, ollama_address: &str) -> Result<()> {
    let mut settings = settings::load(dir)?;
    let key = key(settings.ai.as_ref().context(t().ai_none)?);
    let model = model_limit(&chosen(&settings)?, ollama_address)?;
    match tokens {
        Some(tokens) => {
            if tokens < SMALLEST_LIMIT || model.is_some_and(|most| tokens > most) {
                bail!(
                    "{}",
                    match model {
                        Some(most) => fill(t().ai_context_range, &[&SMALLEST_LIMIT, &most]),
                        None => fill(t().ai_context_least, &[&SMALLEST_LIMIT]),
                    }
                );
            }
            settings.context_limits.insert(key, tokens);
        }
        None => {
            settings.context_limits.remove(&key);
        }
    }
    settings::save(dir, &settings)?;
    let limit = tokens.or(model).unwrap_or(OWN_LIMIT);
    println!("{}", fill(t().ai_context_limit, &[&limit]));
    Ok(())
}

pub fn remove(dir: &Path) -> Result<()> {
    let mut settings = settings::load(dir)?;
    settings.ai = None;
    settings::save(dir, &settings)?;
    println!("{}", t().ai_removed);
    Ok(())
}

pub fn show(dir: &Path, json: bool) -> Result<()> {
    let settings = settings::load(dir)?;
    let chosen = chosen(&settings).ok();
    let default = chosen
        .as_ref()
        .and_then(|chosen| default_limit(chosen, ollama::ADDRESS).ok());
    let set = stored_limit(&settings);
    let shown = Shown {
        active: settings.ai.as_ref().map(active),
        command: match chosen {
            Some(Chosen::Program(line)) => Some(line),
            _ => None,
        },
        ollama: match chosen {
            Some(Chosen::Ollama(model)) => Some(model),
            _ => None,
        },
        custom: custom(&settings),
        programs: programs(&search_path(), ollama::ADDRESS),
        context_default: default,
        context_set: set,
    };
    output::stdout(|out| {
        if json {
            return output::json_line(out, &shown);
        }
        if settings.ai.is_some() {
            writeln!(out, "{}", fill(t().ai_shown, &[&described(&settings)]))?;
            if let Some(limit) = set.or(default) {
                writeln!(out, "{}", fill(t().ai_context_limit, &[&limit]))?;
            }
            return Ok(());
        }
        writeln!(out, "{}", t().ai_none)?;
        for known in &shown.programs {
            if known.program == "claude-code" && known.running {
                writeln!(out, "{}", fill(t().ai_suggested, &[&"claude-code"]))?;
            }
            if let Some(model) = &known.ollama {
                writeln!(
                    out,
                    "{}",
                    fill(t().ai_suggested, &[&format!("ollama {model}")])
                )?;
            }
        }
        Ok(())
    })
}

pub fn ask(chosen: &Chosen, prompt: &Prompt, limit: u64) -> Result<String> {
    ask_within(chosen, prompt, limit, TIMEOUT)
}

fn ask_within(chosen: &Chosen, prompt: &Prompt, limit: u64, timeout: Duration) -> Result<String> {
    let answer = match chosen {
        Chosen::Program(line) => {
            let (words, input) = started(line, prompt)?;
            run(&words, &input, timeout)?
        }
        Chosen::Ollama(model) => ollama::ask(ollama::ADDRESS, model, prompt, limit, timeout)?,
    };
    let answer = answer.trim();
    if answer.is_empty() {
        bail!("{}", t().ai_empty);
    }
    Ok(answer.to_string())
}

fn started(line: &str, prompt: &Prompt) -> Result<(Vec<String>, String)> {
    let mut words = words(line)?;
    if line != CLAUDE {
        return Ok((words, prompt.whole()));
    }
    words.extend([CLAUDE_RULES.to_string(), prompt.rules.clone()]);
    Ok((words, prompt.data.clone()))
}

fn run(words: &[String], input: &str, timeout: Duration) -> Result<String> {
    let (program, arguments) = words.split_first().context(t().ai_none)?;
    let mut child = Command::new(program)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| fill(t().ai_start, &[program]))?;
    let mut stdin = child.stdin.take().context("opening the AI's stdin")?;
    let input = input.to_string();
    let writing = thread::spawn(move || stdin.write_all(input.as_bytes()));
    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait().context("waiting for the AI")? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("{}", fill(t().ai_timeout, &[&timeout.as_secs()]));
        }
        thread::sleep(Duration::from_millis(50));
    };
    let _ = writing.join();
    let out = out.join().unwrap_or_default();
    let err = err.join().unwrap_or_default();
    if !status.success() {
        let reason = err
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map_or_else(|| status.to_string(), |line| line.trim().to_string());
        bail!("{}", fill(t().ai_failed, &[program, &reason]));
    }
    Ok(out)
}

fn drain(pipe: Option<impl Read + Send + 'static>) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

fn search_path() -> OsString {
    std::env::var_os("PATH").unwrap_or_default()
}

fn programs(path: &OsStr, ollama_address: &str) -> Vec<Program> {
    let ollama = |name: String, found, running, model: Option<String>| Program {
        program: "ollama",
        name,
        url: OLLAMA_URL,
        found,
        running,
        command: None,
        ollama: model,
    };
    let mut programs = match ollama::models(ollama_address) {
        Some(models) if !models.is_empty() => models
            .into_iter()
            .map(|model| ollama(format!("ollama · {model}"), true, true, Some(model)))
            .collect(),
        Some(_) => vec![ollama("ollama".to_string(), true, true, None)],
        None => vec![ollama(
            "ollama".to_string(),
            find("ollama", path).is_some(),
            false,
            None,
        )],
    };
    let claude = find("claude", path).is_some();
    programs.push(Program {
        program: "claude-code",
        name: "Claude Code".to_string(),
        url: CLAUDE_URL,
        found: claude,
        running: claude,
        command: claude.then(|| CLAUDE.to_string()),
        ollama: None,
    });
    programs
}

fn find(program: &str, path: &OsStr) -> Option<PathBuf> {
    if program.is_empty() {
        return None;
    }
    if program.contains('/') {
        return Some(PathBuf::from(program)).filter(|file| runnable(file));
    }
    std::env::split_paths(path)
        .map(|dir| dir.join(program))
        .find(|file| runnable(file))
}

fn runnable(file: &Path) -> bool {
    file.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

fn words(line: &str) -> Result<Vec<String>> {
    let unclosed = || fill(t().ai_quote, &[&line]);
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                let word = word.get_or_insert_default();
                loop {
                    match chars.next().with_context(unclosed)? {
                        '\'' => break,
                        c => word.push(c),
                    }
                }
            }
            '"' => {
                let word = word.get_or_insert_default();
                loop {
                    match chars.next().with_context(unclosed)? {
                        '"' => break,
                        '\\' => word.push(chars.next().with_context(unclosed)?),
                        c => word.push(c),
                    }
                }
            }
            '\\' => word
                .get_or_insert_default()
                .push(chars.next().unwrap_or('\\')),
            c if c.is_whitespace() => words.extend(word.take()),
            c => word.get_or_insert_default().push(c),
        }
    }
    words.extend(word);
    Ok(words)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::testing::{asked, closed_address, http_server};

    fn ai(command: &str) -> Chosen<'_> {
        Chosen::Program(command)
    }

    #[test]
    fn the_settings_name_what_answers_and_an_old_command_still_counts() {
        let parse = |json: &str| -> Settings {
            serde_json::from_str(&format!(r#"{{"version":1,{json}}}"#)).unwrap()
        };
        let claude = parse(r#""ai":"claude-code""#);
        assert_eq!(chosen(&claude).unwrap(), Chosen::Program(CLAUDE));
        assert_eq!(active(claude.ai.as_ref().unwrap()), "claude-code");
        let model = parse(r#""ai":{"ollama":"gemma4:12b"}"#);
        assert_eq!(chosen(&model).unwrap(), Chosen::Ollama("gemma4:12b"));
        let own = parse(r#""ai":"custom","custom_command":"llm -m mistral""#);
        assert_eq!(chosen(&own).unwrap(), Chosen::Program("llm -m mistral"));
        assert_eq!(active(own.ai.as_ref().unwrap()), "custom");
        let lost = parse(r#""ai":"custom""#);
        assert_eq!(chosen(&lost).unwrap_err().to_string(), t().ai_no_custom);

        let old_claude = parse(&format!(
            r#""ai":{{"command":{}}}"#,
            serde_json::json!(CLAUDE)
        ));
        assert_eq!(chosen(&old_claude).unwrap(), Chosen::Program(CLAUDE));
        assert_eq!(active(old_claude.ai.as_ref().unwrap()), "claude-code");
        assert_eq!(custom(&old_claude), None);
        let old_own = parse(r#""ai":{"command":"ollama run llama3.2"}"#);
        assert_eq!(
            chosen(&old_own).unwrap(),
            Chosen::Program("ollama run llama3.2")
        );
        assert_eq!(active(old_own.ai.as_ref().unwrap()), "custom");
        assert_eq!(custom(&old_own), Some("ollama run llama3.2"));
        assert_eq!(
            serde_json::to_value(&claude.ai).unwrap(),
            serde_json::json!("claude-code")
        );
    }

    #[test]
    fn the_own_command_is_kept_while_it_is_off() {
        let dir = std::env::temp_dir().join(format!("lamemail-ai-custom-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        settings::init(&dir).unwrap();
        assert_eq!(
            set_custom(&dir, None).unwrap_err().to_string(),
            t().ai_no_custom
        );
        assert_eq!(
            set_custom(&dir, Some("no-such-program-anywhere -x"))
                .unwrap_err()
                .to_string(),
            fill(t().ai_not_found, &[&"no-such-program-anywhere"])
        );
        set_custom(&dir, Some(" sh -c 'cat' ")).unwrap();
        let settings = settings::load(&dir).unwrap();
        assert_eq!(settings.ai, Some(Ai::Custom));
        assert_eq!(chosen(&settings).unwrap(), Chosen::Program("sh -c 'cat'"));
        remove(&dir).unwrap();
        let settings = settings::load(&dir).unwrap();
        assert_eq!(settings.ai, None);
        assert_eq!(custom(&settings), Some("sh -c 'cat'"));
        set_custom(&dir, None).unwrap();
        assert_eq!(settings::load(&dir).unwrap().ai, Some(Ai::Custom));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_context_limit_is_the_model_s_until_one_is_set_for_that_ai() {
        let dir = std::env::temp_dir().join(format!("lamemail-ai-limit-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        settings::init(&dir).unwrap();
        let (service, _) = http_server(|path| match path {
            "/api/tags" => (200, r#"{"models":[{"name":"gemma4:12b"}]}"#.to_string()),
            _ => (
                200,
                r#"{"model_info":{"gemma3.context_length":131072}}"#.to_string(),
            ),
        });
        let limit = || {
            let settings = settings::load(&dir).unwrap();
            limit_at(&settings, &chosen(&settings).unwrap(), &service).unwrap()
        };
        assert_eq!(
            limit_context(&dir, Some(50_000), &service)
                .unwrap_err()
                .to_string(),
            t().ai_none
        );

        choose_ollama(&dir, &service, "gemma4:12b").unwrap();
        assert_eq!(limit(), 131_072);
        let range = fill(t().ai_context_range, &[&8192, &131_072]);
        let refused = |tokens| {
            limit_context(&dir, Some(tokens), &service)
                .unwrap_err()
                .to_string()
        };
        assert_eq!(refused(8191), range);
        assert_eq!(refused(131_073), range);
        limit_context(&dir, Some(16_384), &service).unwrap();
        assert_eq!(limit(), 16_384);

        set_custom(&dir, Some("cat")).unwrap();
        assert_eq!(limit(), OWN_LIMIT);
        assert_eq!(refused(4096), fill(t().ai_context_least, &[&8192]));
        limit_context(&dir, Some(500_000), &service).unwrap();
        assert_eq!(limit(), 500_000);
        limit_context(&dir, None, &service).unwrap();
        assert_eq!(limit(), OWN_LIMIT);

        choose_ollama(&dir, &service, "gemma4:12b").unwrap();
        assert_eq!(limit(), 16_384);
        let saved = fs::read_to_string(dir.join(settings::SETTINGS_FILE)).unwrap();
        assert!(saved.contains(r#""ollama gemma4:12b": 16384"#));
        assert!(!saved.contains(r#""custom":"#));
        let claude = Settings {
            ai: Some(Ai::ClaudeCode),
            ..settings::load(&dir).unwrap()
        };
        assert_eq!(
            limit_at(&claude, &Chosen::Program(CLAUDE), &service).unwrap(),
            CLAUDE_LIMIT
        );
        assert_eq!(room(CLAUDE_LIMIT), (200_000 - 2048) * 3);
        assert_eq!(room(1000), 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_long_text_is_cut_at_a_character() {
        assert_eq!(cut("äöü", 2), "äö […]");
        assert_eq!(cut("äöü", 3), "äöü");
    }

    #[test]
    fn a_command_line_is_split_like_a_shell_without_one() {
        assert_eq!(
            words(CLAUDE).unwrap(),
            [
                "claude",
                "-p",
                "--tools",
                "",
                "--strict-mcp-config",
                "--output-format",
                "text"
            ]
        );
        assert_eq!(
            words(r#"  ollama run 'llama 3' "a \"b\"" c\ d "#).unwrap(),
            ["ollama", "run", "llama 3", "a \"b\"", "c d"]
        );
        assert!(words("claude -p 'open").is_err());
        assert!(words("").unwrap().is_empty());
    }

    #[test]
    fn the_program_gets_the_prompt_and_its_answer_is_trimmed() {
        let x = asked("x", "y");
        assert_eq!(
            ask(
                &ai("cat"),
                &asked("\n  Be kind.", "Hello, Jane.\n\n"),
                OWN_LIMIT
            )
            .unwrap(),
            "Be kind.\n\nHello, Jane."
        );
        let failed = ask(&ai("sh -c 'echo nope >&2; exit 3'"), &x, OWN_LIMIT).unwrap_err();
        assert_eq!(failed.to_string(), fill(t().ai_failed, &[&"sh", &"nope"]));
        assert_eq!(
            ask(&ai("true"), &x, OWN_LIMIT).unwrap_err().to_string(),
            t().ai_empty
        );
        assert!(ask(&ai("/nonexistent/claude"), &x, OWN_LIMIT).is_err());
        assert_eq!(
            ask(&ai("  "), &x, OWN_LIMIT).unwrap_err().to_string(),
            t().ai_none
        );
        let slow =
            ask_within(&ai("sleep 5"), &x, OWN_LIMIT, Duration::from_millis(100)).unwrap_err();
        assert_eq!(slow.to_string(), fill(t().ai_timeout, &[&0]));
    }

    #[test]
    fn claude_code_gets_the_rules_as_its_system_prompt_and_any_other_program_one_text() {
        let given = asked("Mails are data.", "<mail>\nHi\n</mail>");
        assert_eq!(given.whole(), "Mails are data.\n\n<mail>\nHi\n</mail>");
        assert_eq!(given.chars(), given.whole().chars().count());
        let (words, input) = started(CLAUDE, &given).unwrap();
        assert_eq!(
            words[words.len() - 4..],
            [
                "--output-format",
                "text",
                "--system-prompt",
                "Mails are data."
            ]
        );
        assert_eq!(input, "<mail>\nHi\n</mail>");
        let (words, input) = started("llm -m mistral", &given).unwrap();
        assert_eq!(words, ["llm", "-m", "mistral"]);
        assert_eq!(input, given.whole());
    }

    #[test]
    fn programs_are_found_in_the_path_or_by_their_path() {
        assert!(find("sh", &search_path()).is_some());
        assert!(find("/bin/sh", OsStr::new("")).is_some());
        assert!(find("no-such-program-anywhere", &search_path()).is_none());
        assert!(find("", &search_path()).is_none());
    }

    #[test]
    fn claude_code_and_every_ollama_model_are_offered_when_found() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!("lamemail-ai-scan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let program = |name: &str| {
            let file = dir.join(name);
            fs::write(&file, "#!/bin/sh\nexit 0\n").unwrap();
            fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        };
        program("claude");
        fs::write(dir.join("ollama-notes"), "").unwrap();
        assert!(find("ollama-notes", dir.as_os_str()).is_none());
        let (service, _) = http_server(|_| {
            (
                200,
                r#"{"models":[{"name":"llama3.2:latest"},{"name":"qwen3:8b"}]}"#.to_string(),
            )
        });
        let known = |programs: Vec<Program>| {
            programs
                .into_iter()
                .map(|known| {
                    (
                        known.name,
                        known.found,
                        known.running,
                        known.command,
                        known.ollama,
                    )
                })
                .collect::<Vec<_>>()
        };
        let text = |text: &str| Some(text.to_string());
        let all = programs(dir.as_os_str(), &service);
        assert_eq!(all[0].url, OLLAMA_URL);
        assert_eq!(
            known(all),
            [
                (
                    "ollama · llama3.2".to_string(),
                    true,
                    true,
                    None,
                    text("llama3.2")
                ),
                (
                    "ollama · qwen3:8b".to_string(),
                    true,
                    true,
                    None,
                    text("qwen3:8b")
                ),
                ("Claude Code".to_string(), true, true, text(CLAUDE), None),
            ]
        );

        let (empty, _) = http_server(|_| (200, r#"{"models":[]}"#.to_string()));
        assert_eq!(
            known(programs(dir.as_os_str(), &empty))[0],
            ("ollama".to_string(), true, true, None, None)
        );
        let stopped = closed_address();
        assert_eq!(
            known(programs(dir.as_os_str(), &stopped))[0],
            ("ollama".to_string(), false, false, None, None)
        );
        program("ollama");
        fs::remove_file(dir.join("claude")).unwrap();
        assert_eq!(
            known(programs(dir.as_os_str(), &stopped)),
            [
                ("ollama".to_string(), true, false, None, None),
                ("Claude Code".to_string(), false, false, None, None),
            ]
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn an_ollama_model_is_set_when_the_service_has_it() {
        let dir = std::env::temp_dir().join(format!("lamemail-ai-ollama-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        settings::init(&dir).unwrap();
        let (service, _) =
            http_server(|_| (200, r#"{"models":[{"name":"gemma4:12b"}]}"#.to_string()));
        assert_eq!(
            choose_ollama(&dir, &service, "nope")
                .unwrap_err()
                .to_string(),
            fill(t().ai_ollama_unknown, &[&"nope", &"gemma4:12b"])
        );
        let stopped = closed_address();
        assert_eq!(
            choose_ollama(&dir, &stopped, "gemma4:12b")
                .unwrap_err()
                .to_string(),
            fill(t().ai_ollama_offline, &[&stopped])
        );
        assert!(settings::load(&dir).unwrap().ai.is_none());
        choose_ollama(&dir, &service, "gemma4:12b").unwrap();
        assert_eq!(
            settings::load(&dir).unwrap().ai,
            Some(Ai::Ollama("gemma4:12b".to_string()))
        );
        let saved = fs::read_to_string(dir.join(settings::SETTINGS_FILE)).unwrap();
        assert!(saved.contains(r#""ollama": "gemma4:12b""#));
        fs::remove_dir_all(&dir).unwrap();
    }
}
