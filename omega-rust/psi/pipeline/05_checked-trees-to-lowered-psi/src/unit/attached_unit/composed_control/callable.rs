//! Emit a composed callee using the enclosing closure's catalogs and identities.
use super::super::super::{
    MachineId, ServiceDeclaration, ServiceId, ServiceReachId, StructuralDomainDeclaration,
    StructuralParameterDeclaration, StructuralTypeDeclaration,
};
use super::super::operation_frame::BoundaryParameters;
use super::super::{
    BoundaryMachineDeclaration, CheckedBoundaryMachinePlan, SemanticDomainId, StructuralDomainId,
    StructuralTypeId, TerminalMachine, ValueDeclaration, lookup_machine_id,
};
use super::{CheckedTrees, LoweringError, catalogs, scalar_calls, state_graph};
use crate::scalar_graph::scalar_call_closure::callee::PreparedScalarCallee;
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
    pub structural_domains: &'a [StructuralDomainDeclaration],
    pub domain_ids: &'a [(SemanticDomainId, StructuralDomainId)],
    pub services: &'a [ServiceDeclaration],
    pub service_ids: &'a [(ServiceReachId, ServiceId)],
    pub root_service_reach: &'a terminal_psi::TerminalRootServiceReach,
    pub boundaries: &'a [BoundaryMachineDeclaration],
    pub boundary_parameters: &'a [BoundaryParameters],
    pub machine_ids: &'a [(symbols::SymbolHandle, MachineId)],
    pub signatures: &'a [MachineSignature],
    pub scalar_requirement_counts: &'a [(symbols::SymbolHandle, usize)],
    /// The closure's Unit bodies and prepared scalar callees, which a
    /// state's scalar calls resolve against exactly as an ordinary body's do.
    pub closure: &'a [symbols::SymbolHandle],
    pub prepared_scalar_machines: &'a [PreparedScalarCallee<'a>],
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
        super::ComposedOccurrences,
        Vec<terminal_psi::ScalarBlockInvariant>,
    ),
    LoweringError,
> {
    let mut catalogs = catalogs::ComposedCatalogs {
        temporary_places: Vec::new(),
        result_places: Vec::new(),
        structural_types: Cow::Borrowed(shared.structural_types),
        type_ids: Cow::Borrowed(shared.type_ids),
        structural_domains: Cow::Borrowed(shared.structural_domains),
        domain_ids: Cow::Borrowed(shared.domain_ids),
        services: Cow::Borrowed(shared.services),
        root_service_reach: Cow::Borrowed(shared.root_service_reach),
        boundary_machines: Cow::Borrowed(shared.boundaries),
        lowered_boundaries: catalogs::LoweredComposedBoundary::roster(shared.boundary_parameters),
        boundary_parameters: Cow::Borrowed(shared.boundary_parameters),
        signatures: Cow::Borrowed(shared.signatures),
        closure: shared.closure,
        prepared_scalar_machines: shared.prepared_scalar_machines,
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
    let signature = signatures::find(shared.signatures, plan.machine)?;
    machine.contract.requires = signature.requires.clone();
    machine.contract.ensures = scalar_guarantees(
        checked,
        plan,
        &machine,
        &signature.erased_scalar_parameters,
        &mut catalogs.scalar_calls.next_call_obligation,
    )?;
    *counters.place = catalogs.next_place;
    *counters.value = catalogs.next_value;
    *counters.block = catalogs.next_block;
    *counters.operation = catalogs.next_operation;
    *counters.edge = catalogs.next_edge;
    *counters.call_obligation = catalogs.scalar_calls.next_call_obligation;
    Ok((machine, occurrences, scalar_block_invariants))
}

/// A scalar result's normal-return guarantees: the authored `ensures` and a
/// closed result range, stated over the entry parameters and the result
/// exactly as an ordinary single-state body publishes them
/// (`ordinary_machine`). Publication is not proof: the closure's operation
/// proof finalization proves each clause from the facts every exit of the
/// graph shares (Terminal intersects the exit paths), so a guarantee that
/// holds only per returning state stops lowering at its undischarged
/// obligation rather than being dropped. A crash supplies neither a result
/// nor its guarantees.
fn scalar_guarantees(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
    machine: &TerminalMachine,
    erased: &[ValueDeclaration],
    next_obligation: &mut u64,
) -> Result<Vec<terminal_psi::ContractClause>, LoweringError> {
    let terminal_psi::TerminalMachineResult::Scalar(result) = &machine.result else {
        return Ok(Vec::new());
    };
    let entry = plan
        .states
        .first()
        .ok_or(LoweringError::Unsupported("Unit graph has no entry state"))?;
    let (source, state) =
        crate::expression_preparation::source_custody::authored_state(checked, entry.state)?;
    crate::scalar_graph::scalar_contracts::validate_guarantees(checked, source, state)?;
    let contract = checked
        .facts
        .contract_plans
        .for_machine(plan.machine)
        .ok_or(LoweringError::Unsupported(
            "scalar graph result has no checked contract",
        ))?;
    let refined = crate::scalar_graph::scalar_contracts::with_result_range(
        checked,
        entry.state,
        machine.parameters.len(),
        &contract.closed_scalar_values,
    )?;
    let mut namespace = machine.parameters.clone();
    namespace.push(*result);
    crate::scalar_graph::scalar_contracts::clauses(refined.ensures(), &namespace, erased)?
        .into_iter()
        .map(|proposition| {
            Ok(terminal_psi::ContractClause {
                obligation: crate::terminal_identities::obligation_id(
                    crate::terminal_identities::allocate_dense(next_obligation)?,
                ),
                proposition,
            })
        })
        .collect()
}
