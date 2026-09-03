//! Selected type, boundary, and service catalogs for composed Unit control.

use super::*;
use crate::attached_unit::catalog::{
    collect_installation_machine_contract_services, collect_published_contract_services,
    collect_service_summary, lower_program_local_root_introductions, lower_selected_unit_services,
    lower_unit_structural_type_roots,
};

pub(crate) struct ComposedCatalogs {
    pub(crate) structural_types: Vec<StructuralTypeDeclaration>,
    pub(crate) type_ids: Vec<(String, StructuralTypeId)>,
    pub(crate) services: Vec<ServiceDeclaration>,
    pub(crate) root_service_reach: psi_terminal::TerminalRootServiceReach,
    pub(crate) boundary_machines: Vec<BoundaryMachineDeclaration>,
    pub(crate) lowered_boundaries: Vec<LoweredComposedBoundary>,
    pub(crate) internal_targets: Vec<LoweredComposedInternalTarget>,
    pub(crate) service_ids: Vec<(ServiceReachId, ServiceId)>,
    pub(crate) next_place: u64,
}

fn lower_composed_services(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    contract_service_reach: ServiceReachPlan,
    service_reach: ServiceReachSummary,
    states: &[psi_checked_trees::CheckedComposedUnitControlStatePlan],
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
    internal_targets: &[(&psi_checked_trees::CheckedUnitEffectMachinePlan, String)],
) -> Result<(Vec<ServiceDeclaration>, Vec<(ServiceReachId, ServiceId)>), LoweringError> {
    let facts = &checked.facts.service_reaches;
    let mut selected = Vec::new();
    collect_installation_machine_contract_services(
        checked,
        machine,
        contract_service_reach,
        service_reach,
        &mut selected,
    )?;
    for operation in states.iter().flat_map(|state| &state.operations) {
        let service_reach = match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall { service_reach, .. }
            | CheckedUnitEffectOperationPlan::CallUnit { service_reach, .. } => *service_reach,
            _ => return unsupported("composed Unit control contains a non-call operation"),
        };
        collect_service_summary(&facts.rows, service_reach, &mut selected)?;
    }
    for (boundary, _) in boundaries {
        collect_published_contract_services(
            &facts.rows,
            boundary.contract_service_reach,
            boundary.service_reach,
            &mut selected,
        )?;
    }
    for (target, _) in internal_targets {
        collect_installation_machine_contract_services(
            checked,
            target.machine,
            target.contract_service_reach,
            target.service_reach,
            &mut selected,
        )?;
    }
    lower_selected_unit_services(checked, selected)
}

pub(crate) struct LoweredComposedInternalTarget {
    pub(super) source: psi_symbols::SymbolHandle,
    pub(super) id: MachineId,
    pub(super) attachment_type_identity: String,
    pub(super) contract_service_reach: ServiceReachPlan,
    pub(super) service_reach: ServiceReachSummary,
    pub(super) nested_call_target: Option<psi_symbols::SymbolHandle>,
}

pub(crate) struct LoweredComposedBoundary {
    pub(crate) source: psi_symbols::SymbolHandle,
    pub(crate) id: BoundaryMachineId,
    pub(crate) checked_structural_parameters:
        Vec<psi_checked_trees::CheckedUnitStructuralParameterPlan>,
    pub(crate) scalar_parameters: Vec<ScalarType>,
}

pub(super) fn lower_composed_catalogs(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedComposedUnitControlMachinePlan,
    admitted: &admission::AdmittedComposedUnit<'_>,
) -> Result<ComposedCatalogs, LoweringError> {
    lower_catalogs(
        checked,
        plan.machine,
        &plan.attachment_type_identity,
        plan.contract_service_reach,
        plan.service_reach,
        &plan.states,
        &admitted.boundaries,
        &admitted.internal_targets,
    )
}

pub(crate) fn lower_dynamic_catalogs(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedDynamicScalarCallPlan,
    continuation: &psi_checked_trees::CheckedDynamicUnitContinuationPlan,
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
) -> Result<ComposedCatalogs, LoweringError> {
    let contract_service_reach = checked
        .facts
        .service_reaches
        .plan_for_machine(plan.caller_machine)
        .ok_or(LoweringError::Unsupported(
            "direct dynamic continuation has no checked service contract",
        ))?;
    lower_catalogs(
        checked,
        plan.caller_machine,
        &plan.caller_attachment_type_identity,
        contract_service_reach,
        plan.caller_service_reach,
        &continuation.leaves,
        boundaries,
        &[],
    )
}

