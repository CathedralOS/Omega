//! Compile, publish, and optionally execute one project with a private output directory.
//! Interpreter comparison observes the same produced Terminal artifact. This operation
//! neither prints nor exits; callers decide how to present outcomes and exit status.

mod compilation;
mod private_observation;
mod process_exit;
mod standard_input;

pub use process_exit::ProcessExitObservation;

use crate::temporary_directory::TemporaryDirectory;
use compiler::{CompileOptions, CompileReport};
use diagnostics::Diagnostic;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

pub struct RunRequest {
    pub private_entry_ram: Vec<compilation_products::PrivateEntryRamInitialization>,
    pub root_path: PathBuf,
    /// An explicit target produces an image without executing it on the host.
    pub target_name: Option<String>,
    /// Host comparison snapshots process stdin to EOF for independent native
    /// and Terminal replay. Without comparison, native execution inherits stdin.
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
        private_entry_ram: Vec<(u32, Vec<u8>)>,
    },
}

/// Host-presented termination comparison only, not an equivalence proof, a semantic
/// trace comparison or an output comparison. Stored codes remain exact
/// interpreter statuses even when the host presents fewer bits.
#[derive(Debug)]
pub enum InterpreterComparison {
    NotRequested,
    Declined(String),
    Agrees { exit_code: i32 },
    Disagrees { interpreter_exit_code: i32 },
    AgreesCrash { cause: terminal_psi::CrashCause },
    DisagreesCrash { cause: terminal_psi::CrashCause },
    Failed(Vec<Diagnostic>),
}

