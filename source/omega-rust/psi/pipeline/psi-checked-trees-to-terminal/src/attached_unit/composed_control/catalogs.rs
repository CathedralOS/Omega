//! Selected type, boundary, and service catalogs for composed Unit control.

use super::*;
use crate::attached_unit::catalog::{
    collect_installation_machine_contract_services, collect_published_contract_services,
    collect_service_summary, lower_program_local_root_introductions, lower_selected_unit_services,
    lower_unit_structural_type_roots,
};

pub(super) struct ComposedCatalogs {
    pub(super) structural_types: Vec<StructuralTypeDeclaration>,
    pub(super) type_ids: Vec<(String, StructuralTypeId)>,
    pub(super) services: Vec<ServiceDeclaration>,
    pub(super) root_service_reach: psi_terminal::TerminalRootServiceReach,
    pub(super) boundary_machines: Vec<BoundaryMachineDeclaration>,
    pub(super) lowered_boundaries: Vec<LoweredComposedBoundary>,
    pub(super) internal_targets: Vec<LoweredComposedInternalTarget>,
    pub(super) service_ids: Vec<(ServiceReachId, ServiceId)>,
    pub(super) next_place: u64,
}

fn lower_composed_services(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedComposedUnitControlMachinePlan,
    admitted: &admission::AdmittedComposedUnit<'_>,
) -> Result<(Vec<ServiceDeclaration>, Vec<(ServiceReachId, ServiceId)>), LoweringError> {
    let facts = &checked.facts.service_reaches;
    let mut selected = Vec::new();
    collect_installation_machine_contract_services(
        checked,
        plan.machine,
        plan.contract_service_reach,
        plan.service_reach,
        &mut selected,
    )?;
    for operation in plan.states.iter().flat_map(|state| &state.operations) {
        let service_reach = match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall { service_reach, .. }
            | CheckedUnitEffectOperationPlan::CallUnit { service_reach, .. } => *service_reach,
            _ => return unsupported("composed Unit control contains a non-call operation"),
        };
        collect_service_summary(&facts.rows, service_reach, &mut selected)?;
    }
    for (boundary, _) in &admitted.boundaries {
        collect_published_contract_services(
            &facts.rows,
            boundary.contract_service_reach,
            boundary.service_reach,
            &mut selected,
        )?;
    }
    for (target, _) in &admitted.internal_targets {
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

pub(super) struct LoweredComposedInternalTarget {
    pub(super) source: psi_symbols::SymbolHandle,
    pub(super) id: MachineId,
    pub(super) attachment_type_identity: String,
    pub(super) contract_service_reach: ServiceReachPlan,
    pub(super) service_reach: ServiceReachSummary,
}

pub(super) struct LoweredComposedBoundary {
    pub(super) source: psi_symbols::SymbolHandle,
    pub(super) id: BoundaryMachineId,
    pub(super) checked_structural_parameters:
        Vec<psi_checked_trees::CheckedUnitStructuralParameterPlan>,
    pub(super) scalar_parameters: Vec<ScalarType>,
}

pub(super) fn lower_composed_catalogs(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedComposedUnitControlMachinePlan,
    admitted: &admission::AdmittedComposedUnit<'_>,
) -> Result<ComposedCatalogs, LoweringError> {
    let mut type_roots = vec![plan.attachment_type_identity.clone()];
    type_roots.extend(
        plan.states
            .iter()
            .flat_map(|state| &state.structural_parameters)
            .map(|parameter| parameter.type_identity.clone()),
    );
    for (boundary, _) in &admitted.boundaries {
        type_roots.extend(boundary.attachment_type_identity.iter().cloned());
        type_roots.extend(
            boundary
                .structural_parameters
                .iter()
                .map(|parameter| parameter.type_identity.clone()),
        );
    }
    type_roots.extend(
        admitted
            .internal_targets
            .iter()
            .map(|(target, _)| target.attachment_type_identity.clone()),
    );
    let (structural_types, type_ids) = lower_unit_structural_type_roots(checked, &type_roots)?;
    let (services, service_ids) = lower_composed_services(checked, plan, admitted)?;
    let root_service_reach = lower_root_service_reach(checked, plan.machine, &service_ids)?;
    let mut boundary_machines = Vec::with_capacity(admitted.boundaries.len());
    let mut lowered_boundaries = Vec::with_capacity(admitted.boundaries.len());
    let internal_targets = admitted
        .internal_targets
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
                attachment_type_identity: target.attachment_type_identity.clone(),
                contract_service_reach: target.contract_service_reach,
                service_reach: target.service_reach,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut next_place = 1_u64;
    for (index, (boundary, identity)) in admitted.boundaries.iter().enumerate() {
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
            result: None,
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