#[allow(clippy::too_many_arguments)]
fn lower_catalogs(
    checked: &CheckedTrees,
    machine: psi_symbols::SymbolHandle,
    attachment_type_identity: &str,
    contract_service_reach: ServiceReachPlan,
    service_reach: ServiceReachSummary,
    states: &[psi_checked_trees::CheckedComposedUnitControlStatePlan],
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
    admitted_internal_targets: &[(&psi_checked_trees::CheckedUnitEffectMachinePlan, String)],
) -> Result<ComposedCatalogs, LoweringError> {
    let mut type_roots = vec![attachment_type_identity.to_owned()];
    type_roots.extend(
        states
            .iter()
            .flat_map(|state| &state.structural_parameters)
            .map(|parameter| parameter.type_identity.clone()),
    );
    for (boundary, _) in boundaries {
        type_roots.extend(boundary.attachment_type_identity.iter().cloned());
        type_roots.extend(
            boundary
                .structural_parameters
                .iter()
                .map(|parameter| parameter.type_identity.clone()),
        );
    }
    type_roots.extend(
        admitted_internal_targets
            .iter()
            .filter_map(|(target, _)| target.attachment_type_identity.clone()),
    );
    let (structural_types, type_ids) = lower_unit_structural_type_roots(checked, &type_roots)?;
    let (services, service_ids) = lower_composed_services(
        checked,
        machine,
        contract_service_reach,
        service_reach,
        states,
        boundaries,
        admitted_internal_targets,
    )?;
    let root_service_reach = lower_root_service_reach(checked, machine, &service_ids)?;
    let mut boundary_machines = Vec::with_capacity(boundaries.len());
    let mut lowered_boundaries = Vec::with_capacity(boundaries.len());
    let internal_targets = admitted_internal_targets
        .iter()
        .enumerate()
        .map(|(index, (target, _))| {
            Ok(LoweredComposedInternalTarget {
                source: target.machine,
                id: machine_id(
                    u64::try_from(index)
                        .map_err(|_| {
                            LoweringError::Unsupported(
                                "composed Unit internal target count exceeds u64",
                            )
                        })?
                        .checked_add(2)
                        .ok_or(LoweringError::Unsupported(
                            "composed Unit internal machine identity space is exhausted",
                        ))?,
                ),
                attachment_type_identity: target.attachment_type_identity.clone().ok_or(
                    LoweringError::Unsupported(
                        "composed Unit internal target is not an attached machine",
                    ),
                )?,
                contract_service_reach: target.contract_service_reach,
                service_reach: target.service_reach,
                nested_call_target: target.operations.iter().find_map(
                    |operation| match operation {
                        CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. } => {
                            Some(*target_machine)
                        }
                        _ => None,
                    },
                ),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut next_place = 1_u64;
    for (index, (boundary, identity)) in boundaries.iter().enumerate() {
        let scalar_parameters = boundary
            .scalar_parameters
            .iter()
            .map(|parameter| terminal_scalar_type(parameter.primitive_type))
            .collect::<Result<Vec<_>, _>>()?;
        let id = boundary_machine_id(dense_identity(index)?);
        let structural_parameters = lower_unit_parameters(
            &boundary.structural_parameters,
            &type_ids,
            &[],
            &mut next_place,
        )?;
        boundary_machines.push(BoundaryMachineDeclaration {
            id,
            identity: identity.clone(),
            attachment: boundary
                .attachment_type_identity
                .as_ref()
                .map(|identity| lookup_type_id(&type_ids, identity))
                .transpose()?,
            scalar_parameters: scalar_parameters.clone(),
            structural_parameters: structural_parameters.clone(),
            result: BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: lower_program_local_root_introductions(
                checked,
                boundary,
                identity,
                &structural_parameters,
                &[],
            )?,
            content_guarantees: lower_boundary_content_guarantees(
                &checked.facts.qualifications.content.conservation_plans,
                boundary.state,
            )?,
            published_service_ceiling: lower_published_service_ceiling(
                &checked.facts.service_reaches.rows,
                boundary.contract_service_reach,
                boundary.service_reach,
                &service_ids,
            )?,
        });
        lowered_boundaries.push(LoweredComposedBoundary {
            source: boundary.machine,
            id,
            checked_structural_parameters: boundary.structural_parameters.clone(),
            scalar_parameters,
        });
    }
    Ok(ComposedCatalogs {
        structural_types,
        type_ids,
        services,
        root_service_reach,
        boundary_machines,
        lowered_boundaries,
        internal_targets,
        service_ids,
        next_place,
    })
}
