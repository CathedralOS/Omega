//! Exact command vocabulary and positional argument admission.

use std::{ffi::OsString, path::PathBuf};

use crate::error::OfflinePolicyCommandError;

pub(super) const USAGE: &str =
    "usage:\n  omega-optimization-policy-offline capture <output-corpus> <decision-log>...";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CaptureRequest {
    pub(super) output: PathBuf,
    pub(super) logs: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum OfflinePolicyCommand {
    Capture(CaptureRequest),
    Help,
}

pub(super) fn parse(
    arguments: impl IntoIterator<Item = OsString>,
) -> Result<OfflinePolicyCommand, OfflinePolicyCommandError> {
    let mut arguments = arguments.into_iter();
    let _program = arguments.next();
    let Some(command) = arguments.next() else {
        return Err(OfflinePolicyCommandError::Usage("missing command"));
    };

    if command == "--help" || command == "-h" || command == "help" {
        return no_trailing_arguments(arguments, OfflinePolicyCommand::Help);
    }
    if command != "capture" {
        return Err(OfflinePolicyCommandError::Usage("unknown command"));
    }

    let output = arguments
        .next()
        .map(PathBuf::from)
        .ok_or(OfflinePolicyCommandError::Usage(
            "missing output corpus path",
        ))?;
    let logs = arguments.map(PathBuf::from).collect::<Vec<_>>();
    if logs.is_empty() {
        return Err(OfflinePolicyCommandError::Usage(
            "capture requires at least one decision log",
        ));
    }
    Ok(OfflinePolicyCommand::Capture(CaptureRequest {
        output,
        logs,
    }))
}

fn no_trailing_arguments(
    mut arguments: impl Iterator<Item = OsString>,
    command: OfflinePolicyCommand,
) -> Result<OfflinePolicyCommand, OfflinePolicyCommandError> {
    if arguments.next().is_some() {
        return Err(OfflinePolicyCommandError::Usage(
            "help accepts no trailing arguments",
        ));
    }
    Ok(command)
}
