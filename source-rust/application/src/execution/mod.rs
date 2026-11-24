/*

Compile, publish, and execute an Omega project (`omega run`).

This module owns the execution pipeline behind `commands::run` without printing
to the console or exiting the process; callers decide how to present `RunOutcome`
and `CaseRunOutcome`.

We support two entry points:
- `run_project`: compiles the project into a temporary `probe` directory (removed
  on return unless `keep_artifacts` is set) and runs it according to
  `ExecutionSelection`:
  An explicit target makes `Native` and `Both` stop after publication
  (`ExecutionOutcome::TargetOnly`), even when the target matches the host.
  - `Native`: emits and publishes the native executable
    (`ExecutionProduct::Native`) and spawns it on the host.
  - `Terminal`: stops compilation after Build, package, trust, provider, and
    entry admission (`ExecutionProduct::Runtime`) without emitting any
    object file or native executable, and runs the admitted Terminal Psi program
    directly on `terminal_runtime` with the caller's live `stdin`.
  - `Both`: runs the produced Terminal Psi artifact on `terminal_runtime`,
    publishes and spawns the native executable, verifies that any
    `private_entry_ram` observations match byte-for-byte, and compares how both
    runs terminated and what each wrote to stdout and stderr
    (`InterpreterComparison`).
- `run_project_cases`: compiles once for the selected engine. Each input gets
  its own invocation and streams. Psi executes the runtime product directly.
  Native and comparison batches publish the executable once and launch a new
  guest for each input. The case table records outcomes in input order.

Reference runs are sponsored by the CLI with `DEFAULT_PSI_WORK_ALLOWANCE`
(10,000,000 work units) unless overridden by `psi_work_budget`. Exhausting
work during `--both` declines the comparison (`InterpreterComparison::Declined`),
and so does a Psi exit status the host can show only truncated; `omega run`
then exits 204, because a comparison that did not happen is not agreement. When `keep_artifacts` is true, we also write
`terminal.stdout`, `terminal.stderr`, `psi.work.tsv`, `terminal.effects`,
and any `*.entry-ram.*` snapshots into `build_dir`.

*/

/*

Why `--both` captures `stdin` lazily and distinguishes preflight unavailability:

1. Lazy `stdin` snapshot (`ReplayInput::capture`):
   When running both engines (`ExecutionSelection::Both`), both the Terminal Psi
   interpreter and the native child process need to see the exact same `stdin`
   bytes from offset 0. However, snapping caller `stdin` to EOF unconditionally
   before execution would hang any program that does not read `stdin` whenever
   `omega run --both` is invoked from an interactive terminal or an open pipe.
   Instead, we first call `psi::execute` with `stdin = None`. If the program
   does not import `runtime-standard-input`, preflight and execution finish
   without touching caller `stdin`, and we pass `Stdio::null()` to the native
   child. Only when preflight refuses with `MissingProvision { capability:
   "runtime-standard-input", .. }` do we snapshot caller `stdin` into a
   temporary file (`ReplayInput`) and replay fresh file handles to both engines.

2. `TerminalOutcome::Unavailable` vs. `TerminalOutcome::Failed`:
   In `run_terminal`, an error where `unavailable_before_guest_effects()` is true
   (a missing provider, host adapter, or interpreter capability caught during
   preflight) becomes `Unavailable`. Any failure after guest execution has
   started becomes `Failed`, because earlier boundary calls may already have
   performed host side effects and any buffered output inside the runtime is
   lost.

3. Host-presented exit comparison (`InterpreterComparison`):
   `InterpreterComparison` compares whether the native process exit status
   (`ProcessExitObservation`) agrees with how the host OS presents the
   interpreter's completion or trap. The enum still stores the exact 32-bit
   interpreter exit code even on Unix, where `waitpid` only preserves the low 8
   bits.

4. Output comparison (`output_comparison`):
   Once both runs finished the same way, their stdout and stderr bytes must
   also match; otherwise the result is `InterpreterComparison::OutputDisagrees`
   with the first differing line. A runnable program's output is part of its
   result, so equal exit statuses alone are not agreement.

*/

mod cases;
mod compilation;
mod private_observation;
mod psi;
mod retained_artifacts;
mod standard_input;
mod termination;

pub use cases::{CaseOutcome, CaseRunRequest, run_project_cases};
pub(crate) use termination::PsiTermination;

