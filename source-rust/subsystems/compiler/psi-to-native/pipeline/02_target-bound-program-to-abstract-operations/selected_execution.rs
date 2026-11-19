//! Lower the selected root or callable to abstract operations.
use diagnostics::Diagnostic;
use verified_optimization_input::{PreparedNativeExecution, TerminalNativeRealizationProposal};

pub fn lower_native_callable(
    execution: &PreparedNativeExecution,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    proposal: &TerminalNativeRealizationProposal,
    entry: terminal_psi::TargetMachineCallable,
    scalar_applications: Option<crate::NativeScalarApplications<'_>>,
) -> Result<
    (
        crate::VerifiedNativeArtifactInput,
        boundary_applications::TerminalBoundaryApplicationCoverage,
    ),
    Vec<Diagnostic>,
> {
    execution
        .authorization()
        .require_artifact(artifact)
        .map_err(|error| vec![Diagnostic::error(error.to_string())])?;
    let checked_scope = proposal.callable_execution_scope(artifact, entry, Some(execution))?;
    let selection = crate::NativeExecutionSelection {
        scope: Some(&checked_scope),
        scalar_applications,
        providers: checked_scope.local_adapters(),
        plans: &proposal.providers().selection,
        callbacks: checked_scope.callbacks(),
        entry_call: checked_scope.entry_call(),
    };
    let scope = crate::selected_input::NativeScopePreparation::prepare(
        execution.authorization(),
        proposal.target_profile(),
        entry,
        &selection,
    )
    .map_err(crate::admit_providers::artifact_lowering_error)?;
    let provenance = proposal.selected_intrinsic_provenance(execution.selection())?;
    let coverage = proposal.settle_scoped_boundary_applications(
        artifact,
        &checked_scope,
        execution.selection(),
        &provenance,
    )?;
    let input = scope
        .lower(
            artifact,
            &[],
            &selection,
            execution.evidence().map(std::sync::Arc::as_ref),
        )
        .map_err(crate::admit_providers::artifact_lowering_error)?;
    let input = input
        .bind_selected_provider_coordinates(artifact, execution.selection())
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    Ok((input, coverage))
}

/// Lower one prepared native execution to the abstract program in a single
/// pass. The execution scope, its provider demand and the scalar values it
/// reads are all settled before Omega 00 starts, so this stage runs once,
/// front to back, with no caller work between preparing and lowering.
pub fn lower_native_execution(
    proposal: &TerminalNativeRealizationProposal,
    execution: &PreparedNativeExecution,
    private_entry_ram: &[psi_entry_memory::CheckedPrivateEntryRamRecipe],
    scalar_applications: Option<crate::NativeScalarApplications<'_>>,
) -> Result<crate::VerifiedNativeArtifactInput, Vec<Diagnostic>> {
    let authorization = execution.authorization();
    let artifact = authorization.artifact();
    let checked = execution.scope();
    let scope = crate::selected_input::NativeScopePreparation::prepare(
        authorization,
        proposal.target_profile(),
        terminal_psi::TargetMachineCallable::Machine(checked.entry()),
        &crate::NativeExecutionSelection {
            scope: Some(checked),
            scalar_applications: None,
            providers: checked.local_adapters(),
            plans: &proposal.providers().selection,
            callbacks: checked.callbacks(),
            entry_call: checked.entry_call(),
        },
    )
    .map_err(crate::admit_providers::artifact_lowering_error)?;
    let input = scope
        .lower_with_private_entry_ram(
            artifact,
            private_entry_ram,
            &crate::NativeExecutionSelection {
                scope: Some(execution.scope()),
                scalar_applications,
                providers: execution.local_adapters(),
                plans: &proposal.providers().selection,
                callbacks: &proposal.occurrences().callbacks,
                entry_call: execution.scope().entry_call(),
            },
            execution.evidence().map(std::sync::Arc::as_ref),
        )
        .map_err(crate::admit_providers::artifact_lowering_error)?;
    input
        .bind_selected_provider_coordinates(artifact, execution.selection())
        .map_err(|reason| vec![Diagnostic::error(reason)])
}
