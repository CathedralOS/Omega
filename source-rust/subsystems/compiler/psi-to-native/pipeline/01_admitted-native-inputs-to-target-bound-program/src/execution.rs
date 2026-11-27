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
pub(super) fn plan_published_target_execution(
    origins: host_provider_inputs::ProviderExecutionOrigins,
    target: target::TargetProfile,
    logical: &ClosedLogicalProviders,
    bindings: &[CallableBoundaryImplementation],
    review: Option<provider_demand::PackageReviewRequirements>,
    has_bound_applications: bool,
    components: std::collections::BTreeMap<
        semantic_vocabulary::PackageKeyIdentity,
        std::sync::Arc<component_description::VerifiedComponent>,
    >,
) -> Result<SelectedExecution, NativeProgramPlanningError> {
    let artifact = std::sync::Arc::clone(origins.artifact());
    // An inspection or package review can select providers without an executable entry.
    // Keep the capture result until an execution product asks to bind the entry.
    let folded_entry = crate::folded_entry::FoldedEntryReach::capture(
        artifact.view(),
        target.target_name(),
        bindings,
        has_bound_applications,
    );
    let module = artifact.semantic_module();
    let (mut inputs, review) = crate::provider_execution_inputs::plan_inputs_and_review(
        origins,
        target,
        logical.requirements(),
        review,
    )
    .map_err(NativeProgramPlanningError::Admission)?;
    if !components.is_empty() {
        inputs.components = Some(crate::target_supply::PublishedComponentSuppliers {
            receiver: std::sync::Arc::clone(&artifact),
            suppliers: components,
        });
    }
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
    let selected = selected_boundary_implementations(module, &realization).map_err(Admission)?;
    if selected
        .windows(2)
        .any(|pair| pair[0].boundary == pair[1].boundary)
    {
        return Err(Admission(vec![Diagnostic::error(
            "chosen provider has competing exact bodies for one boundary application",
        )]));
    }
    if selected != bindings {
        return Err(Admission(vec![Diagnostic::error(
            "native planning changed a closed logical provider body",
        )]));
    }
    let realization = realization
        .resolve_installation_reach(module, &module.installation_reach_requirements())
        .map_err(|reason| Admission(vec![Diagnostic::error(reason)]))?;
    SelectedExecution::from_realization(module, Some(target), realization).map_err(Admission)
}

/// Resolve executable checked bodies supplied by this compilation. Independent calls
/// reach the imported requirement, whose actual implementation belongs to its supplier.
pub(super) fn selected_boundary_implementations(
    module: &terminal_psi::TerminalModule,
    realization: &crate::SelectedProviderRealization,
) -> Result<Vec<CallableBoundaryImplementation>, Vec<Diagnostic>> {
    let mut bindings = Vec::new();
    for (declaration, mode) in realization
        .provider_declarations()
        .iter()
        .zip(realization.composition_modes())
    {
        if mode == semantic_vocabulary::source_selections::CompositionMode::Independent {
            continue;
        }
        if !declaration.rows.iter().any(|row| {
            matches!(
                row.binding,
                build_providers::plans::ProviderBinding::CheckedAdapter { .. }
            )
        }) {
            continue;
        }
        let selected = target_provider_admission::selected_provider_candidate_rows(declaration, module)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        for row in selected {
            for position in row.candidate_positions {
                let candidate = module.interfaces.provider_candidates.get(position).ok_or_else(|| {
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
    Ok(bindings)
}
