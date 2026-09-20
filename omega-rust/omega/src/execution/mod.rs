//! Compile, publish, and optionally execute one project with a private output directory.
//! Interpreter comparison observes the same checked package input. This operation
//! neither prints nor exits; callers decide how to present outcomes and exit status.

mod compilation;

use crate::temporary_directory::TemporaryDirectory;
use compiler::{CompileOptions, CompileReport, TrustAdmissionSettlement};
use diagnostics::Diagnostic;
use std::path::PathBuf;
use std::process::{Command, Output};

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
/// observation keeps the distinction for presentation while exposing the
/// single value the interpreter-agreement comparison needs.
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

    /// One-line presentation of the termination for run diagnostics.
    pub fn describe(self) -> String {
        match self {
            Self::Exited(code) => format!("exit {code}"),
            Self::Signaled(signal) => format!("signal {signal}"),
            Self::Terminated => "terminated without an exit code".to_owned(),
        }
    }
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
    let output_directory = TemporaryDirectory::create("probe", request.keep_artifacts)
        .map_err(RunError::TemporaryStorage)?;
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
    interpretation: Option<Result<checked_interpreter::InterpretOutcome, Vec<Diagnostic>>>,
) -> InterpreterComparison {
    match interpretation {
        None => InterpreterComparison::NotRequested,
        Some(Err(diagnostics)) => InterpreterComparison::Failed(diagnostics),
        Some(Ok(outcome)) => match outcome.error {
            Some(reason) => InterpreterComparison::Declined(reason),
            None if native_exit.code() == Some(outcome.exit_code) => {
                InterpreterComparison::Agrees {
                    exit_code: outcome.exit_code,
                }
            }
            None => InterpreterComparison::Disagrees {
                interpreter_exit_code: outcome.exit_code,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_code_exposes_only_reported_exit_codes() {
        assert_eq!(ProcessExitObservation::Exited(7).code(), Some(7));
        assert_eq!(ProcessExitObservation::Signaled(11).code(), None);
        assert_eq!(ProcessExitObservation::Terminated.code(), None);
    }

    #[test]
    fn observation_describe_names_each_termination_channel() {
        assert_eq!(ProcessExitObservation::Exited(42).describe(), "exit 42");
        assert_eq!(ProcessExitObservation::Signaled(11).describe(), "signal 11");
        assert_eq!(
            ProcessExitObservation::Terminated.describe(),
            "terminated without an exit code"
        );
    }

    #[cfg(unix)]
    #[test]
    fn observe_reads_the_unix_wait_status_portably() {
        use std::os::unix::process::ExitStatusExt;

        // Wait-status layout: exited children carry the code in the high byte;
        // signal deaths carry the signal in the low seven bits with no code.
        let exited = std::process::ExitStatus::from_raw(42 << 8);
        let signaled = std::process::ExitStatus::from_raw(11);

        assert_eq!(
            ProcessExitObservation::observe(&exited),
            ProcessExitObservation::Exited(42)
        );
        assert_eq!(
            ProcessExitObservation::observe(&signaled),
            ProcessExitObservation::Signaled(11)
        );
    }

    #[cfg(unix)]
    #[test]
    fn observe_does_not_fabricate_a_code_for_signal_death() {
        use std::os::unix::process::ExitStatusExt;

        // A raw status with neither an exit-code channel nor a signal payload
        // (e.g. a stopped-process encoding read as terminated) must surface as
        // Terminated, not as a synthesized numeric exit.
        let status = std::process::ExitStatus::from_raw(0x7f);
        let observed = ProcessExitObservation::observe(&status);

        assert_ne!(
            observed.code(),
            Some(-1),
            "a missing exit code must never collapse into a fabricated -1"
        );
        assert!(matches!(
            observed,
            ProcessExitObservation::Signaled(127) | ProcessExitObservation::Terminated
        ));
    }
}