pub use project_build::observations::ProcessExitObservation;

use project_build::observations::{OutputDivergence, ProgramStreams, first_output_difference};

use diagnostics::Diagnostic;
use package_compilation::build_execution::BuildSnapshotRequest;
use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

const DEFAULT_PSI_WORK_ALLOWANCE: u64 = 10_000_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExecutionSelection {
    #[default]
    Native,
    Terminal,
    Both, // Differential comparison (Native + Terminal), not a third engine.
}

pub struct RunRequest {
    pub build_snapshot: Option<BuildSnapshotRequest>,
    pub private_entry_ram: Vec<psi_entry_memory::PrivateEntryRamInitialization>,
    pub root_path: PathBuf,
    pub target_name: Option<String>,
    pub selection: ExecutionSelection,
    pub psi_work_budget: Option<std::num::NonZeroU64>, // None selects DEFAULT_PSI_WORK_ALLOWANCE.
    pub keep_artifacts: bool,
}

pub struct RunOutcome {
    pub build_dir: PathBuf, // Removed on return unless keep_artifacts was requested.
    pub execution: ExecutionOutcome,
}

pub enum ExecutionOutcome {
    TargetOnly {
        target_name: String,
    },
    Host {
        can_end_the_process: Option<bool>,
        output: Output,
        exit: ProcessExitObservation,
        comparison: InterpreterComparison,
        private_entry_ram: Vec<(u32, Vec<u8>)>,
    },
    Terminal(TerminalOutcome),
}

pub enum TerminalOutcome {
    Completed(terminal_runtime::TerminalRuntimeObservation),
    Unavailable(String),
    Failed(String),
}

#[derive(Debug)]
pub enum InterpreterComparison {
    NotRequested,
    Declined(String),
    Agrees {
        exit_code: i32,
    },
    Disagrees {
        interpreter_exit_code: i32,
    },
    AgreesCrash {
        cause: terminal_psi::CrashCause,
    },
    DisagreesCrash {
        cause: terminal_psi::CrashCause,
    },
    // Both runs finished the same way but wrote different output.
    OutputDisagrees {
        termination: AgreedTermination,
        divergence: OutputDivergence,
    },
    Failed(Vec<Diagnostic>),
}

// How both runs finished when they finished the same way.
#[derive(Debug, Clone, Copy)]
pub enum AgreedTermination {
    Exit(i32),
    Crash(terminal_psi::CrashCause),
}

