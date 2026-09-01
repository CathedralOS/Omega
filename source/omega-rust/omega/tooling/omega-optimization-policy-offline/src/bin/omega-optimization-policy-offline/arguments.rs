//! Exact command vocabulary and positional argument admission.

use std::{ffi::OsString, path::PathBuf};

use crate::error::OfflinePolicyCommandError;

pub(super) const USAGE: &str = "usage:
  omega-optimization-policy-offline capture <output-corpus> <decision-log>...
  omega-optimization-policy-offline train <input-corpus> <output-model>
  omega-optimization-policy-offline evaluate <input-corpus> <input-model> <output-report>
  omega-optimization-policy-offline regression <input-corpus> <input-model> <output-report>";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CaptureRequest {
    pub(super) output: PathBuf,
    pub(super) logs: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TrainingRequest {
    pub(super) corpus: PathBuf,
    pub(super) output: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EvaluationRequest {
    pub(super) corpus: PathBuf,
    pub(super) model: PathBuf,
    pub(super) output: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum OfflinePolicyCommand {
    Capture(CaptureRequest),
    Train(TrainingRequest),
    Evaluate(EvaluationRequest),
    Regression(EvaluationRequest),
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
    match command.to_str() {
        Some("capture") => parse_capture(arguments),
        Some("train") => parse_training(arguments),
        Some("evaluate") => parse_evaluation(arguments, false),
        Some("regression") => parse_evaluation(arguments, true),
        _ => Err(OfflinePolicyCommandError::Usage("unknown command")),
    }
}

fn parse_capture(
    mut arguments: impl Iterator<Item = OsString>,
) -> Result<OfflinePolicyCommand, OfflinePolicyCommandError> {
    let output = required_path(&mut arguments, "missing output corpus path")?;
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

fn parse_training(
    mut arguments: impl Iterator<Item = OsString>,
) -> Result<OfflinePolicyCommand, OfflinePolicyCommandError> {
    let corpus = required_path(&mut arguments, "missing input corpus path")?;
    let output = required_path(&mut arguments, "missing output model path")?;
    reject_trailing(&mut arguments, "train accepts exactly two paths")?;
    Ok(OfflinePolicyCommand::Train(TrainingRequest {
        corpus,
        output,
    }))
}

fn parse_evaluation(
    mut arguments: impl Iterator<Item = OsString>,
    regression: bool,
) -> Result<OfflinePolicyCommand, OfflinePolicyCommandError> {
    let corpus = required_path(&mut arguments, "missing input corpus path")?;
    let model = required_path(&mut arguments, "missing input model path")?;
    let output = required_path(&mut arguments, "missing output report path")?;
    reject_trailing(
        &mut arguments,
        if regression {
            "regression accepts exactly three paths"
        } else {
            "evaluate accepts exactly three paths"
        },
    )?;
    let request = EvaluationRequest {
        corpus,
        model,
        output,
    };
    Ok(if regression {
        OfflinePolicyCommand::Regression(request)
    } else {
        OfflinePolicyCommand::Evaluate(request)
    })
}

fn required_path(
    arguments: &mut impl Iterator<Item = OsString>,
    missing: &'static str,
) -> Result<PathBuf, OfflinePolicyCommandError> {
    arguments
        .next()
        .map(PathBuf::from)
        .ok_or(OfflinePolicyCommandError::Usage(missing))
}

fn reject_trailing(
    arguments: &mut impl Iterator<Item = OsString>,
    message: &'static str,
) -> Result<(), OfflinePolicyCommandError> {
    if arguments.next().is_some() {
        return Err(OfflinePolicyCommandError::Usage(message));
    }
    Ok(())
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
