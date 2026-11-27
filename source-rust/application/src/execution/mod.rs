//! Prepare and interpret a source or saved Psi program.
mod cases;
mod compilation;
mod psi;
mod retained_artifacts;
mod termination;

pub use cases::{CaseRunRequest, run_project_cases};
pub(crate) use termination::PsiTermination;

use diagnostics::Diagnostic;
use package_compilation::build_execution::BuildSnapshotRequest;
use platform_custody::temporary_directory::TemporaryDirectory;
use std::path::PathBuf;
const DEFAULT_PSI_WORK_ALLOWANCE: u64 = 10_000_000;

pub struct RunRequest {
    pub run_directories: platform_custody::directory_mappings::DirectoryMappings,
    pub build_snapshot: Option<BuildSnapshotRequest>,
    pub private_entry_ram: Vec<psi_entry_memory::PrivateEntryRamInitialization>,
    pub root_path: PathBuf,
    pub target_name: Option<String>,
    pub psi_work_budget: Option<std::num::NonZeroU64>, // None selects DEFAULT_PSI_WORK_ALLOWANCE.
    pub keep_artifacts: bool,
}

pub struct RunOutcome {
    pub build_dir: PathBuf, // Removed on return unless keep_artifacts was requested.
    pub execution: TerminalOutcome,
}

pub enum TerminalOutcome {
    Completed(terminal_runtime::TerminalRuntimeObservation),
    Unavailable(String),
    Failed(String),
}

#[derive(Debug)]
pub enum RunError {
    TemporaryStorage(std::io::Error),
    StandardInput(std::io::Error),
    ReferenceOutput(std::io::Error),
    Compilation(Vec<Diagnostic>),
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
        }
    }
}

impl std::error::Error for RunError {}

/// Admit the selected product, present its registered plan and run its tests
/// before interpreting the program.
fn prepare_program(
    artifact: Option<psi_authorization::AuthorizedTerminalArtifact>,
    input: compilation::ProjectInput,
    mut present_test: impl FnMut(&str),
) -> Result<compilation::RunProduct, RunError> {
    let prepared = compilation::compile(artifact, input).map_err(RunError::Compilation)?;
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
            target_name: request.target_name,
        },
        present_test,
    )?;
    if request.keep_artifacts {
        retained_artifacts::retain(&report, &build_dir)?;
    }
    let outcome = TerminalOutcome::from(psi::execute(
        &report,
        Some(Box::new(std::io::stdin())),
        psi_work_budget,
        request.run_directories,
    ));
    if request.keep_artifacts
        && let TerminalOutcome::Completed(observation) = &outcome
    {
        retain_terminal_observation(&build_dir, observation, psi_work_budget)?;
    }
    Ok(RunOutcome {
        build_dir,
        execution: outcome,
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
