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
const CLAUDE_URL: &str = "https://claude.com/claude-code";
const OLLAMA_URL: &str = "https://ollama.com";
const TIMEOUT: Duration = Duration::from_secs(180);
const CONTEXT_LIMITS: [usize; 4] = [10_000, 20_000, 50_000, 100_000];
const CONTEXT_LIMIT: usize = 20_000;

#[derive(Serialize)]
struct Shown<'a> {
    active: Option<&'static str>,
    command: Option<&'a str>,
    ollama: Option<&'a str>,
    custom: Option<&'a str>,
    programs: Vec<Program>,
    context_limit: usize,
    context_limits: [usize; 4],
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

pub fn set_context_limit(dir: &Path, chars: usize) -> Result<()> {
    if !CONTEXT_LIMITS.contains(&chars) {
        let limits = CONTEXT_LIMITS.map(|limit| limit.to_string()).join(", ");
        bail!("{}", fill(t().ai_context_limits, &[&limits]));
    }
    let mut settings = settings::load(dir)?;
    settings.context_limit = Some(chars);
    settings::save(dir, &settings)?;
    println!("{}", fill(t().ai_context_limit, &[&chars]));
    Ok(())
}

pub fn context_limit(settings: &Settings) -> usize {
    settings.context_limit.unwrap_or(CONTEXT_LIMIT)
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
        context_limit: context_limit(&settings),
        context_limits: CONTEXT_LIMITS,
    };
    output::stdout(|out| {
        if json {
            return output::json_line(out, &shown);
        }
        writeln!(
            out,
            "{}",
            fill(t().ai_context_limit, &[&shown.context_limit])
        )?;
        if settings.ai.is_some() {
            return writeln!(out, "{}", fill(t().ai_shown, &[&described(&settings)]));
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

pub fn ask(chosen: &Chosen, prompt: &str) -> Result<String> {
    ask_within(chosen, prompt, TIMEOUT)
}

fn ask_within(chosen: &Chosen, prompt: &str, timeout: Duration) -> Result<String> {
    let answer = match chosen {
        Chosen::Program(line) => run(&words(line)?, prompt, timeout)?,
        Chosen::Ollama(model) => ollama::ask(ollama::ADDRESS, model, prompt, timeout)?,
    };
    let answer = answer.trim();
    if answer.is_empty() {
        bail!("{}", t().ai_empty);
    }
    Ok(answer.to_string())
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
    let claude = find("claude", path).is_some();
    let mut programs = vec![Program {
        program: "claude-code",
        name: "Claude Code".to_string(),
        url: CLAUDE_URL,
        found: claude,
        running: claude,
        command: claude.then(|| CLAUDE.to_string()),
        ollama: None,
    }];
    match ollama::models(ollama_address) {
        Some(models) if !models.is_empty() => programs.extend(
            models
                .into_iter()
                .map(|model| ollama(format!("ollama · {model}"), true, true, Some(model))),
        ),
        Some(_) => programs.push(ollama("ollama".to_string(), true, true, None)),
        None => programs.push(ollama(
            "ollama".to_string(),
            find("ollama", path).is_some(),
            false,
            None,
        )),
    }
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
    use crate::testing::{closed_address, http_server};

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
    fn the_context_limit_is_one_of_the_steps_and_twenty_thousand_until_set() {
        let dir = std::env::temp_dir().join(format!("lamemail-ai-limit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        settings::init(&dir).unwrap();
        assert_eq!(context_limit(&settings::load(&dir).unwrap()), 20_000);
        let refused = set_context_limit(&dir, 30_000).unwrap_err().to_string();
        assert_eq!(
            refused,
            fill(t().ai_context_limits, &[&"10000, 20000, 50000, 100000"])
        );
        set_context_limit(&dir, 50_000).unwrap();
        let settings = settings::load(&dir).unwrap();
        assert_eq!(context_limit(&settings), 50_000);
        assert!(settings.ai.is_none());
        std::fs::remove_dir_all(&dir).unwrap();
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
        assert_eq!(
            ask(&ai("cat"), "\n  Hello, Jane.\n\n").unwrap(),
            "Hello, Jane."
        );
        let failed = ask(&ai("sh -c 'echo nope >&2; exit 3'"), "x").unwrap_err();
        assert_eq!(failed.to_string(), fill(t().ai_failed, &[&"sh", &"nope"]));
        assert_eq!(ask(&ai("true"), "x").unwrap_err().to_string(), t().ai_empty);
        assert!(ask(&ai("/nonexistent/claude"), "x").is_err());
        assert_eq!(ask(&ai("  "), "x").unwrap_err().to_string(), t().ai_none);
        let slow = ask_within(&ai("sleep 5"), "x", Duration::from_millis(100)).unwrap_err();
        assert_eq!(slow.to_string(), fill(t().ai_timeout, &[&0]));
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
        assert_eq!(all[1].url, OLLAMA_URL);
        assert_eq!(
            known(all),
            [
                ("Claude Code".to_string(), true, true, text(CLAUDE), None),
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
            ]
        );

        let (empty, _) = http_server(|_| (200, r#"{"models":[]}"#.to_string()));
        assert_eq!(
            known(programs(dir.as_os_str(), &empty))[1],
            ("ollama".to_string(), true, true, None, None)
        );
        let stopped = closed_address();
        assert_eq!(
            known(programs(dir.as_os_str(), &stopped))[1],
            ("ollama".to_string(), false, false, None, None)
        );
        program("ollama");
        fs::remove_file(dir.join("claude")).unwrap();
        assert_eq!(
            known(programs(dir.as_os_str(), &stopped)),
            [
                ("Claude Code".to_string(), false, false, None, None),
                ("ollama".to_string(), true, false, None, None),
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
