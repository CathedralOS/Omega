//! Prepare a saved or source program for the selected execution engine, then run
//! the same target-bound Psi artifact with its published provisions.
use crate::saved_program::{ExecutionProduct, SavedProgramError};
use compiler::CompileReport;
use diagnostics::Diagnostic;
use orchestration::{BuildError, BuildRequest, BuildTargets, NativeDestination, ProjectProduct};
use package_compilation::build_execution::BuildSnapshotRequest;
use product_publication::{
    CompletedProduct, RetainedNativeArtifact, RetainedNativeProgramIdentity,
    TerminalRuntimeProvisions,
};
use std::path::{Path, PathBuf};
use terminal_runtime::{
    TerminalDomainExitDelegation, TerminalRuntimeError, TerminalRuntimeObservation,
    TerminalRuntimeRequest,
};

pub(super) enum RunProduct {
    Saved {
        root: PathBuf,
        product: CompletedProduct,
    },
    Project(CompileReport),
}

pub(super) struct RunArtifact<'product> {
    pub artifact: &'product terminal_codec::CanonicalTerminalArtifact,
    pub native_identity: Option<RetainedNativeProgramIdentity>,
    pub image: Option<&'product RetainedNativeArtifact>,
}

impl RunProduct {
    pub(super) fn artifact(&self) -> Option<RunArtifact<'_>> {
        let (native, portable, image) = match self {
            Self::Project(report) => (
                report.retained_native_program(),
                report.artifact(),
                report.retained_native_artifact(),
            ),
            Self::Saved { product, .. } => (
                product.retained_native_program(),
                product.artifact(),
                product.retained_native_artifact(),
            ),
        };
        Some(RunArtifact {
            artifact: native.as_ref().map(|p| p.psi_artifact()).or(portable)?,
            native_identity: native.map(|p| p.identity()),
            image,
        })
    }

    fn runtime(&self) -> Option<&TerminalRuntimeProvisions> {
        match self {
            Self::Saved { product, .. } => product.runtime_provisions(),
            Self::Project(report) => report.published_runtime_provisions(),
        }
    }

    pub(super) fn private_entry_ram(&self) -> &[psi_entry_memory::CheckedPrivateEntryRamRecipe] {
        self.runtime()
            .map(|provisions| provisions.execution().private_entry_ram())
            .unwrap_or_default()
    }

    pub(super) fn test_roster(&self) -> &[products::TestRosterEntry] {
        match self {
            Self::Saved { .. } => &[],
            Self::Project(report) => report.test_roster(),
        }
    }

    pub(super) fn publish(self, directory: &Path) -> Result<(Self, PathBuf), String> {
        match self {
            Self::Saved { root, product } => {
                let published = product.publish(&root, directory)?;
                let path = published
                    .checked_native_executable_path()
                    .ok_or("native publication did not retain executable custody")?
                    .to_owned();
                Ok((
                    Self::Saved {
                        root,
                        product: published,
                    },
                    path,
                ))
            }
            Self::Project(report) => {
                let (published, path) =
                    orchestration::publication::publish_native_artifact(report, directory)?;
                Ok((Self::Project(published), path))
            }
        }
    }

    pub(super) fn validate_component_process_inputs(
        &self,
        executable: &Path,
    ) -> Result<(), String> {
        match self {
            Self::Saved { product, .. } => product.validate_component_process_inputs(executable),
            Self::Project(report) => report.validate_component_process_inputs(executable),
        }
    }
}

pub(super) struct ProjectInput {
    pub root_path: PathBuf,
    pub build_directory: PathBuf,
    pub target_name: Option<String>,
    pub private_entry_ram: Vec<psi_entry_memory::PrivateEntryRamInitialization>,
    pub build_snapshot: Option<BuildSnapshotRequest>,
}

pub(super) struct ProbeCompilation {
    pub report: RunProduct,
    pub target_name: &'static str,
    pub interpretations: Vec<Result<TerminalRuntimeObservation, TerminalRuntimeError>>, // One reference run per supplied input, in input order.
}

