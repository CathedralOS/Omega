//! Archived project-run adapter for the retired orchestration request and report contracts.

use super::{ProjectInput, RunProduct};
use crate::saved_program::ExecutionProduct;
use diagnostics::Diagnostic;
use orchestration::{BuildError, BuildRequest, BuildTargets, NativeDestination, ProjectProduct};

fn run_project_input(
    input: ProjectInput,
    requested: ExecutionProduct,
) -> Result<(RunProduct, &'static str), Vec<Diagnostic>> {
    let ProjectInput {
        root_path,
        build_directory,
        target_name,
        private_entry_ram,
        build_snapshot,
    } = input;
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
    Ok((RunProduct::Project(report), target_name))
}
