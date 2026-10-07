//! Compile, publish, and optionally execute one project with a private output directory.
//! Interpreter comparison observes the same produced Terminal artifact. This operation
//! neither prints nor exits; callers decide how to present outcomes and exit status.

mod compilation;
mod standard_input;

use crate::temporary_directory::TemporaryDirectory;
use compiler::{CompileOptions, CompileReport};
use diagnostics::Diagnostic;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

pub struct RunRequest {
    pub root_path: PathBuf,
    /// An explicit target produces an image without executing it on the host.
    pub target_name: Option<String>,
    /// Host comparison snapshots process stdin to EOF for native replay.
    /// Terminal stdin imports currently decline until their exact adapters
    /// are supplied. Without comparison, native execution inherits stdin.
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
        /// How the child terminated, observed portably.
        exit: ProcessExitObservation,
        comparison: InterpreterComparison,
    },
}

/// Portable observation of one child process's termination.
///
/// `ExitStatus::code` reports `None` when the host delivers no exit code —
/// on Unix that means signal death — and collapsing it into a numeric
/// fallback conflates signal termination with a real exit code. The
/// observation keeps the distinction for presentation and comparison. It is
/// host presentation, not the exact semantic process-exit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessExitObservation {
    /// The process terminated through the host's exit-code channel.
    Exited(i32),
    /// The process died on a Unix signal without reporting an exit code.
    Signaled(i32),
    /// The process ended with no observable exit code on this host.
    Terminated,
}

impl ProcessExitObservation {
    pub fn observe(status: &std::process::ExitStatus) -> Self {
        if let Some(code) = status.code() {
            return Self::Exited(code);
        }
        #[cfg(unix)]
        if let Some(signal) = std::os::unix::process::ExitStatusExt::signal(status) {
            return Self::Signaled(signal);
        }
        Self::Terminated
    }

    /// The reported exit code, or `None` when the host supplied none.
    pub const fn code(self) -> Option<i32> {
        match self {
            Self::Exited(code) => Some(code),
            Self::Signaled(_) | Self::Terminated => None,
        }
    }

    /// Compare only what the executing host exposes. Unix wait status retains
    /// the low eight bits; Windows retains the full i32 bit pattern. Mapping
    /// belongs here, not in the interpreter: its semantic status must remain
    /// exact, and agreement here cannot establish full semantic trace equality.
    fn matches_host_presentation(self, semantic_status: i32) -> bool {
        let expected_status = if cfg!(unix) {
            semantic_status & 255
        } else {
            semantic_status
        };
        self.code() == Some(expected_status)
    }

    /// One-line presentation of the termination for run diagnostics.
    pub fn describe(self) -> String {
        match self {
            Self::Exited(code) => format!("exit {code}"),
            Self::Signaled(signal) => format!("signal {signal}"),
            Self::Terminated => "terminated without an exit code".to_owned(),
        }
    }
}