pub(super) fn compile(
    artifact: Option<psi_authorization::AuthorizedTerminalArtifact>,
    input: ProjectInput,
    requested: ExecutionProduct,
    reference_inputs: Vec<std::fs::File>,
    psi_work_budget: u64,
) -> Result<ProbeCompilation, Vec<Diagnostic>> {
    let ProjectInput {
        root_path,
        build_directory,
        target_name,
        private_entry_ram,
        build_snapshot,
    } = input;
    let (report, target_name) = if let Some(artifact) = artifact {
        if build_snapshot.is_some() {
            return Err(vec![Diagnostic::error(
                "a saved Psi program cannot accept source build inputs",
            )]);
        }
        let profile = target::TargetProfile::from_omega_target_name(target_name.as_deref())
            .map_err(|error| vec![error])?;
        let host = crate::saved_program::configured_host_packages()?;
        let product = crate::saved_program::prepare(
            artifact,
            profile,
            host.as_ref(),
            &Default::default(),
            &private_entry_ram,
            requested,
        )
        .map_err(|error| match error {
            SavedProgramError::Diagnostics(diagnostics) => diagnostics,
            other => vec![Diagnostic::error(other.to_string())],
        })?;
        (
            RunProduct::Saved {
                root: root_path,
                product,
            },
            profile.target_name(),
        )
    } else {
        if root_path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("psi"))
        {
            return Err(vec![Diagnostic::error(
                "execution of a saved Psi program requires producer trust",
            )]);
        }
        let targets = match target_name.as_deref() {
            Some(name) => BuildTargets::Single(
                target::TargetProfile::from_omega_target_name(Some(name))
                    .map_err(|diagnostic| vec![diagnostic])?,
            ),
            None => BuildTargets::Host,
        };
        let outcomes = orchestration::build(BuildRequest {
            stage_validators: Default::default(),
            root_path,
            build_directory: Some(build_directory),
            targets,
            product: ProjectProduct::Native {
                installation_assumptions: Vec::new(),
                destination: match requested {
                    ExecutionProduct::Native => NativeDestination::Retained,
                    ExecutionProduct::Runtime => NativeDestination::RuntimeAdmission,
                },
                private_entry_ram,
            },
            timings: false,
            offline: false,
            verify: false,
            require_package_project: false,
            optimization_rollback: Default::default(),
            build_snapshot,
        })
        .map_err(BuildError::into_diagnostics)?;
        let [outcome]: [_; 1] = outcomes.try_into().map_err(|_| {
            vec![Diagnostic::error(
                "execution requires exactly one project Build outcome",
            )]
        })?;
        let target_name = outcome.target.target_name();
        let outcome = outcome.result.map_err(BuildError::into_diagnostics)?;
        let report = outcome
            .into_report()
            .map_err(BuildError::into_diagnostics)?
            .report;
        (RunProduct::Project(report), target_name)
    };
    let interpretations = reference_inputs
        .into_iter()
        .map(|input| {
            interpret_product(&report, Some(Box::new(input)), target_name, psi_work_budget)
        })
        .collect();
    Ok(ProbeCompilation {
        report,
        target_name,
        interpretations,
    })
}

pub(super) fn interpret_product(
    report: &RunProduct,
    input: Option<Box<dyn std::io::Read + Send>>,
    target: &str,
    psi_work_budget: u64,
) -> Result<TerminalRuntimeObservation, TerminalRuntimeError> {
    let program = report
        .artifact()
        .ok_or(TerminalRuntimeError::InvalidProvision(
            "run lost its produced Terminal artifact before execution",
        ))?;
    let runtime = report.runtime();
    project_build::published_terminal::execute_published_terminal(
        TerminalRuntimeRequest {
            entry: None,
            filesystem: terminal_runtime::TerminalFilesystemDelegation::HostNamespace,
            native_services: terminal_runtime::TerminalNativeServiceDelegation::MatchingHost,
            native_provider: Some(native_provider_sponsor::for_adapters(
                std::process::Command::new(std::env::current_exe().map_err(|error| {
                    TerminalRuntimeError::MissingProvision {
                        capability: "native-provider-sponsor",
                        reason: error.to_string(),
                    }
                })?),
                runtime
                    .into_iter()
                    .flat_map(|runtime| runtime.published_native_adapters()),
            )),
            x86_flags: terminal_runtime::capture_host_x86_flags(target)?,
            memory_ordering:
                terminal_runtime::TerminalMemoryOrderingEnvironment::private_sequential(target),
            provider_proposal: None,
            artifact: program.artifact,
            target: Some(target),
            provisions: runtime.map(|provisions| provisions.execution()),
            work_budget: psi_work_budget,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: input,
            clock: Some(terminal_runtime::TerminalClockEnvironment::Host),
        },
        program.image,
    )
}
