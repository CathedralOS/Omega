/*

Compile, publish, and execute an Omega project (`omega run`).

This module owns the execution pipeline behind `commands::run` without printing
to the console or exiting the process; callers decide how to present `RunOutcome`
and `CaseRunOutcome`.

We support two entry points:
- `run_project`: compiles the project into a temporary `probe` directory (removed
  on return unless `keep_artifacts` is set) and runs it according to
  `ExecutionSelection`:
  - `Native`: emits and publishes the native executable (`NativeDestination::Retained`)
    and spawns it on the host, or stops at `ExecutionOutcome::TargetOnly` when
    `target_name` names a foreign target.
  - `Terminal`: stops compilation after Build, package, trust, provider, and
    entry admission (`NativeDestination::RuntimeAdmission`) without emitting any
    object file or native executable, and runs the admitted Terminal Psi program
    directly on `terminal_runtime` with the caller's live `stdin`.
  - `Both`: runs the produced Terminal Psi artifact on `terminal_runtime`,
    publishes and spawns the native executable, verifies that any
    `private_entry_ram` observations match byte-for-byte, and compares how both
    runs terminated (`InterpreterComparison`).
- `run_project_cases`: compiles the project once on the host, replays every case
  file in `inputs` through the Terminal Psi interpreter (when
  `compare_interpreter` is true) and the native executable, and writes
  `<case>.stdout`, `<case>.stderr`, `<case>.terminal.stdout`,
  `<case>.terminal.stderr`, and `results.tsv` into `output_directory`.

Reference runs are sponsored by the CLI with `DEFAULT_PSI_WORK_ALLOWANCE`
(10,000,000 work units) unless overridden by `psi_work_budget`. Exhausting
work during `--both` declines the comparison (`InterpreterComparison::Declined`)
rather than failing the native run. When `keep_artifacts` is true, we also write
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
   Instead, we first call `interpret_product` with `stdin = None`. If the program
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

*/

mod compilation;
mod private_observation;
mod process_exit;
mod retained_artifacts;
mod standard_input;

pub use process_exit::ProcessExitObservation;

use compiler::CompileOptions;
use diagnostics::Diagnostic;
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
    pub build_snapshot: Option<compiler::BuildSnapshotRequest>,
    pub private_entry_ram: Vec<compilation_products::PrivateEntryRamInitialization>,
    pub root_path: PathBuf,
    pub target_name: Option<String>,
    pub selection: ExecutionSelection,
    pub psi_work_budget: Option<std::num::NonZeroU64>, // None selects DEFAULT_PSI_WORK_ALLOWANCE.
    pub keep_artifacts: bool,
}

pub struct RunOutcome {
    pub build_dir: PathBuf, // Removed on return unless keep_artifacts was requested.
    pub test_roster: Vec<compilation_products::TestRosterEntry>, // Registered tests that ran before the program.
    pub execution: ExecutionOutcome,
}

