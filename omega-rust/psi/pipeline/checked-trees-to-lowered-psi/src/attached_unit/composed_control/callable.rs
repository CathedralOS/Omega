//! Emit a composed callee using the enclosing closure's catalogs and identities.

use super::*;

pub(in crate::attached_unit) enum CallableBody<'a> {
    Composed(admission::AdmittedComposedUnit<'a>),
    Graph(state_graph::AdmittedGraph<'a>),
}

impl<'a> CallableBody<'a> {
    pub(in crate::attached_unit) fn boundaries(
        &self,
    ) -> &[(&'a CheckedBoundaryMachinePlan, String)] {
        match self {
            Self::Composed(body) => &body.boundaries,
            Self::Graph(body) => &body.boundaries,
        }
    }
}

pub(in crate::attached_unit) fn admit<'a>(
    checked: &'a CheckedTrees,
    plan: &'a checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<CallableBody<'a>, LoweringError> {
    if state_graph::has_shared_graph_custody(checked, plan) {
        state_graph::admit(checked, plan).map(CallableBody::Graph)
    } else {
        admission::admit_composed_unit_control(checked, plan).map(CallableBody::Composed)
    }
}

pub(in crate::attached_unit) struct SharedCatalog<'a> {
    pub structural_types: &'a [StructuralTypeDeclaration],
    pub type_ids: &'a [(String, StructuralTypeId)],
    pub domain_ids: &'a [(SemanticDomainId, StructuralDomainId)],
    pub services: &'a [ServiceDeclaration],
    pub service_ids: &'a [(ServiceReachId, ServiceId)],
    pub root_service_reach: &'a terminal_psi::TerminalRootServiceReach,
    pub boundaries: &'a [BoundaryMachineDeclaration],
    pub boundary_parameters: &'a [(
        symbols::SymbolHandle,
        BoundaryMachineId,
        Vec<StructuralParameterDeclaration>,
        Vec<ScalarType>,
    )],
    pub machine_ids: &'a [(symbols::SymbolHandle, MachineId)],
    pub scalar_parameters: &'a [(symbols::SymbolHandle, Vec<ValueDeclaration>)],
    pub requirements: &'a [(symbols::SymbolHandle, Vec<Proposition>)],
    pub scalar_requirement_counts: &'a [(symbols::SymbolHandle, usize)],
}

pub(in crate::attached_unit) struct EmissionCounters<'a> {
    pub place: &'a mut u64,
    pub value: &'a mut u64,
    pub block: &'a mut u64,
    pub operation: &'a mut u64,
    pub edge: &'a mut u64,
    pub call_obligation: &'a mut u64,
}

pub(in crate::attached_unit) fn emit(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
    admitted: CallableBody<'_>,
    parameters: Vec<StructuralParameterDeclaration>,
    scalar_parameters: Vec<ValueDeclaration>,
    shared: SharedCatalog<'_>,
    counters: EmissionCounters<'_>,
) -> Result<(TerminalMachine, Vec<LoweredSourceCallOccurrence>), LoweringError> {
    let lowered_boundaries = shared
        .boundary_parameters
        .iter()
        .map(|(source, id, _, scalar_parameters)| {
            let source_plan =
                unique_unit_boundary(&checked.facts.flow.terminal_unit_effects, *source)?;
            let declaration = shared
                .boundaries
                .iter()
                .find(|declaration| declaration.id == *id)
                .ok_or(LoweringError::Unsupported(
                    "shared callable boundary is absent",
                ))?;
            Ok(catalogs::LoweredComposedBoundary {
                source: *source,
                id: *id,
                checked_structural_parameters: source_plan.structural_parameters.clone(),
                scalar_parameters: scalar_parameters.clone(),
                result: declaration.result.clone(),
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let source_targets = match &admitted {
        CallableBody::Composed(body) => &body.internal_targets,
        CallableBody::Graph(body) => &body.internal_targets,
    };
    let internal_targets = source_targets
        .iter()
        .map(|(body, _)| {
            let target = body.entry()?;
            let parameters = shared
                .scalar_parameters
                .iter()
                .find_map(|(source, parameters)| (*source == target.machine).then_some(parameters))
                .ok_or(LoweringError::Unsupported(
                    "shared callable scalar signature is absent",
                ))?;
            let requirements = shared
                .requirements
                .iter()
                .find_map(|(source, requirements)| {
                    (*source == target.machine).then_some(requirements)
                })
                .ok_or(LoweringError::Unsupported(
                    "shared callable requirements are absent",
                ))?;
            if !target.structural_parameters.is_empty() || !requirements.is_empty() {
                return unsupported(
                    "composed Unit call needs structural arguments or caller-specific requirements",
                );
            }
            Ok(catalogs::LoweredComposedInternalTarget {
                source: target.machine,
                id: lookup_machine_id(shared.machine_ids, target.machine)?,
                scalar_parameters: parameters
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
    let mut catalogs = catalogs::ComposedCatalogs {
        structural_types: shared.structural_types.to_vec(),
        type_ids: shared.type_ids.to_vec(),
        domain_ids: shared.domain_ids.to_vec(),
        services: shared.services.to_vec(),
        root_service_reach: shared.root_service_reach.clone(),
        boundary_machines: shared.boundaries.to_vec(),
        lowered_boundaries,
        internal_targets,
        service_ids: shared.service_ids.to_vec(),
        scalar_calls: scalar_calls::ComposedScalarCalls::from_shared(
            shared.machine_ids.to_vec(),
            shared.scalar_requirement_counts.to_vec(),
            *counters.call_obligation,
        ),
        root_crash_routes: lower_checked_crash_routes(checked, plan.machine)?,
        shared_units: None,
        next_place: *counters.place,
        next_value: *counters.value,
        next_block: *counters.block,
        next_operation: *counters.operation,
        next_edge: *counters.edge,
    };
    let identity = lookup_machine_id(shared.machine_ids, plan.machine)?;
    let (mut machine, occurrences) = match admitted {
        CallableBody::Composed(body) => emission::emit_callable_body(
            checked,
            plan,
            body,
            identity,
            scalar_parameters,
            parameters,
            &mut catalogs,
        )?,
        CallableBody::Graph(body) => state_graph::emit(
            checked,
            plan,
            body,
            identity,
            parameters,
            scalar_parameters,
            &mut catalogs,
        )?,
    };
    machine.contract.requires = shared
        .requirements
        .iter()
        .find_map(|(source, requirements)| {
            (*source == plan.machine).then_some(requirements.clone())
        })
        .ok_or(LoweringError::Unsupported(
            "composed callable entry requirements are absent",
        ))?;
    *counters.place = catalogs.next_place;
    *counters.value = catalogs.next_value;
    *counters.block = catalogs.next_block;
    *counters.operation = catalogs.next_operation;
    *counters.edge = catalogs.next_edge;
    *counters.call_obligation = catalogs.scalar_calls.next_call_obligation;
    Ok((machine, occurrences))
}
