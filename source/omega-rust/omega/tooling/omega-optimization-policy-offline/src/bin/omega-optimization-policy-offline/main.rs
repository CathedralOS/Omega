#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance. Offline policy artifact commands.

mod arguments;
mod capture;
mod error;
mod publication;

#[cfg(test)]
mod tests;

use std::{env, process::ExitCode};

use arguments::OfflinePolicyCommand;
use error::OfflinePolicyCommandError;

fn main() -> ExitCode {
    match run(env::args_os()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            error.exit_code()
        }
    }
}

fn run(
    arguments: impl IntoIterator<Item = std::ffi::OsString>,
) -> Result<(), OfflinePolicyCommandError> {
    match arguments::parse(arguments)? {
        OfflinePolicyCommand::Capture(request) => capture::capture(request),
        OfflinePolicyCommand::Help => {
            println!("{}", arguments::USAGE);
            Ok(())
        }
    }
}
