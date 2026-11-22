//! The command-line boundary owns arguments, presentation and process status.

#![forbid(unsafe_code)]

mod source;

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "Usage: omega source parse [--] <file>\n       omega source resolve [--package-root <directory>] [--] <package-relative file>...\n       omega source type [--package-root <directory>] [--] <package-relative file>...\n       omega source prepare [--package-root <directory>] [--] <package-relative file>...\n       omega source lower [--package-root <directory>] [--] <package-relative file>...\n       omega source check --only scalar-replacements [--package-root <directory>] [--] <package-relative file>...";
const HELP: &str = "Parse checks syntax. Resolve checks names. Type checks declarations, expressions and machine calls. Prepare evaluates constants and static indices and checks the resulting types. Lower constructs portable Psi bodies from prepared source. Check --only scalar-replacements checks scalar replacement casts in closed machine bodies under their entry contracts. For these package commands, list the source files of one package rooted at the current directory. With --package-root, paths are relative to that directory. Build captures the complete package there and binds its anonymous owner. This mode requires a root without an authored build.omg. Success produces no output.";

enum Command<'a> {
    Help,
    Parse(&'a Path),
    Stage(&'a [OsString], source::Stage, Option<&'a Path>),
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
        [group, action, rest @ ..]
            if group == "source" && (action == "resolve" || action == "type" || action == "prepare" || action == "lower" || action == "check") =>
        {
            if let [flag] = rest
                && is_help(flag)
            {
                return Ok(Command::Help);
            }
            let (stage, rest) = match action.to_str() {
                Some("check") => match rest {
                    [flag, scope, tail @ ..] if flag == "--only" && scope == "scalar-replacements" => (source::Stage::CheckScalarReplacements, tail),
                    _ => return Err("source check requires --only scalar-replacements".into()),
                },
                Some("lower") => (source::Stage::Lower, rest),
                Some("prepare") => (source::Stage::Prepare, rest),
                Some("type") => (source::Stage::Type, rest),
                _ => (source::Stage::Resolve, rest),
            };
            let (root, rest) = match rest {
                [flag, directory, tail @ ..] if flag == "--package-root" => (Some(Path::new(directory)), tail),
                [flag] if flag == "--package-root" => return Err("--package-root requires a directory".into()),
                _ => (None, rest),
            };
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
            Ok(Command::Stage(files, stage, root))
        }
        [group, action, ..] if group == "source" => Err(format!("unknown source command '{}'", action.to_string_lossy())),
        [group] if group == "source" => Err("expected a source command".into()),
        [name, ..] => Err(format!("unknown command '{}'", name.to_string_lossy())),
        [] => Err("expected a command".into()),
    }
}

fn write_help() -> io::Result<()> {
    // Rust's Unix stdout treats EBADF as a successful write. Use the actual
    // descriptor so redirecting help to a read-only stream reports the error.
    #[cfg(unix)]
    let mut output = {
        use std::os::fd::AsFd;
        std::fs::File::from(io::stdout().as_fd().try_clone_to_owned()?)
    };
    #[cfg(not(unix))]
    let mut output = io::stdout().lock();
    writeln!(output, "{USAGE}\n\n{HELP}")?;
    output.flush()
}

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let result = match command(&arguments) {
        Ok(Command::Help) => write_help().map_err(|e| format!("omega: could not write help: {e}")),
        Ok(Command::Parse(path)) => source::parse_file(path),
        Ok(Command::Stage(paths, stage, root)) => source::check_files(paths, stage, root),
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
