//! Compile a batch once, then run each named input with the selected engine.
use super::{
    AgreedTermination, DEFAULT_PSI_WORK_ALLOWANCE, ExecutionSelection, InterpreterComparison,
    ProcessExitObservation, ProgramStreams, PsiTermination, RunError, TerminalOutcome,
    compare_interpretation, compilation, format_work_tsv, private_observation, psi,
    retained_artifacts,
};
use crate::saved_program::ExecutionProduct;
use package_compilation::build_execution::BuildSnapshotRequest;
use platform_custody::temporary_directory::TemporaryDirectory;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct CaseRunRequest {
    pub build_snapshot: Option<BuildSnapshotRequest>,
    pub private_entry_ram: Vec<psi_entry_memory::PrivateEntryRamInitialization>,
    pub root_path: PathBuf,
    pub target_name: Option<String>,
    pub selection: ExecutionSelection,
    pub psi_work_budget: Option<std::num::NonZeroU64>,
    pub inputs: Vec<PathBuf>, // Run in order; each case is named by its file stem.
    pub output_directory: PathBuf,
    pub keep_artifacts: bool,
}

pub enum CaseOutcome {
    Native {
        name: String,
        exit: ProcessExitObservation,
        comparison: InterpreterComparison,
    },
    Psi {
        name: String,
        termination: PsiTermination,
    },
}

pub struct CaseRunOutcome {
    pub build_dir: PathBuf,
    pub cases: Vec<CaseOutcome>,
}

struct InputCase {
    path: PathBuf,
    name: String,
}

impl InputCase {
    fn open(&self) -> Result<std::fs::File, RunError> {
        std::fs::File::open(&self.path).map_err(RunError::StandardInput)
    }

    fn output_path(&self, directory: &Path, suffix: &str) -> PathBuf {
        directory.join(format!("{}.{suffix}", self.name))
    }

    fn write_observation(&self, directory: &Path, suffix: &str, bytes: &[u8]) -> Result<(), RunError> {
        use std::io::Write;
        let path = self.output_path(directory, suffix);
        // Distinct stems can refer to the same file on a case-insensitive volume.
        // Preserve the first case's evidence and withhold the batch result if so.
        std::fs::File::create_new(&path)
            .and_then(|mut file| file.write_all(bytes))
            .map_err(|error| RunError::ReferenceOutput(std::io::Error::new(
                error.kind(),
                format!("{}: {error}", path.display()),
            )))
    }
}

pub fn run_project_cases(
    artifact: Option<psi_authorization::AuthorizedTerminalArtifact>,
    request: CaseRunRequest,
) -> Result<CaseRunOutcome, RunError> {
    let budget = request
        .psi_work_budget
        .map_or(DEFAULT_PSI_WORK_ALLOWANCE, std::num::NonZeroU64::get);
    let scratch = TemporaryDirectory::create("probe", request.keep_artifacts)
        .map_err(RunError::TemporaryStorage)?;
    let build_dir = scratch.path().to_path_buf();
    let directory = &request.output_directory;
    let mut names = std::collections::BTreeSet::new();
    let mut inputs = Vec::with_capacity(request.inputs.len());
    for path in request.inputs {
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.is_empty() || name.contains(['\t', '\r', '\n']) || !names.insert(name.clone()) {
            return Err(RunError::StandardInput(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "case {} needs a distinct output name that fits one results.tsv field",
                    path.display()
                ),
            )));
        }
        inputs.push(InputCase { path, name });
    }
    let product = match request.selection {
        ExecutionSelection::Terminal => ExecutionProduct::Runtime,
        ExecutionSelection::Both | ExecutionSelection::Native => ExecutionProduct::Native,
    };
    let compilation::ProbeCompilation {
        report,
        target_name,
    } = compilation::compile(
        artifact,
        compilation::ProjectInput {
            root_path: request.root_path,
            build_directory: build_dir.clone(),
            private_entry_ram: request.private_entry_ram,
            build_snapshot: request.build_snapshot,
            target_name: request.target_name,
        },
        product,
    )
    .map_err(RunError::Compilation)?;
    if request.keep_artifacts {
        retained_artifacts::retain(&report, &build_dir)?;
    }
    let (cases, table) = match request.selection {
        ExecutionSelection::Terminal => {
            // Single-engine observations have a fresh destination. A failed
            // guest cannot inherit streams from an earlier invocation.
            std::fs::create_dir(directory).map_err(RunError::ReferenceOutput)?;
            psi_cases(&inputs, directory, &report, target_name, budget)?
        }
        selection => {
            std::fs::create_dir_all(directory).map_err(RunError::ReferenceOutput)?;
            // Publishing consumes the retained Psi. Interpret each requested
            // case while the same compiled product still owns its artifact.
            let mut references = Vec::new();
            if selection == ExecutionSelection::Both {
                for input in &inputs {
                    references.push(psi::execute(
                        &report,
                        Some(Box::new(input.open()?)),
                        target_name,
                        budget,
                    ));
                }
            }
            let (report, executable) = report.publish(&build_dir).map_err(RunError::Publication)?;
            native_cases(
                &inputs,
                directory,
                &report,
                &executable,
                references.into_iter(),
                budget,
            )?
        }
    };
    std::fs::write(directory.join("results.tsv"), table).map_err(RunError::ReferenceOutput)?;
    Ok(CaseRunOutcome { build_dir, cases })
}

