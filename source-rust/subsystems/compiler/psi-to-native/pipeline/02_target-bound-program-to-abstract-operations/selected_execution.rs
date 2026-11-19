//! Lower the selected root or callable to abstract operations.
use diagnostics::Diagnostic;
use verified_optimization_input::{PreparedNativeExecution, TerminalNativeRealizationProposal};

pub fn lower_native_callable(
    callable: provider_realization::native_execution::BoundNativeCallable<'_>,
    scalar_applications: Option<crate::NativeScalarApplications<'_>>,
) -> Result<
    (
        crate::VerifiedNativeArtifactInput,
        boundary_applications::TerminalBoundaryApplicationCoverage,
    ),
    Vec<Diagnostic>,
> {
    let (proposal, execution, artifact, entry, checked_scope) = callable.into_parts();
    let selection = crate::NativeExecutionSelection::checked(
        &checked_scope,
        &proposal.providers().selection,
        scalar_applications,
    );
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
            &[],
            checked_scope.local_adapters(),
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
    let selection = crate::NativeExecutionSelection::checked(
        checked,
        &proposal.providers().selection,
        scalar_applications,
    );
    let scope = crate::selected_input::NativeScopePreparation::prepare(
        authorization,
        proposal.target_profile(),
        terminal_psi::TargetMachineCallable::Machine(checked.entry()),
        &selection,
    )
    .map_err(crate::admit_providers::artifact_lowering_error)?;
    let input = scope
        .lower_with_private_entry_ram(
            private_entry_ram,
            execution.local_adapters(),
            execution.evidence().map(std::sync::Arc::as_ref),
        )
        .map_err(crate::admit_providers::artifact_lowering_error)?;
    input
        .bind_selected_provider_coordinates(artifact, execution.selection())
        .map_err(|reason| vec![Diagnostic::error(reason)])
}
