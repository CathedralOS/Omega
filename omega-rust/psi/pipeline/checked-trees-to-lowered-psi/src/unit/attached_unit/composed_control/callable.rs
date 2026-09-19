//! Emit a composed callee using the enclosing closure's catalogs and identities.
use super::super::super::{
    BoundaryMachineId, LoweredSourceCallOccurrence, MachineId, ServiceDeclaration, ServiceId,
    ServiceReachId, StructuralParameterDeclaration, StructuralTypeDeclaration,
};
use super::super::{
    BoundaryMachineDeclaration, CheckedBoundaryMachinePlan, ScalarType, SemanticDomainId,
    StructuralDomainId, StructuralTypeId, TerminalMachine, ValueDeclaration, lookup_machine_id,
    unique_unit_boundary,
};
use super::{CheckedTrees, LoweringError, catalogs, scalar_calls, state_graph};
use crate::unit::attached_unit::signatures::{self, MachineSignature};
use std::borrow::Cow;

pub(in crate::unit::attached_unit) type CallableBody<'a> = state_graph::AdmittedGraph<'a>;

impl<'a> CallableBody<'a> {
    pub(in crate::unit::attached_unit) fn boundaries(
        &self,
    ) -> &[(&'a CheckedBoundaryMachinePlan, String)] {
        &self.boundaries
    }
}

pub(in crate::unit::attached_unit) fn admit<'a>(
    checked: &'a CheckedTrees,
    plan: &'a checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<CallableBody<'a>, LoweringError> {
    if !state_graph::has_shared_graph_custody(checked, plan) {
        return super::super::unsupported("composed callee has unsupported graph custody");
    }
    state_graph::admit(checked, plan)
}

pub(in crate::unit::attached_unit) struct SharedCatalog<'a> {
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
    pub signatures: &'a [MachineSignature],
    pub scalar_requirement_counts: &'a [(symbols::SymbolHandle, usize)],
}

pub(in crate::unit::attached_unit) struct EmissionCounters<'a> {
    pub place: &'a mut u64,
    pub value: &'a mut u64,
    pub block: &'a mut u64,
    pub operation: &'a mut u64,
    pub edge: &'a mut u64,
    pub call_obligation: &'a mut u64,
}

pub(in crate::unit::attached_unit) fn emit(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
    admitted: CallableBody<'_>,
    parameters: Vec<StructuralParameterDeclaration>,
    scalar_parameters: Vec<ValueDeclaration>,
    shared: SharedCatalog<'_>,
    counters: EmissionCounters<'_>,
) -> Result<
    (
        TerminalMachine,
        Vec<LoweredSourceCallOccurrence>,
        Vec<terminal_psi::ScalarBlockInvariant>,
    ),
    LoweringError,
> {
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
    let source_targets = &admitted.internal_targets;
    let internal_targets = source_targets
        .iter()
        .map(|(body, _)| {
            let target = body.entry()?;
            let signature = signatures::find(shared.signatures, target.machine)?;
            Ok(catalogs::LoweredComposedInternalTarget {
                result: body.result()?,
                source: target.machine,
                structural_parameters: target.structural_parameters.to_vec(),
                id: lookup_machine_id(shared.machine_ids, target.machine)?,
                erased_scalar_formals: signature.erased_scalar_parameters.clone(),
                requires: signature.requires.clone(),
                scalar_parameters: signature
                    .scalar_parameters
                    .iter()
                    .map(|parameter| parameter.scalar_type)
                    .collect(),
                parameter_relative_crash_routes: crate::unit::effective_crash_routes(
                    checked,
                    target.machine,
                )?,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let mut catalogs = catalogs::ComposedCatalogs {
        temporary_places: Vec::new(),
        result_places: Vec::new(),
        structural_types: Cow::Borrowed(shared.structural_types),
        type_ids: Cow::Borrowed(shared.type_ids),
        domain_ids: Cow::Borrowed(shared.domain_ids),
        services: Cow::Borrowed(shared.services),
        root_service_reach: Cow::Borrowed(shared.root_service_reach),
        boundary_machines: Cow::Borrowed(shared.boundaries),
        lowered_boundaries,
        internal_targets,
        service_ids: Cow::Borrowed(shared.service_ids),
        scalar_calls: scalar_calls::ComposedScalarCalls::from_shared(
            shared.machine_ids.to_vec(),
            shared.scalar_requirement_counts.to_vec(),
            *counters.call_obligation,
        ),
        root_crash_routes: crate::unit::effective_crash_routes(checked, plan.machine)?,
        shared_units: None,
        next_place: *counters.place,
        next_value: *counters.value,
        next_block: *counters.block,
        next_operation: *counters.operation,
        next_edge: *counters.edge,
    };
    let identity = lookup_machine_id(shared.machine_ids, plan.machine)?;
    // The signature's erased formals are the single namespace the published
    // contract's `requires` and erased call lanes both cite; the emitted
    // contract reuses them rather than minting parallel declarations.
    let entry_erased_formals = signatures::find(shared.signatures, plan.machine)?
        .erased_scalar_parameters
        .clone();
    let (mut machine, occurrences, scalar_block_invariants) = state_graph::emit(
        checked,
        plan,
        admitted,
        identity,
        parameters,
        scalar_parameters,
        entry_erased_formals,
        &mut catalogs,
    )?;
    // `requires` already carries the merged closed clause proposition ahead
    // of the runtime requirements in published contract order.
    machine.contract.requires = signatures::find(shared.signatures, plan.machine)?
        .requires
        .clone();
    *counters.place = catalogs.next_place;
    *counters.value = catalogs.next_value;
    *counters.block = catalogs.next_block;
    *counters.operation = catalogs.next_operation;
    *counters.edge = catalogs.next_edge;
    *counters.call_obligation = catalogs.scalar_calls.next_call_obligation;
    Ok((machine, occurrences, scalar_block_invariants))
}
