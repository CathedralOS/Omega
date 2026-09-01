#![forbid(unsafe_code)]

//! Optimizer module role: executable entrance. Offline policy artifact commands.

mod arguments;
mod capture;
mod error;
mod evaluation;
mod inputs;
mod publication;
mod regression_manifest;
mod training;

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
        OfflinePolicyCommand::Train(request) => training::train(request),
        OfflinePolicyCommand::Evaluate(request) => evaluation::evaluate(request),
        OfflinePolicyCommand::Regression(request) => evaluation::regression(request),
        OfflinePolicyCommand::CreateRegressionManifest(request) => {
            regression_manifest::create(request)
        }
        OfflinePolicyCommand::CheckRegressionManifest(request) => {
            regression_manifest::check(request)
        }
        OfflinePolicyCommand::Help => {
            println!("{}", arguments::USAGE);
            Ok(())
        }
    }
}