impl AgreedTermination {
    fn agreement(self) -> InterpreterComparison {
        match self {
            Self::Exit(exit_code) => InterpreterComparison::Agrees { exit_code },
            Self::Crash(cause) => InterpreterComparison::AgreesCrash { cause },
        }
    }
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
                write!(formatter, "cannot retain Psi run output: {error}")
            }
            Self::Compilation(diagnostics) => {
                formatter.write_str("compilation failed")?;
                diagnostics::write_bounded_diagnostics(formatter, diagnostics)
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

/// Admit the selected product, present its registered plan and run its tests
/// before handing it to the application engine.
fn prepare_program(
    artifact: Option<psi_authorization::AuthorizedTerminalArtifact>,
    input: compilation::ProjectInput,
    selection: ExecutionSelection,
    mut present_test: impl FnMut(&str),
) -> Result<compilation::RunProduct, RunError> {
    use crate::saved_program::ExecutionProduct;
    let requested = match selection {
        ExecutionSelection::Terminal => ExecutionProduct::Runtime,
        ExecutionSelection::Native | ExecutionSelection::Both => ExecutionProduct::Native,
    };
    let prepared =
        compilation::compile(artifact, input, requested).map_err(RunError::Compilation)?;
    if let Some(plan) = prepared.tests {
        for row in plan.rows() {
            present_test(&row);
        }
        for verdict in plan.execute().map_err(RunError::Compilation)? {
            present_test(&verdict);
        }
    }
    Ok(prepared.product)
}

pub fn run_project(
    artifact: Option<psi_authorization::AuthorizedTerminalArtifact>,
    request: RunRequest,
    present_test: impl FnMut(&str),
) -> Result<RunOutcome, RunError> {
    let psi_work_budget = request
        .psi_work_budget
        .map_or(DEFAULT_PSI_WORK_ALLOWANCE, std::num::NonZeroU64::get);
    let output_directory = TemporaryDirectory::create("probe", request.keep_artifacts)
        .map_err(RunError::TemporaryStorage)?;
    let build_dir = output_directory.path().to_path_buf();
    let report = prepare_program(
        artifact,
        compilation::ProjectInput {
            root_path: request.root_path,
            build_directory: build_dir.clone(),
            private_entry_ram: request.private_entry_ram,
            build_snapshot: request.build_snapshot,
            target_name: request.target_name.clone(),
        },
        request.selection,
        present_test,
    )?;
    if request.selection == ExecutionSelection::Terminal {
        return run_terminal(report, request.keep_artifacts, build_dir, psi_work_budget);
    }
    let (replay_input, interpretation) =
        if request.selection == ExecutionSelection::Both && request.target_name.is_none() {
            match psi::execute(&report, None, psi_work_budget) {
                Err(terminal_runtime::TerminalRuntimeError::MissingProvision {
                    capability: "runtime-standard-input",
                    ..
                }) => {
                    let captured = standard_input::ReplayInput::capture(std::io::stdin().lock())
                        .map_err(RunError::StandardInput)?;
                    let input = captured.open_input().map_err(RunError::StandardInput)?;
                    let retry = psi::execute(&report, Some(Box::new(input)), psi_work_budget);
                    (Some(captured), Some(retry))
                }
                outcome => (None, Some(outcome)),
            }
        } else {
            (None, None)
        };
    if request.keep_artifacts {
        retained_artifacts::retain(&report, &build_dir)?;
    }
    // Publication consumes the retained Psi when it installs the executable.
    let can_end_the_process = report
        .artifact()
        .map(|published| published.artifact.semantic_module().can_end_the_process());
    let (report, executable) = report.publish(&build_dir).map_err(RunError::Publication)?;
    let execution = match request.target_name {
        Some(target_name) => ExecutionOutcome::TargetOnly { target_name },
        None => {
            let standard_input = match (&replay_input, &interpretation) {
                (Some(input), _) => {
                    Stdio::from(input.open_input().map_err(RunError::StandardInput)?)
                }
                (None, Some(Ok(_))) => Stdio::null(),
                (None, _) => Stdio::inherit(),
            };
            report
                .validate_component_process_inputs(&executable)
                .map_err(RunError::Publication)?;
            let mut command = Command::new(&executable);
            command.stdin(standard_input);
            let (output, private_entry_ram, private_frames) =
                private_observation::capture(command, report.private_entry_ram())
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
            if let Some(Ok(reference)) = interpretation.as_ref()
                && reference.private_entry_ram != private_entry_ram
            {
                return Err(RunError::ReferenceOutput(std::io::Error::other(
                    "native and Psi original entry backing observations disagree",
                )));
            }
            if request.keep_artifacts
                && let Some(Ok(reference)) = interpretation.as_ref()
            {
                retain_terminal_observation(&build_dir, reference, psi_work_budget)?;
            }
            let comparison = compare_interpretation(
                exit,
                ProgramStreams {
                    stdout: &output.stdout,
                    stderr: &output.stderr,
                },
                interpretation,
            );
            ExecutionOutcome::Host {
                can_end_the_process,
                private_entry_ram,
                output,
                exit,
                comparison,
            }
        }
    };
    Ok(RunOutcome {
        build_dir,
        execution,
    })
}

fn run_terminal(
    report: compilation::RunProduct,
    keep_artifacts: bool,
    build_dir: PathBuf,
    psi_work_budget: u64,
) -> Result<RunOutcome, RunError> {
    if keep_artifacts {
        retained_artifacts::retain(&report, &build_dir)?;
    }
    let outcome = TerminalOutcome::from(psi::execute(
        &report,
        Some(Box::new(std::io::stdin())),
        psi_work_budget,
    ));
    if keep_artifacts && let TerminalOutcome::Completed(observation) = &outcome {
        retain_terminal_observation(&build_dir, observation, psi_work_budget)?;
    }
    Ok(RunOutcome {
        build_dir,
        execution: ExecutionOutcome::Terminal(outcome),
    })
}

fn retain_terminal_observation(
    build_dir: &std::path::Path,
    reference: &terminal_runtime::TerminalRuntimeObservation,
    psi_work_budget: u64,
) -> Result<(), RunError> {
    std::fs::write(build_dir.join("terminal.stdout"), &reference.stdout)
        .map_err(RunError::ReferenceOutput)?;
    std::fs::write(build_dir.join("terminal.stderr"), &reference.stderr)
        .map_err(RunError::ReferenceOutput)?;
    std::fs::write(
        build_dir.join("psi.work.tsv"),
        format_work_tsv(reference, psi_work_budget),
    )
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
    Ok(())
}

fn format_work_tsv(
    reference: &terminal_runtime::TerminalRuntimeObservation,
    psi_work_budget: u64,
) -> String {
    format!(
        "work_budget\t{psi_work_budget}\nwork_used\t{}\nwork_schedule\t{:?}\n",
        reference.work_usage.total_units(),
        reference.work_usage.schedule(),
    )
}

fn compare_interpretation(
    native_exit: ProcessExitObservation,
    native_streams: ProgramStreams<'_>,
    interpretation: Option<
        Result<
            terminal_runtime::TerminalRuntimeObservation,
            terminal_runtime::TerminalRuntimeError,
        >,
    >,
) -> InterpreterComparison {
    match interpretation {
        None => InterpreterComparison::NotRequested,
        Some(Err(error)) if error.comparison_unavailable() => {
            InterpreterComparison::Declined(error.to_string())
        }
        Some(Err(error)) => {
            InterpreterComparison::Failed(vec![Diagnostic::error(error.to_string())])
        }
        Some(Ok(outcome)) => compare_finished_runs(
            native_exit,
            native_streams,
            outcome.status,
            ProgramStreams {
                stdout: &outcome.stdout,
                stderr: &outcome.stderr,
            },
        ),
    }
}

// Two runs agree when they finished the same way and wrote the same output.
// Output is compared only after the finish agrees, so a run that already
// diverged on its exit status or crash reports that first.
fn compare_finished_runs(
    native_exit: ProcessExitObservation,
    native_streams: ProgramStreams<'_>,
    psi_status: terminal_runtime::TerminalExecutionStatus,
    psi_streams: ProgramStreams<'_>,
) -> InterpreterComparison {
    let termination = match compare_termination(native_exit, psi_status) {
        InterpreterComparison::Agrees { exit_code } => AgreedTermination::Exit(exit_code),
        InterpreterComparison::AgreesCrash { cause } => AgreedTermination::Crash(cause),
        unresolved => return unresolved,
    };
    match first_output_difference(native_streams, psi_streams) {
        None => termination.agreement(),
        Some(divergence) => InterpreterComparison::OutputDisagrees {
            termination,
            divergence,
        },
    }
}

fn compare_termination(
    native_exit: ProcessExitObservation,
    psi_status: terminal_runtime::TerminalExecutionStatus,
) -> InterpreterComparison {
    use terminal_runtime::{TerminalExecutionResult, TerminalExecutionStatus};

    let exit_code = match psi_status {
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => 0,
        TerminalExecutionStatus::ExternalCompletion(completion) => completion.return_code,
        TerminalExecutionStatus::Complete(result) => {
            return InterpreterComparison::Declined(format!(
                "program entry returned a non-process result: {result:?}"
            ));
        }
        TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
            return InterpreterComparison::Declined(format!(
                "Psi reference execution: {exhaustion}; raise --psi-work-budget to permit more work"
            ));
        }
        TerminalExecutionStatus::AwaitingEffect(site) => {
            return InterpreterComparison::Declined(format!(
                "Psi reference execution awaits host readiness at {site:?}"
            ));
        }
        TerminalExecutionStatus::HostComputationPending(site) => {
            return InterpreterComparison::Declined(format!(
                "Psi reference execution has an unfinished host computation at {site:?}"
            ));
        }
        TerminalExecutionStatus::Crashed(crash) => {
            return match native_exit.matches_crash_presentation(crash.cause) {
                Some(true) => InterpreterComparison::AgreesCrash { cause: crash.cause },
                Some(false) => InterpreterComparison::DisagreesCrash { cause: crash.cause },
                None => InterpreterComparison::Declined(format!(
                    "Psi observed {:?}; host crash-presentation comparison is unavailable",
                    crash.cause
                )),
            };
        }
    };
    compare_exit_status(native_exit, exit_code)
}

