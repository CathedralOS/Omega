//! The command-line boundary owns arguments, presentation and process status.

#![forbid(unsafe_code)]

mod source;

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "Usage: omega source parse [--] <file>\n       omega source resolve [--] <package-relative file>...\n       omega source type [--] <package-relative file>...";
const HELP: &str = "Parse checks syntax. Resolve checks names. Type checks declarations, expressions and machine calls. For resolve and type, list the source files of one package rooted at the current directory. Success produces no output.";

enum Command<'a> {
    Help,
    Parse(&'a Path),
    Stage(&'a [OsString], source::Stage),
}

fn command(arguments: &[OsString]) -> Result<Command<'_>, String> {
    let is_help = |argument: &OsString| argument == "--help" || argument == "-h";
    match arguments {
        [flag] if is_help(flag) => Ok(Command::Help),
        [group, flag] if group == "source" && is_help(flag) => Ok(Command::Help),
        [group, action, rest @ ..] if group == "source" && action == "parse" => match rest {
            [flag] if is_help(flag) => Ok(Command::Help),
            [marker, file] if marker == "--" => Ok(Command::Parse(Path::new(file))),
            [file] if !file.as_encoded_bytes().starts_with(b"-") => Ok(Command::Parse(Path::new(file))),
            [flag] if flag != "--" => Err(format!("unknown option '{}'", flag.to_string_lossy())),
            _ => Err("source parse requires exactly one file".into()),
        },
        [group, action, rest @ ..] if group == "source" && (action == "resolve" || action == "type") => {
            if let [flag] = rest
                && is_help(flag)
            {
                return Ok(Command::Help);
            }
            let files = if rest.first().is_some_and(|flag| flag == "--") {
                &rest[1..]
            } else {
                if let Some(flag) = rest.iter().find(|p| p.as_encoded_bytes().starts_with(b"-")) {
                    return Err(format!("unknown option '{}'", flag.to_string_lossy()));
                }
                rest
            };
            if files.is_empty() {
                return Err(format!("source {} requires at least one file", action.to_string_lossy()));
            }
            Ok(Command::Stage(
                files,
                if action == "type" { source::Stage::Type } else { source::Stage::Resolve },
            ))
        }
        [group, action, ..] if group == "source" => Err(format!("unknown source command '{}'", action.to_string_lossy())),
        [group] if group == "source" => Err("expected a source command".into()),
        [name, ..] => Err(format!("unknown command '{}'", name.to_string_lossy())),
        [] => Err("expected a command".into()),
    }
}

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let result = match command(&arguments) {
        Ok(Command::Help) => writeln!(io::stdout().lock(), "{USAGE}\n\n{HELP}").map_err(|e| format!("omega: could not write help: {e}")),
        Ok(Command::Parse(path)) => source::parse_file(path),
        Ok(Command::Stage(paths, stage)) => source::check_files(paths, stage),
        Err(message) => {
            eprintln!("omega: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