pub enum ExecutionOutcome {
    TargetOnly {
        target_name: String,
    },
    Host {
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
    let psi_work_budget = request
        .psi_work_budget
        .map_or(DEFAULT_PSI_WORK_ALLOWANCE, std::num::NonZeroU64::get);
    let output_directory = TemporaryDirectory::create("probe", request.keep_artifacts)
        .map_err(RunError::TemporaryStorage)?;
    let build_dir = output_directory.path().to_path_buf();
    if request.selection == ExecutionSelection::Terminal {
        return run_terminal(request, build_dir, psi_work_budget);
    }
    let compilation::ProbeCompilation {
        report,
        target_name,
        ..
    } = compilation::compile(
        request.private_entry_ram,
        request.build_snapshot,
        CompileOptions {
            root_path: request.root_path,
            build_dir: Some(build_dir.clone()),
            target_name: request.target_name.clone(),
        },
        orchestration::NativeDestination::Retained,
        Vec::new(),
        psi_work_budget,
    )
    .map_err(RunError::Compilation)?;
    let test_roster = report.test_roster().to_vec();
    let (replay_input, interpretation) =
        if request.selection == ExecutionSelection::Both && request.target_name.is_none() {
            match compilation::interpret_product(&report, None, target_name, psi_work_budget) {
                Err(terminal_runtime::TerminalRuntimeError::MissingProvision {
                    capability: "runtime-standard-input",
                    ..
                }) => {
                    let captured = standard_input::ReplayInput::capture(std::io::stdin().lock())
                        .map_err(RunError::StandardInput)?;
                    let input = captured.open_input().map_err(RunError::StandardInput)?;
                    let retry = compilation::interpret_product(
                        &report,
                        Some(Box::new(input)),
                        target_name,
                        psi_work_budget,
                    );
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
    let (report, executable) =
        orchestration::publication::publish_native_artifact(report, &build_dir)
            .map_err(RunError::Publication)?;
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
                retain_terminal_observation(&build_dir, reference, psi_work_budget)?;
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
        build_dir,
        test_roster,
        execution,
    })
}

fn run_terminal(
    request: RunRequest,
    build_dir: PathBuf,
    psi_work_budget: u64,
) -> Result<RunOutcome, RunError> {
    let compilation::ProbeCompilation {
        report,
        target_name,
        ..
    } = compilation::compile(
        request.private_entry_ram,
        request.build_snapshot,
        CompileOptions {
            root_path: request.root_path,
            build_dir: Some(build_dir.clone()),
            target_name: request.target_name,
        },
        orchestration::NativeDestination::RuntimeAdmission,
        Vec::new(),
        psi_work_budget,
    )
    .map_err(RunError::Compilation)?;
    if request.keep_artifacts {
        retained_artifacts::retain(&report, &build_dir)?;
    }
    let test_roster = report.test_roster().to_vec();
    let outcome = match compilation::interpret_product(
        &report,
        Some(Box::new(std::io::stdin())),
        target_name,
        psi_work_budget,
    ) {
        Ok(observation) => {
            if request.keep_artifacts {
                retain_terminal_observation(&build_dir, &observation, psi_work_budget)?;
            }
            TerminalOutcome::Completed(observation)
        }
        Err(error) if error.unavailable_before_guest_effects() => {
            TerminalOutcome::Unavailable(error.to_string())
        }
        Err(error) => TerminalOutcome::Failed(error.to_string()),
    };
    Ok(RunOutcome {
        build_dir,
        test_roster,
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

pub struct CaseRunRequest {
    pub build_snapshot: Option<compiler::BuildSnapshotRequest>,
    pub private_entry_ram: Vec<compilation_products::PrivateEntryRamInitialization>,
    pub root_path: PathBuf,
    pub compare_interpreter: bool,
    pub psi_work_budget: Option<std::num::NonZeroU64>,
    pub inputs: Vec<PathBuf>, // Run in order; each case is named by its file stem.
    pub output_directory: PathBuf,
    pub keep_artifacts: bool,
}

pub struct CaseOutcome {
    pub name: String,
    pub exit: ProcessExitObservation,
    pub comparison: InterpreterComparison,
}

pub struct CaseRunOutcome {
    pub build_dir: PathBuf,
    pub cases: Vec<CaseOutcome>,
}

pub fn run_project_cases(request: CaseRunRequest) -> Result<CaseRunOutcome, RunError> {
    let psi_work_budget = request
        .psi_work_budget
        .map_or(DEFAULT_PSI_WORK_ALLOWANCE, std::num::NonZeroU64::get);
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
        ..
    } = compilation::compile(
        request.private_entry_ram,
        request.build_snapshot,
        CompileOptions {
            root_path: request.root_path,
            build_dir: Some(build_dir.clone()),
            target_name: None,
        },
        orchestration::NativeDestination::Retained,
        reference_inputs,
        psi_work_budget,
    )
    .map_err(RunError::Compilation)?;
    if request.keep_artifacts {
        retained_artifacts::retain(&report, &build_dir)?;
    }
    let (report, executable) =
        orchestration::publication::publish_native_artifact(report, &build_dir)
            .map_err(RunError::Publication)?;
    let mut interpretations = interpretations.into_iter();
    let mut cases = Vec::new();
    let mut table = String::from("case\tnative\tinterpreter\tinterpreter_exit\n");
    for input in &request.inputs {
        let name = input
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        report
            .validate_component_process_inputs(&executable)
            .map_err(RunError::Publication)?;
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
            write(
                "psi.work.tsv",
                format_work_tsv(reference, psi_work_budget).as_bytes(),
            )?;
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
    Ok(CaseRunOutcome { build_dir, cases })
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
            terminal_runtime::TerminalRuntimeObservation,
            terminal_runtime::TerminalRuntimeError,
        >,
    >,
) -> InterpreterComparison {
    use terminal_runtime::{TerminalExecutionResult, TerminalExecutionStatus};

    match interpretation {
        None => InterpreterComparison::NotRequested,
        Some(Err(error)) if error.comparison_unavailable() => {
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
                TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
                    return InterpreterComparison::Declined(format!(
                        "Psi reference execution: {exhaustion}; raise --psi-work-budget to permit more work"
                    ));
                }
                TerminalExecutionStatus::AwaitingEffect(site) => {
                    return InterpreterComparison::Declined(format!(
                        "Terminal reference execution awaits host readiness at {site:?}"
                    ));
                }
                TerminalExecutionStatus::HostComputationPending(site) => {
                    return InterpreterComparison::Declined(format!(
                        "Terminal reference execution has an unfinished host computation at {site:?}"
                    ));
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

#[cfg(test)]
mod comparison_tests {
    use super::{InterpreterComparison, ProcessExitObservation, compare_interpretation};
    use terminal_runtime::TerminalRuntimeError;

    #[test]
    fn native_inspection_failure_fails_comparison() {
        let comparison = compare_interpretation(
            ProcessExitObservation::Exited(0),
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
            Some(Err(TerminalRuntimeError::MissingProvision {
                capability: "runtime-provider-installation",
                reason: "absent".to_owned(),
            })),
        );
        assert!(matches!(comparison, InterpreterComparison::Declined(_)));
    }
}
