//! Settle physical supply for the logical choices closed by source settlement.
use crate::SelectedExecution;
use crate::target_supply::{
    RealizedTargetSupply, TargetSupplyExecutionInputs, TargetSupplyRealizationError,
    TargetSupplyRealizationRequest, realize_target_supply,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::MachineId;
use terminal_psi::TerminalModule;
use terminal_semantics::{
    callable_values::CallableBoundaryImplementation, logical_providers::ClosedLogicalProviders,
};

#[derive(Debug)]
pub enum NativeProgramPlanningError {
    Supply(TargetSupplyRealizationError),
    Admission(Vec<Diagnostic>),
}

/// Unscoped programs must close without any boundary supply.
pub fn plan_unscoped_execution(
    module: &TerminalModule,
    roots: &[MachineId],
) -> Result<SelectedExecution, Vec<Diagnostic>> {
    let realization = RealizedTargetSupply::unscoped(module, roots)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    SelectedExecution::from_realization(module, None, realization)
}

/// Select the concrete providers from a portable handoff. Package review reads
/// its own completed inventory; its refusals never enlarge execution demand.
pub fn plan_published_target_execution(
    origins: backend_plan::ProviderExecutionOrigins,
    target: target::TargetProfile,
    logical: &ClosedLogicalProviders,
    bindings: &[CallableBoundaryImplementation],
    review: Option<backend_plan::package_review::PackageReviewRequirements>,
    folded_entry: Result<backend_plan::FoldedEntryReach, String>,
) -> Result<SelectedExecution, NativeProgramPlanningError> {
    let artifact = std::sync::Arc::clone(&origins.artifact);
    let module = artifact.semantic_module();
    let (inputs, review) = crate::provider_execution_inputs::plan_inputs_and_review(
        origins,
        target,
        logical.requirements(),
        review,
    )
    .map_err(NativeProgramPlanningError::Admission)?;
    let selected = plan_target_execution(module, target, logical, bindings, inputs)?
        .with_folded_entry_reach(folded_entry);
    Ok(match review {
        Some(review) => selected.with_package_review(module, review),
        None => selected,
    })
}

/// Admit the complete target supply before issuing a selected execution. A
/// native provider must implement precisely the bodies named by logical reach.
pub fn plan_target_execution(
    module: &TerminalModule,
    target: target::TargetProfile,
    logical: &ClosedLogicalProviders,
    bindings: &[CallableBoundaryImplementation],
    inputs: TargetSupplyExecutionInputs,
) -> Result<SelectedExecution, NativeProgramPlanningError> {
    use NativeProgramPlanningError::{Admission, Supply};
    let realization = realize_target_supply(TargetSupplyRealizationRequest::from_execution_inputs(
        module, logical, inputs,
    ))
    .map_err(Supply)?;
    if selected_boundary_implementations(module, &realization).map_err(Admission)? != bindings {
        return Err(Admission(vec![Diagnostic::error(
            "native planning changed a closed logical provider body",
        )]));
    }
    let realization = realization
        .resolve_installation_reach(module, &module.installation_reach_requirements())
        .map_err(|reason| Admission(vec![Diagnostic::error(reason)]))?;
    SelectedExecution::from_realization(module, Some(target), realization).map_err(Admission)
}

/// Grow only the body bindings of actually chosen maps. Registering their
/// other rows makes those bodies available at a call; it does not invoke them.
fn selected_boundary_implementations(
    module: &terminal_psi::TerminalModule,
    realization: &crate::target_supply::RealizedTargetSupply,
) -> Result<Vec<CallableBoundaryImplementation>, Vec<Diagnostic>> {
    let mut bindings = Vec::new();
    for declaration in realization.provider_declarations() {
        if !declaration.rows.iter().any(|row| {
            matches!(
                row.binding,
                effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
            )
        }) {
            continue;
        }
        let selected = backend_plan::selected_provider_candidate_rows(declaration, module)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        for row in selected {
            for position in row.candidate_positions {
                let candidate = module.provider_candidates.get(position).ok_or_else(|| {
                    vec![Diagnostic::error(
                        "chosen provider lost its authenticated candidate coordinate",
                    )]
                })?;
                bindings.push(CallableBoundaryImplementation {
                    boundary: candidate.boundary,
                    machine: candidate.candidate,
                });
            }
        }
    }
    bindings.sort_unstable();
    bindings.dedup();
    if bindings
        .windows(2)
        .any(|pair| pair[0].boundary == pair[1].boundary)
    {
        return Err(vec![Diagnostic::error(
            "chosen provider has competing exact bodies for one boundary application",
        )]);
    }
    Ok(bindings)
}
