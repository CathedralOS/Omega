//! Composed roots and callable Unit bodies share one selected catalog.
use super::super::super::super::ServiceReachPlan;
use super::super::super::{
    CheckedBoundaryMachinePlan, CheckedUnitEffectOperationPlan, ServiceReachSummary,
    collect_installation_machine_contract_services, collect_service_summary, unsupported,
};
use super::super::{CheckedTrees, LoweringError};

use crate::unit::attached_unit::bodies::UnitBody;
use crate::unit::attached_unit::shared_closure::ExternalUnitRoots;
use crate::unit::attached_unit::{UnitClosureRequest, lower_unit_closure};

#[allow(clippy::too_many_arguments)]
pub(in crate::unit::attached_unit::composed_control) fn lower(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    attachment_type_identity: Option<&str>,
    contract_service_reach: ServiceReachPlan,
    service_reach: ServiceReachSummary,
    states: &[checked_trees::CheckedComposedUnitControlStatePlan],
    boundaries: &[(&CheckedBoundaryMachinePlan, String)],
    targets: &[(UnitBody<'_>, String)],
) -> Result<super::super::catalogs::ComposedCatalogs<'static>, LoweringError> {
    let mut services = Vec::new();
    collect_installation_machine_contract_services(
        checked,
        machine,
        contract_service_reach,
        service_reach,
        &mut services,
    )?;
    for operation in states
        .iter()
        .flat_map(|state| state.operation_dependencies())
        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
    {
        let reach = match operation {
            CheckedUnitEffectOperationPlan::BoundaryCall { service_reach, .. }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { service_reach, .. }
            | CheckedUnitEffectOperationPlan::StructuralCall { service_reach, .. }
            | CheckedUnitEffectOperationPlan::ScalarCall { service_reach, .. }
            | CheckedUnitEffectOperationPlan::CallUnit { service_reach, .. } => *service_reach,
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
            | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
            | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
            | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
            | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
            | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
            | CheckedUnitEffectOperationPlan::EstablishViewSubslice { .. }
            | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. } => continue,
            _ => return unsupported("composed root retained a non-call operation"),
        };
        collect_service_summary(&checked.facts.service_reaches.rows, reach, &mut services)?;
    }
    let mut type_roots = attachment_type_identity
        .map(str::to_owned)
        .into_iter()
        .collect::<Vec<_>>();
    type_roots.extend(crate::scalar_graph::scalar_computations::cases::type_roots(
        checked, machine,
    )?);
    type_roots.extend(
        states
            .iter()
            .flat_map(|state| &state.structural_parameters)
            .map(|parameter| parameter.type_identity.clone()),
    );
    type_roots.extend(
        states
            .iter()
            .flat_map(|state| state.operation_dependencies())
            .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
                | CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
                | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. } => {
                    Some(result.type_identity.clone())
                }
                _ => None,
            }),
    );
    let unit_roots = targets
        .iter()
        .map(|(target, _)| target.entry().map(|entry| entry.machine))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let boundary_roots = boundaries
        .iter()
        .map(|(boundary, _)| boundary.machine)
        .collect::<Vec<_>>();
    let scalar_roots = super::super::scalar_calls::selected_targets(checked, machine, states)?;
    let shared = lower_unit_closure(
        checked,
        &UnitClosureRequest {
            external: Some(ExternalUnitRoots {
                boundary_roots: &boundary_roots,
                structural_type_roots: &type_roots,
                service_roots: &services,
                scalar_roots: &scalar_roots,
            }),
            ..UnitClosureRequest::unit(checked, machine, &unit_roots)
        },
    )?;
    let scalar_calls = super::super::scalar_calls::ComposedScalarCalls::from_shared(
        shared.machine_ids,
        shared.scalar_requirement_counts,
        shared.next_call_obligation,
    );
    Ok(super::super::catalogs::ComposedCatalogs {
        temporary_places: Vec::new(),
        result_places: Vec::new(),
        structural_types: shared
            .lowered
            .semantic_module
            .structural_types
            .clone()
            .into(),
        type_ids: shared.type_ids.into(),
        structural_domains: shared
            .lowered
            .semantic_module
            .structural_domains
            .clone()
            .into(),
        domain_ids: shared.domain_ids.into(),
        services: shared.lowered.semantic_module.services.clone().into(),
        root_service_reach: std::borrow::Cow::Owned(
            shared.lowered.semantic_module.root_service_reach.clone(),
        ),
        boundary_machines: shared
            .lowered
            .semantic_module
            .boundary_machines
            .clone()
            .into(),
        lowered_boundaries: super::super::catalogs::LoweredComposedBoundary::roster(
            &shared.boundary_parameters,
        ),
        boundary_parameters: shared.boundary_parameters.into(),
        signatures: shared.signatures.into(),
        // Direct dynamic leaves admit only boundary and Unit calls.
        closure: &[],
        prepared_scalar_machines: &[],
        service_ids: shared.service_ids.into(),
        next_place: shared.next_place,
        scalar_calls,
        root_crash_routes: crate::unit::effective_crash_routes(checked, machine)?,
        shared_units: Some(shared.lowered),
        next_value: shared.next_value,
        next_block: shared.next_block,
        next_operation: shared.next_operation,
        next_edge: shared.next_edge,
    })
}
