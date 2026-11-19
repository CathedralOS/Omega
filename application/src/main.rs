//! The command-line boundary owns arguments, presentation and process status.

#![forbid(unsafe_code)]

mod source;

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "Usage: omega source parse [--] <file>";
const HELP: &str = "Check the syntax of one Omega source file. Success produces no output.";

enum Command<'a> {
    Help,
    Parse(&'a Path),
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
        [group, action, ..] if group == "source" => Err(format!("unknown source command '{}'", action.to_string_lossy())),
        [group] if group == "source" => Err("expected a source command".into()),
        [name, ..] => Err(format!("unknown command '{}'", name.to_string_lossy())),
        [] => Err("expected a command".into()),
    }
}

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    match command(&arguments) {
        Ok(Command::Help) => match writeln!(io::stdout().lock(), "{USAGE}\n\n{HELP}") {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("omega: could not write help: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(Command::Parse(path)) => match source::parse_file(path) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        },
        Err(message) => {
            eprintln!("omega: {message}\n{USAGE}");
            ExitCode::from(2)
        }
    }
}
