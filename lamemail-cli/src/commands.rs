use anyhow::Result;
use clap::{Arg, ArgAction, Command};
use serde::Serialize;

use crate::output;

const PROGRAM: &str = "lamemail";

#[derive(Serialize)]
struct Described {
    command: String,
    usage: String,
    about: String,
    arguments: Vec<Argument>,
}

#[derive(Serialize)]
struct Argument {
    name: String,
    help: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    values: Vec<String>,
}

pub fn run(program: &Command, json: bool) -> Result<()> {
    output::stdout(|out| {
        describe(program).iter().try_for_each(|command| {
            if json {
                output::json_line(out, command)
            } else {
                writeln!(out, "{}\n    {}", command.usage, command.about)
            }
        })
    })
}

fn describe(program: &Command) -> Vec<Described> {
    let mut described = vec![Described::of(String::new(), program)];
    collect(program, "", &mut described);
    described
}

fn collect(command: &Command, parent: &str, described: &mut Vec<Described>) {
    for sub in command.get_subcommands().filter(|sub| !sub.is_hide_set()) {
        let name = format!("{parent} {}", sub.get_name())
            .trim_start()
            .to_string();
        if sub.has_subcommands() {
            collect(sub, &name, described);
        } else {
            described.push(Described::of(name, sub));
        }
    }
}

impl Described {
    fn of(name: String, command: &Command) -> Described {
        let arguments: Vec<&Arg> = command
            .get_arguments()
            .filter(|arg| !arg.is_hide_set())
            .collect();
        let mut usage: Vec<String> =
            std::iter::once(format!("{PROGRAM} {name}").trim_end().to_string())
                .chain(arguments.iter().map(|arg| synopsis(arg)))
                .collect();
        if command.has_subcommands() {
            usage.push("<command>".to_string());
        }
        Described {
            command: name,
            usage: usage.join(" "),
            about: command
                .get_about()
                .map(ToString::to_string)
                .unwrap_or_default(),
            arguments: arguments
                .into_iter()
                .map(|arg| Argument {
                    name: form(arg),
                    help: arg.get_help().map(ToString::to_string).unwrap_or_default(),
                    values: values(arg),
                })
                .collect(),
        }
    }
}

fn values(arg: &Arg) -> Vec<String> {
    if !arg.get_action().takes_values() {
        return Vec::new();
    }
    arg.get_possible_values()
        .iter()
        .filter(|value| !value.is_hide_set())
        .map(|value| value.get_name().to_string())
        .collect()
}

fn synopsis(arg: &Arg) -> String {
    if arg.is_required_set() {
        form(arg)
    } else {
        format!("[{}]", form(arg))
    }
}

fn form(arg: &Arg) -> String {
    let value = arg
        .get_value_names()
        .map_or_else(|| arg.get_id().to_string(), |names| names.join(" "))
        .to_lowercase();
    let form = match arg.get_long() {
        Some(long) if arg.get_action().takes_values() => format!("--{long} <{value}>"),
        Some(long) => format!("--{long}"),
        None => format!("<{value}>"),
    };
    if matches!(arg.get_action(), ArgAction::Append) {
        format!("{form}...")
    } else {
        form
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;
    use crate::Cli;

    fn find<'a>(described: &'a [Described], command: &str) -> &'a Described {
        described
            .iter()
            .find(|described| described.command == command)
            .unwrap_or_else(|| panic!("no {command}"))
    }

    #[test]
    fn every_command_comes_with_its_usage_and_arguments() {
        let described = describe(&Cli::command());
        let program = &described[0];
        assert_eq!(program.command, "");
        assert_eq!(program.usage, "lamemail [--dir <dir>] <command>");
        assert_eq!(program.arguments.len(), 1);

        let send = find(&described, "send");
        assert!(
            send.usage
                .starts_with("lamemail send [<id>] [--reply <path>] [--forward <path>] [--all] [--from <address>]"),
            "{}",
            send.usage
        );
        assert!(send.usage.contains(" [--attachment <path:index>...] "));
        let argument = |name: &str| send.arguments.iter().find(|arg| arg.name == name);
        assert!(
            argument("--attachment <path:index>...")
                .unwrap()
                .help
                .contains("show --json")
        );
        assert!(argument("--json").unwrap().values.is_empty());
        assert!(send.about.starts_with("Write a mail"));

        let folder = find(&described, "account folder");
        assert_eq!(
            folder.arguments[2].values,
            [
                "inbox", "sent", "drafts", "junk", "trash", "archive", "other"
            ]
        );

        let flag = find(&described, "flag");
        assert_eq!(flag.usage, "lamemail flag <path> <change>");
        assert_eq!(
            flag.arguments[1].values,
            [
                "seen",
                "unseen",
                "flagged",
                "unflagged",
                "answered",
                "forwarded",
                "junk",
                "notjunk"
            ]
        );
        assert_eq!(
            find(&described, "attachment").usage,
            "lamemail attachment <path> [<index>] [--to <dir>] [--json]"
        );
        assert_eq!(
            find(&described, "sync").usage,
            "lamemail sync [<id>...] [--json]"
        );
        assert!(
            find(&described, "account add")
                .usage
                .starts_with("lamemail account add <id> --host <host>")
        );

        let names: Vec<&str> = described.iter().map(|d| d.command.as_str()).collect();
        assert!(names.contains(&"folder list") && !names.contains(&"folders"));
        assert!(!names.contains(&"account") && !names.contains(&"help"));
        assert!(
            described
                .iter()
                .all(|d| d.arguments.iter().all(|arg| arg.name != "--help"))
        );
    }
}
