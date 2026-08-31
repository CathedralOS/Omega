//! Selected type, boundary, and service catalogs for composed Unit control.

use super::*;
use crate::attached_unit::catalog::{
    lower_composed_unit_services, lower_program_local_root_introductions,
};

pub(super) struct ComposedCatalogs {
    pub(super) structural_types: Vec<StructuralTypeDeclaration>,
    pub(super) type_ids: Vec<(String, StructuralTypeId)>,
    pub(super) services: Vec<ServiceDeclaration>,
    pub(super) root_service_reach: psi_terminal::TerminalRootServiceReach,
    pub(super) boundary_machines: Vec<BoundaryMachineDeclaration>,
    pub(super) lowered_boundaries: Vec<(
        psi_symbols::SymbolHandle,
        BoundaryMachineId,
        Vec<ScalarType>,
    )>,
    pub(super) service_ids: Vec<(ServiceReachId, ServiceId)>,
}

pub(super) fn lower_composed_catalogs(
    checked: &CheckedTrees,
    plan: &psi_checked_trees::CheckedComposedUnitControlMachinePlan,
    admitted: &admission::AdmittedComposedUnit<'_>,
) -> Result<ComposedCatalogs, LoweringError> {
    let (structural_types, type_ids) =
        lower_structural_type_plans(std::slice::from_ref(admitted.attachment))?;
    let (services, service_ids) =
        lower_composed_unit_services(checked, plan, &admitted.boundaries)?;
    let root_service_reach = lower_root_service_reach(checked, plan.machine, &service_ids)?;
    let mut boundary_machines = Vec::with_capacity(admitted.boundaries.len());
    let mut lowered_boundaries = Vec::with_capacity(admitted.boundaries.len());
    for (index, (boundary, identity)) in admitted.boundaries.iter().enumerate() {
        let scalar_parameters = boundary
            .scalar_parameters
            .iter()
            .map(|parameter| terminal_scalar_type(parameter.primitive_type))
            .collect::<Result<Vec<_>, _>>()?;
        let id = boundary_machine_id(dense_identity(index)?);
        boundary_machines.push(BoundaryMachineDeclaration {
            id,
            identity: identity.clone(),
            attachment: None,
            scalar_parameters: scalar_parameters.clone(),
            structural_parameters: Vec::new(),
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: lower_program_local_root_introductions(
                checked,
                boundary,
                identity,
                &[],
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
        lowered_boundaries.push((boundary.machine, id, scalar_parameters));
    }
    Ok(ComposedCatalogs {
        structural_types,
        type_ids,
        services,
        root_service_reach,
        boundary_machines,
        lowered_boundaries,
        service_ids,
    })
}