/// Host-presented exit comparison only, not an equivalence proof, a semantic
/// trace comparison or an output comparison. Stored codes remain exact
/// interpreter statuses even when the host presents fewer bits.
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
    StandardInput(std::io::Error),
    Compilation(Vec<Diagnostic>),
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
            Self::StandardInput(error) => {
                write!(formatter, "cannot prepare run standard input: {error}")
            }
            Self::Compilation(diagnostics) => {
                write!(formatter, "native compile FAILED:")?;
                for diagnostic in diagnostics {
                    write!(formatter, "\n  {diagnostic}")?;
                }
                Ok(())
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
    let output_directory = TemporaryDirectory::create("probe", request.keep_artifacts)
        .map_err(RunError::TemporaryStorage)?;
    let build_dir = output_directory.path().to_path_buf();
    // Only comparison needs a finite snapshot. Ordinary execution inherits the
    // caller's stream, and target-only compilation must never consume it.
    let replay_input = if request.compare_interpreter && request.target_name.is_none() {
        Some(
            standard_input::ReplayInput::capture(std::io::stdin().lock())
                .map_err(RunError::StandardInput)?,
        )
    } else {
        None
    };
    let compilation::ProbeCompilation {
        report,
        interpretation,
    } = compilation::compile(
        CompileOptions {
            root_path: request.root_path,
            build_dir: Some(build_dir.clone()),
            target_name: request.target_name.clone(),
        },
        replay_input.is_some(),
    )
    .map_err(RunError::Compilation)?;
    let (report, executable) =
        crate::compilation::publication::publish_native_artifact(report, &build_dir)
            .map_err(RunError::Publication)?;
    let execution = match request.target_name {
        Some(target_name) => ExecutionOutcome::TargetOnly { target_name },
        None => {
            let standard_input = match &replay_input {
                Some(input) => {
                    Stdio::from(input.open_native_input().map_err(RunError::StandardInput)?)
                }
                None => Stdio::inherit(),
            };
            let output = Command::new(&executable)
                .stdin(standard_input)
                .output()
                .map_err(|error| RunError::Spawn { executable, error })?;
            let exit = ProcessExitObservation::observe(&output.status);
            let comparison = compare_interpretation(exit, interpretation);
            ExecutionOutcome::Host {
                output,
                exit,
                comparison,
            }
        }
    };
    Ok(RunOutcome {
        report,
        build_dir,
        execution,
    })
}

fn compare_interpretation(
    native_exit: ProcessExitObservation,
    interpretation: Option<
        Result<
            compiler::runtime::TerminalRuntimeObservation,
            compiler::runtime::TerminalRuntimeError,
        >,
    >,
) -> InterpreterComparison {
    use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};

    match interpretation {
        None => InterpreterComparison::NotRequested,
        Some(Err(error)) if comparison_unavailable(&error) => {
            InterpreterComparison::Declined(error.to_string())
        }
        Some(Err(error)) => {
            InterpreterComparison::Failed(vec![Diagnostic::error(error.to_string())])
        }
        Some(Ok(outcome)) => {
            let exit_code = match outcome.status {
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => 0,
                TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
                TerminalExecutionStatus::Complete(result) => {
                    return InterpreterComparison::Declined(format!(
                        "program entry returned a non-process result: {result:?}"
                    ));
                }
                TerminalExecutionStatus::SponsorExhausted(_) => {
                    return InterpreterComparison::Declined(
                        "Terminal reference execution exhausted its sponsor fuel allowance"
                            .to_owned(),
                    );
                }
                TerminalExecutionStatus::Crashed(crash) => {
                    return InterpreterComparison::Declined(format!(
                        "Terminal observed {:?}; host crash-presentation comparison is unavailable",
                        crash.cause
                    ));
                }
            };
            if native_exit.matches_host_presentation(exit_code) {
                InterpreterComparison::Agrees { exit_code }
            } else {
                InterpreterComparison::Disagrees {
                    interpreter_exit_code: exit_code,
                }
            }
        }
    }
}

/// Unsupported interpretation can surface during requirement discovery,
/// instantiation, or execution. Preserve that typed distinction at every
/// boundary; malformed artifacts and missing custody are still failures.
fn comparison_unavailable(error: &compiler::runtime::TerminalRuntimeError) -> bool {
    use compiler::runtime::TerminalRuntimeError;
    use terminal_interpreter::{
        TerminalArtifactInterpretError, TerminalInstanceError, TerminalInterpretError,
    };

    let interpretation = match error {
        TerminalRuntimeError::MissingProvision { .. } => return true,
        TerminalRuntimeError::Requirements(error)
        | TerminalRuntimeError::Artifact(TerminalArtifactInterpretError::Execution(error))
        | TerminalRuntimeError::Execution(TerminalInstanceError::Execution(error)) => error,
        _ => return false,
    };
    matches!(
        interpretation,
        TerminalInterpretError::UnsupportedCapability(_)
            | TerminalInterpretError::UnsupportedSemanticVariant(_)
            | TerminalInterpretError::ScalarEntryQualificationUnsupported
            | TerminalInterpretError::AffineProjectionNotRepresentable
    )
}