fn psi_cases(
    inputs: &[InputCase],
    directory: &Path,
    report: &compilation::RunProduct,
    target: &str,
    budget: u64,
) -> Result<(Vec<CaseOutcome>, String), RunError> {
    let mut cases = Vec::with_capacity(inputs.len());
    let mut table = String::from("case\ttermination\tdetail\n");
    for input in inputs {
        let outcome = TerminalOutcome::from(psi::execute(
            report,
            Some(Box::new(input.open()?)),
            target,
            budget,
        ));
        if let TerminalOutcome::Completed(observation) = &outcome {
            input.write_observation(directory, "stdout", &observation.stdout)?;
            input.write_observation(directory, "stderr", &observation.stderr)?;
            input.write_observation(
                directory,
                "psi.work.tsv",
                format_work_tsv(observation, budget).as_bytes(),
            )?;
            for (position, contents) in &observation.private_entry_ram {
                input.write_observation(directory, &format!("entry-ram.{position}.bin"), contents)?;
            }
        }
        let termination = PsiTermination::of(&outcome);
        let (kind, detail) = termination.columns();
        writeln!(
            table,
            "{}\t{kind}\t{}",
            input.name,
            detail.replace(['\t', '\r', '\n'], " ")
        )
        .unwrap();
        cases.push(CaseOutcome::Psi {
            name: input.name.clone(),
            termination,
        });
    }
    Ok((cases, table))
}

fn native_cases(
    inputs: &[InputCase],
    directory: &Path,
    report: &compilation::RunProduct,
    executable: &Path,
    mut interpretations: impl Iterator<
        Item = Result<
            terminal_runtime::TerminalRuntimeObservation,
            terminal_runtime::TerminalRuntimeError,
        >,
    >,
    psi_work_budget: u64,
) -> Result<(Vec<CaseOutcome>, String), RunError> {
    let mut cases = Vec::with_capacity(inputs.len());
    let mut table = String::from("case\tnative\tinterpreter\tinterpreter_exit\n");
    for input in inputs {
        let name = &input.name;
        report
            .validate_component_process_inputs(executable)
            .map_err(RunError::Publication)?;
        let mut command = Command::new(executable);
        command.stdin(Stdio::from(input.open()?));
        let (output, private_entry_ram, private_frames) =
            private_observation::capture(command, report.private_entry_ram()).map_err(|error| {
                RunError::Spawn {
                    executable: executable.to_owned(),
                    error,
                }
            })?;
        let write = |suffix: &str, bytes: &[u8]| {
            std::fs::write(input.output_path(directory, suffix), bytes).map_err(RunError::ReferenceOutput)
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
        let comparison = compare_interpretation(
            exit,
            ProgramStreams {
                stdout: &output.stdout,
                stderr: &output.stderr,
            },
            interpretation,
        );
        let (verdict, interpreter_exit) = comparison_columns(&comparison);
        table.push_str(&format!(
            "{name}\t{}\t{verdict}\t{interpreter_exit}\n",
            exit.code()
                .map_or_else(|| exit.describe(), |code| code.to_string())
        ));
        cases.push(CaseOutcome::Native {
            name: name.clone(),
            exit,
            comparison,
        });
    }
    Ok((cases, table))
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
        InterpreterComparison::OutputDisagrees { termination, .. } => (
            "disagrees-output",
            match termination {
                AgreedTermination::Exit(exit_code) => exit_code.to_string(),
                AgreedTermination::Crash(cause) => format!("{cause:?}"),
            },
        ),
        InterpreterComparison::Failed(_) => ("failed", String::new()),
    }
}
