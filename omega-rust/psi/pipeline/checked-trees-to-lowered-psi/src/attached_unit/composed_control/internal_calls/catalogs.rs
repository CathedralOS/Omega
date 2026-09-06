//! Composed roots and ordinary Unit bodies share one selected catalog.

use super::*;
use crate::attached_unit::{lower_shared_unit_closure, shared_closure::ExternalUnitRoots};

#[allow(clippy::too_many_arguments)]
pub(in crate::attached_unit::composed_control) fn lower(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    attachment_type_identity: &str,
    contract_service_reach: ServiceReachPlan,
    service_reach: ServiceReachSummary,
    states: &[checked_trees::CheckedComposedUnitControlStatePlan],
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
    targets: &[(&checked_trees::CheckedUnitEffectMachinePlan, String)],
) -> Result<super::super::catalogs::ComposedCatalogs, LoweringError> {
    let mut services = Vec::new();
    collect_installation_machine_contract_services(
        checked,
        machine,
        contract_service_reach,
        service_reach,
        &mut services,
    )?;
    for operation in states.iter().flat_map(|state| &state.operations) {
        let reach = match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall { service_reach, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { service_reach, .. }
            | CheckedUnitEffectOperationPlan::CallUnit { service_reach, .. } => *service_reach,
            _ => return unsupported("composed root retained a non-call operation"),
        };
        collect_service_summary(&checked.facts.service_reaches.rows, reach, &mut services)?;
    }
    let mut type_roots = vec![attachment_type_identity.to_owned()];
    type_roots.extend(
        states
            .iter()
            .flat_map(|state| &state.structural_parameters)
            .map(|parameter| parameter.type_identity.clone()),
    );
    let unit_roots = targets
        .iter()
        .map(|(target, _)| target.machine)
        .collect::<Vec<_>>();
    let boundary_roots = boundaries
        .iter()
        .map(|(boundary, _)| boundary.machine)
        .collect::<Vec<_>>();
    let scalar_roots = super::super::scalar_calls::selected_targets(checked, machine, states)?;
    let shared = lower_shared_unit_closure(
        checked,
        machine,
        &unit_roots,
        Some(ExternalUnitRoots {
            boundary_roots: &boundary_roots,
            structural_type_roots: &type_roots,
            service_roots: &services,
            scalar_roots: &scalar_roots,
        }),
    )?;
    let lowered_boundaries = shared
        .boundary_parameters
        .iter()
        .map(|(source, id, _, scalar_parameters)| {
            let boundary =
                unique_unit_boundary(&checked.facts.flow.terminal_unit_effects, *source)?;
            let declaration = shared
                .lowered
                .semantic_module
                .boundary_machines
                .iter()
                .find(|boundary| boundary.id == *id)
                .ok_or(LoweringError::Unsupported(
                    "shared Unit boundary declaration is absent",
                ))?;
            Ok(super::super::catalogs::LoweredComposedBoundary {
                source: *source,
                id: *id,
                checked_structural_parameters: boundary.structural_parameters.clone(),
                scalar_parameters: scalar_parameters.clone(),
                result: declaration.result.clone(),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let internal_targets = targets
        .iter()
        .map(|(target, _)| {
            let id = lookup_machine_id(&shared.machine_ids, target.machine)?;
            let declaration = shared
                .lowered
                .semantic_module
                .machines
                .iter()
                .find(|machine| machine.id == id)
                .ok_or(LoweringError::Unsupported(
                    "shared Unit target declaration is absent",
                ))?;
            if !declaration.structural_parameters.is_empty()
                || !declaration.contract.requires.is_empty()
            {
                return unsupported(
                    "composed Unit call needs structural arguments or caller-specific requirements",
                );
            }
            Ok(super::super::catalogs::LoweredComposedInternalTarget {
                source: target.machine,
                id,
                scalar_parameters: declaration
                    .parameters
                    .iter()
                    .map(|parameter| parameter.scalar_type)
                    .collect(),
                parameter_relative_crash_routes: lower_checked_crash_routes(
                    checked,
                    target.machine,
                )?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let scalar_calls = super::super::scalar_calls::ComposedScalarCalls::from_shared(
        shared.machine_ids,
        shared.scalar_requirement_counts,
        shared.next_call_obligation,
    );
    Ok(super::super::catalogs::ComposedCatalogs {
        structural_types: shared.lowered.semantic_module.structural_types.clone(),
        type_ids: shared.type_ids,
        domain_ids: shared.domain_ids,
        services: shared.lowered.semantic_module.services.clone(),
        root_service_reach: shared.lowered.semantic_module.root_service_reach.clone(),
        boundary_machines: shared.lowered.semantic_module.boundary_machines.clone(),
        lowered_boundaries,
        internal_targets,
        service_ids: shared.service_ids,
        next_place: shared.next_place,
        scalar_calls,
        root_crash_routes: lower_checked_crash_routes(checked, machine)?,
        shared_units: Some(shared.lowered),
        next_value: shared.next_value,
        next_block: shared.next_block,
        next_operation: shared.next_operation,
        next_edge: shared.next_edge,
    })
}
