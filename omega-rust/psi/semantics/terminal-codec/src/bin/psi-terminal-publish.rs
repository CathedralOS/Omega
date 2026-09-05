#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};

use terminal_codec::TerminalSemanticArtifactPublication;

fn main() -> ExitCode {
    match run(std::env::args_os().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("psi-terminal-publish: {error}");
            ExitCode::from(1)
        }
    }
}

fn run(arguments: Vec<OsString>) -> Result<(), String> {
    let parsed = Arguments::parse(arguments)?;
    let publication = TerminalSemanticArtifactPublication::begin(&parsed.destination)
        .map_err(|error| error.to_string())?;
    let producer_stdout = publication
        .producer_output()
        .map_err(|error| error.to_string())?;

    let status = Command::new(&parsed.program)
        .args(&parsed.program_arguments)
        .stdout(Stdio::from(producer_stdout))
        .status()
        .map_err(|error| format!("could not run producer {:?}: {error}", parsed.program))?;
    if status.code() != Some(parsed.success_exit) {
        return Err(format!(
            "producer exited with {status}; expected status {}",
            parsed.success_exit
        ));
    }

    let expected_bytes = parsed
        .expected
        .as_ref()
        .map(fs::read)
        .transpose()
        .map_err(|error| format!("could not read expected artifact: {error}"))?;
    let receipt = publication
        .publish(expected_bytes.as_deref())
        .map_err(|error| error.to_string())?;
    eprintln!(
        "published {} bytes as terminal Psi {}:{} to {}",
        receipt.byte_len,
        receipt.identity.vocabulary_marker.get(),
        receipt.identity.program_fingerprint,
        receipt.path.display()
    );
    Ok(())
}

struct Arguments {
    success_exit: i32,
    expected: Option<PathBuf>,
    destination: PathBuf,
    program: OsString,
    program_arguments: Vec<OsString>,
}

impl Arguments {
    fn parse(arguments: Vec<OsString>) -> Result<Self, String> {
        let mut success_exit = 0;
        let mut expected = None;
        let mut cursor = 0;

        while let Some(argument) = arguments.get(cursor) {
            if argument == "--success-exit" {
                let raw = arguments
                    .get(cursor + 1)
                    .ok_or_else(|| "--success-exit requires a value".to_owned())?;
                success_exit = raw
                    .to_str()
                    .ok_or_else(|| "--success-exit must be UTF-8".to_owned())?
                    .parse::<i32>()
                    .map_err(|_| "--success-exit must be an integer".to_owned())?;
                if !(0..=255).contains(&success_exit) {
                    return Err("--success-exit must be between 0 and 255".to_owned());
                }
                cursor += 2;
            } else if argument == "--expect" {
                expected = Some(PathBuf::from(
                    arguments
                        .get(cursor + 1)
                        .ok_or_else(|| "--expect requires a path".to_owned())?,
                ));
                cursor += 2;
            } else {
                break;
            }
        }

        let destination = arguments
            .get(cursor)
            .filter(|argument| *argument != "--")
            .map(PathBuf::from)
            .ok_or_else(usage)?;
        cursor += 1;
        if arguments
            .get(cursor)
            .is_none_or(|argument| argument != "--")
        {
            return Err(usage());
        }
        cursor += 1;
        let program = arguments.get(cursor).cloned().ok_or_else(usage)?;
        let program_arguments = arguments[cursor + 1..].to_vec();

        Ok(Self {
            success_exit,
            expected,
            destination,
            program,
            program_arguments,
        })
    }
}

fn usage() -> String {
    "usage: psi-terminal-publish [--success-exit CODE] [--expect CANONICAL] OUTPUT -- PRODUCER [ARG ...]".to_owned()
}
