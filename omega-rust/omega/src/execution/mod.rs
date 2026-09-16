//! Compile, publish, and optionally execute one project with a private output directory.
//! Interpreter comparison observes the same checked package input. This operation
//! neither prints nor exits; callers decide how to present outcomes and exit status.

mod compilation;
mod temporary_output;

use compiler::{CompileOptions, CompileReport, TrustAdmissionSettlement};
use diagnostics::Diagnostic;
use std::path::PathBuf;
use std::process::{Command, Output};
use temporary_output::TemporaryOutput;

pub struct RunRequest {
    pub root_path: PathBuf,
    /// An explicit target produces an image without executing it on the host.
    pub target_name: Option<String>,
    pub compare_interpreter: bool,
    pub keep_artifacts: bool,
}

pub struct RunOutcome {
    pub report: CompileReport,
    /// Removed on return unless keep_artifacts was requested.
    pub build_dir: PathBuf,
    pub execution: ExecutionOutcome,
}

pub enum ExecutionOutcome {
    TargetOnly {
        target_name: String,
    },
    Host {
        output: Output,
        comparison: InterpreterComparison,
    },
}

/// Exit-code comparison only, not an equivalence proof or an output comparison.
#[derive(Debug)]
pub enum InterpreterComparison {
    NotRequested,
    Declined(String),
    Agrees { exit_code: i32 },
    Disagrees { interpreter_exit_code: i32 },
    Failed(Vec<Diagnostic>),
}

#[derive(Debug)]
pub enum RunError {
    TemporaryStorage(std::io::Error),
    Compilation(Vec<Diagnostic>),
    UnsettledAdmissions(TrustAdmissionSettlement),
    Publication(String),
    Spawn {
        executable: PathBuf,
        error: std::io::Error,
    },
}

impl std::fmt::Display for RunError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TemporaryStorage(error) => {
                write!(formatter, "cannot create run output directory: {error}")
            }
            Self::Compilation(diagnostics) => {
                write!(formatter, "native compile FAILED:")?;
                for diagnostic in diagnostics {
                    write!(formatter, "\n  {diagnostic}")?;
                }
                Ok(())
            }
            Self::UnsettledAdmissions(_) => {
                write!(formatter, "project trust admissions are not settled")
            }
            Self::Publication(error) => write!(formatter, "native publication FAILED: {error}"),
            Self::Spawn { executable, error } => write!(
                formatter,
                "native run failed to spawn {}: {error}",
                executable.display()
            ),
        }
    }
}

impl std::error::Error for RunError {}

pub fn run_project(request: RunRequest) -> Result<RunOutcome, RunError> {
    let output_directory =
        TemporaryOutput::create(request.keep_artifacts).map_err(RunError::TemporaryStorage)?;
    let build_dir = output_directory.path().to_path_buf();
    let compilation::ProbeCompilation {
        report,
        interpretation,
    } = compilation::compile(
        CompileOptions {
            root_path: request.root_path,
            build_dir: Some(build_dir.clone()),
            target_name: request.target_name.clone(),
        },
        request.compare_interpreter && request.target_name.is_none(),
    )
    .map_err(RunError::Compilation)?;
    if !report.trust_admission_settlement().is_exactly_admitted() {
        return Err(RunError::UnsettledAdmissions(
            report.trust_admission_settlement().clone(),
        ));
    }
    let (report, executable) =
        crate::compilation::publication::publish_native_artifact(report, &build_dir)
            .map_err(RunError::Publication)?;
    let execution = match request.target_name {
        Some(target_name) => ExecutionOutcome::TargetOnly { target_name },
        None => {
            let output = Command::new(&executable)
                .output()
                .map_err(|error| RunError::Spawn { executable, error })?;
            let comparison = compare_interpretation(output.status.code(), interpretation);
            ExecutionOutcome::Host { output, comparison }
        }
    };
    Ok(RunOutcome {
        report,
        build_dir,
        execution,
    })
}

fn compare_interpretation(
    native_exit: Option<i32>,
    interpretation: Option<Result<checked_interpreter::InterpretOutcome, Vec<Diagnostic>>>,
) -> InterpreterComparison {
    match interpretation {
        None => InterpreterComparison::NotRequested,
        Some(Err(diagnostics)) => InterpreterComparison::Failed(diagnostics),
        Some(Ok(outcome)) => match outcome.error {
            Some(reason) => InterpreterComparison::Declined(reason),
            None if native_exit == Some(outcome.exit_code) => InterpreterComparison::Agrees {
                exit_code: outcome.exit_code,
            },
            None => InterpreterComparison::Disagrees {
                interpreter_exit_code: outcome.exit_code,
            },
        },
    }
}
