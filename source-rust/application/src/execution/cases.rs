//! Compile a batch once, then run each named input with the selected engine.
use super::{
    DEFAULT_PSI_WORK_ALLOWANCE, ExecutionSelection, ProcessExitObservation, PsiTermination,
    RunError, TerminalOutcome, compilation, format_work_tsv, private_observation, psi,
    retained_artifacts,
};
use package_compilation::build_execution::BuildSnapshotRequest;
use platform_custody::temporary_directory::TemporaryDirectory;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub struct CaseRunRequest {
    pub run_directories: platform_custody::directory_mappings::DirectoryMappings,
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

    fn write_observation(
        &self,
        directory: &Path,
        suffix: &str,
        bytes: &[u8],
    ) -> Result<(), RunError> {
        use std::io::Write;
        let path = self.output_path(directory, suffix);
        // Distinct stems can refer to the same file on a case-insensitive volume.
        // Preserve the first case's evidence and withhold the batch result if so.
        std::fs::File::create_new(&path)
            .and_then(|mut file| file.write_all(bytes))
            .map_err(|error| {
                RunError::ReferenceOutput(std::io::Error::new(
                    error.kind(),
                    format!("{}: {error}", path.display()),
                ))
            })
    }
}

pub fn run_project_cases(
    artifact: Option<psi_authorization::AuthorizedTerminalArtifact>,
    request: CaseRunRequest,
    present_test: impl FnMut(&str),
) -> Result<CaseRunOutcome, RunError> {
    if request.selection != ExecutionSelection::Terminal && !request.run_directories.is_empty() {
        return Err(RunError::InvalidRequest(
            "Product directory mappings require the Psi engine",
        ));
    }
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
    let report = super::prepare_program(
        artifact,
        compilation::ProjectInput {
            root_path: request.root_path,
            build_directory: build_dir.clone(),
            private_entry_ram: request.private_entry_ram,
            build_snapshot: request.build_snapshot,
            target_name: request.target_name,
        },
        request.selection,
        present_test,
    )?;
    if request.keep_artifacts {
        retained_artifacts::retain(&report, &build_dir)?;
    }
    std::fs::create_dir(directory).map_err(RunError::ReferenceOutput)?;
    let (cases, table) = match request.selection {
        ExecutionSelection::Terminal => psi_cases(
            &inputs,
            directory,
            &report,
            budget,
            &request.run_directories,
        )?,
        ExecutionSelection::Native => {
            let (report, executable) = report.publish(&build_dir).map_err(RunError::Publication)?;
            native_cases(&inputs, directory, &report, &executable)?
        }
    };
    std::fs::write(directory.join("results.tsv"), table).map_err(RunError::ReferenceOutput)?;
    Ok(CaseRunOutcome { build_dir, cases })
}

fn psi_cases(
    inputs: &[InputCase],
    directory: &Path,
    report: &compilation::RunProduct,
    budget: u64,
    directories: &platform_custody::directory_mappings::DirectoryMappings,
) -> Result<(Vec<CaseOutcome>, String), RunError> {
    let mut cases = Vec::with_capacity(inputs.len());
    let mut table = String::from("case\ttermination\tdetail\n");
    for input in inputs {
        let outcome = TerminalOutcome::from(psi::execute(
            report,
            Some(Box::new(input.open()?)),
            budget,
            directories.clone(),
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
                input.write_observation(
                    directory,
                    &format!("entry-ram.{position}.bin"),
                    contents,
                )?;
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
) -> Result<(Vec<CaseOutcome>, String), RunError> {
    let mut cases = Vec::with_capacity(inputs.len());
    let mut table = String::from("case\tnative\n");
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
        input.write_observation(directory, "stdout", &output.stdout)?;
        input.write_observation(directory, "stderr", &output.stderr)?;
        if !private_frames.is_empty() {
            input.write_observation(directory, "native.entry-ram.frames", &private_frames)?;
        }
        for (position, contents) in &private_entry_ram {
            input.write_observation(
                directory,
                &format!("native.entry-ram.{position}.bin"),
                contents,
            )?;
        }
        let exit = ProcessExitObservation::observe(&output.status);
        writeln!(
            table,
            "{name}\t{}",
            exit.code()
                .map_or_else(|| exit.describe(), |code| code.to_string())
        )
        .unwrap();
        cases.push(CaseOutcome::Native {
            name: name.clone(),
            exit,
        });
    }
    Ok((cases, table))
}