#[derive(Debug)]
pub enum RunError {
    TemporaryStorage(std::io::Error),
    StandardInput(std::io::Error),
    ReferenceOutput(std::io::Error),
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
            Self::ReferenceOutput(error) => {
                write!(
                    formatter,
                    "cannot retain Terminal reference output: {error}"
                )
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
        interpretations,
    } = compilation::compile(
        request.private_entry_ram,
        CompileOptions {
            root_path: request.root_path,
            build_dir: Some(build_dir.clone()),
            target_name: request.target_name.clone(),
        },
        replay_input
            .as_ref()
            .map(|input| input.open_input())
            .transpose()
            .map_err(RunError::StandardInput)?
            .into_iter()
            .collect(),
    )
    .map_err(RunError::Compilation)?;
    let interpretation = interpretations.into_iter().next();
    if request.keep_artifacts
        && report
            .runtime_provisions()
            .is_some_and(|provisions| !provisions.private_entry_ram().is_empty())
        && let Some(artifact) = report.retained_native_artifact()
    {
        // Keep the exact canonical product and replay model that produced this
        // image, rather than a separate Source recompile or reconstructed frame.
        std::fs::write(
            build_dir.join("program.oterm"),
            artifact.psi_artifact().to_bytes(),
        )
        .map_err(RunError::ReferenceOutput)?;
        std::fs::write(
            build_dir.join("native.object.txt"),
            format!("{:#?}", artifact.object()),
        )
        .map_err(RunError::ReferenceOutput)?;
        std::fs::write(
            build_dir.join("native.identity.txt"),
            format!("{:?}\n", artifact.identity()),
        )
        .map_err(RunError::ReferenceOutput)?;
    }
    let (report, executable) =
        crate::compilation::publication::publish_native_artifact(report, &build_dir)
            .map_err(RunError::Publication)?;
    let execution = match request.target_name {
        Some(target_name) => ExecutionOutcome::TargetOnly { target_name },
        None => {
            let standard_input = match &replay_input {
                Some(input) => Stdio::from(input.open_input().map_err(RunError::StandardInput)?),
                None => Stdio::inherit(),
            };
            let mut command = Command::new(&executable);
            command.stdin(standard_input);
            let (output, private_entry_ram, private_frames) =
                private_observation::capture(command, report.runtime_provisions())
                    .map_err(|error| RunError::Spawn { executable, error })?;
            if request.keep_artifacts {
                if !private_frames.is_empty() {
                    std::fs::write(build_dir.join("native.entry-ram.frames"), &private_frames)
                        .map_err(RunError::ReferenceOutput)?;
                }
                for (position, contents) in &private_entry_ram {
                    std::fs::write(
                        build_dir.join(format!("native.entry-ram.{position}.bin")),
                        contents,
                    )
                    .map_err(RunError::ReferenceOutput)?;
                }
            }
            let exit = ProcessExitObservation::observe(&output.status);
            // `--both --keep` retains the exact reference streams for callers
            // that need to compare numerical bytes rather than exit alone.
            if let Some(Ok(reference)) = interpretation.as_ref()
                && reference.private_entry_ram != private_entry_ram
            {
                return Err(RunError::ReferenceOutput(std::io::Error::other(
                    "native and Terminal original entry backing observations disagree",
                )));
            }
            if request.keep_artifacts
                && let Some(Ok(reference)) = interpretation.as_ref()
            {
                std::fs::write(build_dir.join("terminal.stdout"), &reference.stdout)
                    .map_err(RunError::ReferenceOutput)?;
                std::fs::write(build_dir.join("terminal.stderr"), &reference.stderr)
                    .map_err(RunError::ReferenceOutput)?;
                let events = reference
                    .effects
                    .iter()
                    .map(|effect| format!("{effect:?}\n"))
                    .collect::<String>();
                std::fs::write(build_dir.join("terminal.effects"), events)
                    .map_err(RunError::ReferenceOutput)?;
                for (position, contents) in &reference.private_entry_ram {
                    std::fs::write(
                        build_dir.join(format!("terminal.entry-ram.{position}.bin")),
                        contents,
                    )
                    .map_err(RunError::ReferenceOutput)?;
                }
            }
            let comparison = compare_interpretation(exit, interpretation);
            ExecutionOutcome::Host {
                private_entry_ram,
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

/// Many stdin cases against one compilation: each input file is replayed
/// through the Terminal reference (when requested) and the native program.
pub struct CaseRunRequest {
    pub private_entry_ram: Vec<compilation_products::PrivateEntryRamInitialization>,
    pub root_path: PathBuf,
    pub compare_interpreter: bool,
    /// Input files, run in this order; each case is named by its file stem.
    pub inputs: Vec<PathBuf>,
    /// Receives `<case>.stdout`, `<case>.stderr`, `<case>.terminal.stdout`,
    /// `<case>.terminal.stderr` and `results.tsv`.
    pub output_directory: PathBuf,
    pub keep_artifacts: bool,
}

pub struct CaseOutcome {
    pub name: String,
    pub exit: ProcessExitObservation,
    pub comparison: InterpreterComparison,
}

pub struct CaseRunOutcome {
    pub report: CompileReport,
    pub build_dir: PathBuf,
    pub cases: Vec<CaseOutcome>,
}

pub fn run_project_cases(request: CaseRunRequest) -> Result<CaseRunOutcome, RunError> {
    let output_directory = TemporaryDirectory::create("probe", request.keep_artifacts)
        .map_err(RunError::TemporaryStorage)?;
    let build_dir = output_directory.path().to_path_buf();
    std::fs::create_dir_all(&request.output_directory).map_err(RunError::ReferenceOutput)?;
    let reference_inputs = if request.compare_interpreter {
        request
            .inputs
            .iter()
            .map(std::fs::File::open)
            .collect::<Result<Vec<_>, _>>()
            .map_err(RunError::StandardInput)?
    } else {
        Vec::new()
    };
    let compilation::ProbeCompilation {
        report,
        interpretations,
    } = compilation::compile(
        request.private_entry_ram,
        CompileOptions {
            root_path: request.root_path,
            build_dir: Some(build_dir.clone()),
            target_name: None,
        },
        reference_inputs,
    )
    .map_err(RunError::Compilation)?;
    let (report, executable) =
        crate::compilation::publication::publish_native_artifact(report, &build_dir)
            .map_err(RunError::Publication)?;
    let mut interpretations = interpretations.into_iter();
    let mut cases = Vec::new();
    let mut table = String::from("case\tnative\tinterpreter\tinterpreter_exit\n");
    for input in &request.inputs {
        let name = input
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut command = Command::new(&executable);
        command.stdin(Stdio::from(
            std::fs::File::open(input).map_err(RunError::StandardInput)?,
        ));
        let (output, private_entry_ram, private_frames) =
            private_observation::capture(command, report.runtime_provisions()).map_err(
                |error| RunError::Spawn {
                    executable: executable.clone(),
                    error,
                },
            )?;
        let directory = &request.output_directory;
        let write = |suffix: &str, bytes: &[u8]| {
            std::fs::write(directory.join(format!("{name}.{suffix}")), bytes)
                .map_err(RunError::ReferenceOutput)
        };
        write("stdout", &output.stdout)?;
        write("stderr", &output.stderr)?;
        if !private_frames.is_empty() {
            write("native.entry-ram.frames", &private_frames)?;
        }
        for (position, contents) in &private_entry_ram {
            write(&format!("native.entry-ram.{position}.bin"), contents)?;
        }
        let interpretation = interpretations.next();
        if let Some(Ok(reference)) = interpretation.as_ref() {
            write("terminal.stdout", &reference.stdout)?;
            write("terminal.stderr", &reference.stderr)?;
        }
        let exit = ProcessExitObservation::observe(&output.status);
        let comparison = compare_interpretation(exit, interpretation);
        let (verdict, interpreter_exit) = comparison_columns(&comparison);
        table.push_str(&format!(
            "{name}\t{}\t{verdict}\t{interpreter_exit}\n",
            exit.code()
                .map_or_else(|| exit.describe(), |code| code.to_string())
        ));
        cases.push(CaseOutcome {
            name,
            exit,
            comparison,
        });
    }
    std::fs::write(request.output_directory.join("results.tsv"), table)
        .map_err(RunError::ReferenceOutput)?;
    Ok(CaseRunOutcome {
        report,
        build_dir,
        cases,
    })
}

fn comparison_columns(comparison: &InterpreterComparison) -> (&'static str, String) {
    match comparison {
        InterpreterComparison::NotRequested => ("not-requested", String::new()),
        InterpreterComparison::Declined(_) => ("declined", String::new()),
        InterpreterComparison::Agrees { exit_code } => ("agrees", exit_code.to_string()),
        InterpreterComparison::Disagrees {
            interpreter_exit_code,
        } => ("disagrees", interpreter_exit_code.to_string()),
        InterpreterComparison::AgreesCrash { cause } => ("agrees-crash", format!("{cause:?}")),
        InterpreterComparison::DisagreesCrash { cause } => {
            ("disagrees-crash", format!("{cause:?}"))
        }
        InterpreterComparison::Failed(_) => ("failed", String::new()),
    }
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
                    return match native_exit.matches_crash_presentation(crash.cause) {
                        Some(true) => InterpreterComparison::AgreesCrash { cause: crash.cause },
                        Some(false) => InterpreterComparison::DisagreesCrash { cause: crash.cause },
                        None => InterpreterComparison::Declined(format!(
                            "Terminal observed {:?}; host crash-presentation comparison is unavailable",
                            crash.cause
                        )),
                    };
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
