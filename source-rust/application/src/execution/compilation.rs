//! Prepare one target-bound product from source or a trusted saved program.
use crate::saved_program::{ExecutionProduct, SavedProgramError};
use diagnostics::Diagnostic;
use package_compilation::build_execution::BuildSnapshotRequest;
use product_publication::{CompletedProduct, RetainedNativeArtifact, TerminalRuntimeProvisions};
use std::path::PathBuf;

pub(super) struct RunProduct {
    product: CompletedProduct,
    pub(super) target: target::TargetProfile,
}

pub(super) struct RunArtifact<'product> {
    pub artifact: &'product terminal_codec::CanonicalTerminalArtifact,
    pub image: Option<&'product RetainedNativeArtifact>,
}

impl RunProduct {
    pub(super) fn artifact(&self) -> Option<RunArtifact<'_>> {
        let native = self.product.retained_native_program();
        let portable = self.product.artifact();
        let image = self.product.retained_native_artifact();
        Some(RunArtifact {
            artifact: native.as_ref().map(|p| p.psi_artifact()).or(portable)?,
            image,
        })
    }

    pub(super) fn runtime(&self) -> Option<&TerminalRuntimeProvisions> {
        self.product.runtime_provisions()
    }

    /// The contained admission of saved Psi the host does not trust.
    pub(super) fn contained(&self) -> Option<&psi_authorization::ContainedTerminalArtifact> {
        self.product.contained_artifact()
    }
}

pub(super) struct ProjectInput {
    pub root_path: PathBuf,
    pub build_directory: PathBuf,
    pub target_name: Option<String>,
    pub private_entry_ram: Vec<psi_entry_memory::PrivateEntryRamInitialization>,
    pub build_snapshot: Option<BuildSnapshotRequest>,
}

pub(super) struct CompiledRun {
    pub product: RunProduct,
    pub tests: Option<project_build::RegisteredTestPlan>,
}

pub(super) fn compile(
    artifact: Option<psi_authorization::AdmittedTerminalArtifact>,
    input: ProjectInput,
) -> Result<CompiledRun, Vec<Diagnostic>> {
    let ProjectInput {
        root_path,
        build_directory,
        target_name,
        private_entry_ram,
        build_snapshot,
    } = input;
    let (product, target, tests) = if let Some(artifact) = artifact {
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
            &private_entry_ram,
            ExecutionProduct::Runtime,
        )
        .map_err(|error| match error {
            SavedProgramError::Diagnostics(diagnostics) => diagnostics,
            other => vec![Diagnostic::error(other.to_string())],
        })?;
        (product, profile, None)
    } else {
        if root_path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("psi"))
        {
            return Err(vec![Diagnostic::error(
                "execution of a saved Psi program requires producer trust",
            )]);
        }
        let profile = target::TargetProfile::from_omega_target_name(target_name.as_deref())
            .map_err(|error| vec![error])?;
        let planned = package_manager::project::open_product(
            &root_path,
            profile,
            false,
            &build_directory,
            build_snapshot,
        )?;
        let stage = planned.product_stage(&[profile])?;
        let program = stage.program(profile, Vec::new())?;
        let request = compiler::CompileRequest::Runtime {
            program,
            entry_memory: &private_entry_ram,
        };
        let mut declaration_usage = project_build::DeclarationUsageAdvisories::default();
        planned.collect_declaration_usage(&mut declaration_usage);
        let product = match compiler::compile(
            request,
            compiler::CompileOptions {
                declaration_usage: Some(&mut declaration_usage),
                ..Default::default()
            },
        )
        .map_err(|failure| {
            let mut rows = failure.into_diagnostics();
            planned.attribute_diagnostics(&mut rows);
            rows
        })?
        .artifact
        {
            compiler::Artifact::Runtime(product) => product,
            _ => {
                return Err(vec![Diagnostic::error(
                    "run request did not produce a runtime product",
                )]);
            }
        };
        let plan =
            stage.plan_registered_tests(profile, &[], project_build::TestExecution::Interpret)?;
        let mut file_imports = project_build::FileImportAdvisories::default();
        planned.collect_file_imports(&mut file_imports);
        crate::commands::report_unused_imports(&file_imports);
        crate::commands::report_unused_declarations(&declaration_usage);
        (product, profile, Some(plan))
    };
    Ok(CompiledRun {
        product: RunProduct { product, target },
        tests,
    })
}