// Unix keeps only the low byte of a process status, so a Psi status outside
// 0..=255 cannot be checked against the native one; that run is declined.
fn compare_exit_status(
    native_exit: ProcessExitObservation,
    exit_code: i32,
) -> InterpreterComparison {
    match native_exit.matches_host_presentation(exit_code) {
        Some(true) => InterpreterComparison::Agrees { exit_code },
        Some(false) => InterpreterComparison::Disagrees {
            interpreter_exit_code: exit_code,
        },
        None => InterpreterComparison::Declined(format!(
            "Psi exit {exit_code} does not fit the host's 8-bit exit status; native {} \
             shows only its low byte, so the full argument is unverified",
            native_exit.describe()
        )),
    }
}

#[cfg(test)]
mod comparison_tests {
    use super::{
        AgreedTermination, InterpreterComparison, ProcessExitObservation, ProgramStreams,
        compare_exit_status, compare_finished_runs, compare_interpretation,
    };
    use terminal_runtime::{
        TerminalExecutionResult, TerminalExecutionStatus, TerminalRuntimeError,
    };

    const NO_OUTPUT: ProgramStreams<'static> = ProgramStreams {
        stdout: b"",
        stderr: b"",
    };

    // Both runs exit 0. The twins differ only in the Psi run's stdout, which
    // stands in for an interpreter host service that writes something the
    // native program did not.
    fn finished_with_psi_stdout(psi_stdout: &[u8]) -> InterpreterComparison {
        compare_finished_runs(
            ProcessExitObservation::Exited(0),
            ProgramStreams {
                stdout: b"total 7\n",
                stderr: b"",
            },
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit),
            ProgramStreams {
                stdout: psi_stdout,
                stderr: b"",
            },
        )
    }

    #[test]
    fn equal_output_and_exit_agree() {
        let comparison = finished_with_psi_stdout(b"total 7\n");
        assert!(
            matches!(comparison, InterpreterComparison::Agrees { exit_code: 0 }),
            "{comparison:?}"
        );
    }

    #[test]
    fn different_output_with_equal_exit_disagrees() {
        let comparison = finished_with_psi_stdout(b"total 8\n");
        let InterpreterComparison::OutputDisagrees {
            termination: AgreedTermination::Exit(0),
            divergence,
        } = comparison
        else {
            panic!("equal exit statuses with different stdout must not agree: {comparison:?}");
        };
        assert_eq!(
            divergence.to_string(),
            r#"stdout line 1: native "total 7\n" vs Psi "total 8\n""#
        );
    }

    #[test]
    fn native_inspection_failure_fails_comparison() {
        let comparison = compare_interpretation(
            ProcessExitObservation::Exited(0),
            NO_OUTPUT,
            Some(Err(TerminalRuntimeError::NativeArtifactInspection(
                "corrupt instruction evidence".to_owned(),
            ))),
        );
        let InterpreterComparison::Failed(diagnostics) = comparison else {
            panic!("invalid evidence must fail even when native exits zero: {comparison:?}");
        };
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("corrupt instruction evidence")
        );
    }

    #[test]
    fn absent_host_capability_still_declines_comparison() {
        let comparison = compare_interpretation(
            ProcessExitObservation::Exited(0),
            NO_OUTPUT,
            Some(Err(TerminalRuntimeError::MissingProvision {
                capability: "runtime-provider-installation",
                reason: "absent".to_owned(),
            })),
        );
        assert!(matches!(comparison, InterpreterComparison::Declined(_)));
    }

    // Psi `exit_process(256)` and a native exit of 0 agree only in the low
    // byte Unix keeps; the comparison must not report that as agreement.
    #[test]
    fn truncated_exit_status_is_not_agreement() {
        let truncated = compare_exit_status(ProcessExitObservation::Exited(0), 256);
        if cfg!(unix) {
            assert!(
                matches!(truncated, InterpreterComparison::Declined(_)),
                "{truncated:?}"
            );
        } else {
            assert!(
                matches!(truncated, InterpreterComparison::Disagrees { .. }),
                "{truncated:?}"
            );
        }
        let exact = compare_exit_status(ProcessExitObservation::Exited(7), 7);
        assert!(
            matches!(exact, InterpreterComparison::Agrees { exit_code: 7 }),
            "{exact:?}"
        );
    }
}
