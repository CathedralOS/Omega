use omega_control_flow::{
    ControlFlowBorrowRoots, ControlFlowBoundaryRoots, ControlFlowContractRoots,
    ControlFlowFactRoots, ControlFlowOwnershipRoots, ControlFlowSemanticRoots,
    ControlFlowServiceReachRoots, ControlFlowValueRoots,
};
use omega_state_graph::{
    StateGraph, StateGraphBorrowRoots as SourceBorrowRoots,
    StateGraphBoundaryRoots as SourceBoundaryRoots, StateGraphContractRoots as SourceContractRoots,
    StateGraphFactRoots as SourceFactRoots, StateGraphOwnershipRoots as SourceOwnershipRoots,
    StateGraphSemanticRoots as SourceSemanticRoots,
    StateGraphServiceReachRoots as SourceServiceReachRoots,
    StateGraphValueRoots as SourceValueRoots,
};

use crate::borrows::{
    remap_borrow_activation_owned, remap_borrow_activations, remap_borrow_argument_access_owned,
    remap_borrow_argument_accesses, remap_borrow_call_owned, remap_borrow_calls,
    remap_borrow_loan_owned, remap_borrow_loans, remap_borrow_weakening_owned,
    remap_borrow_weakenings, remap_borrow_writable_root_owned, remap_borrow_writable_roots,
};
use crate::boundaries::{remap_boundary_edge_owned, remap_boundary_edges};
use crate::contracts::{
    remap_contract_call_owned, remap_contract_calls, remap_contract_exit_owned,
    remap_contract_exits, remap_contract_fact_ref_owned, remap_contract_fact_refs,
};
use crate::facts::{
    remap_invariant_owned, remap_invariants, remap_proof_obligation_owned, remap_proof_obligations,
};
use crate::ownership::{remap_permission_event_owned, remap_permission_events};
use crate::values::{remap_value_owned, remap_values};

pub(crate) fn remap_semantic_roots(state_graph: &StateGraph) -> ControlFlowSemanticRoots {
    ControlFlowSemanticRoots::with_roots(
        ControlFlowServiceReachRoots::with_roots(
            state_graph.semantics.service_reach.services.clone(),
            state_graph.semantics.service_reach.rows.clone(),
        ),
        remap_fact_roots(state_graph),
        remap_contract_roots(state_graph),
        remap_value_roots(state_graph),
        remap_boundary_roots(state_graph),
        remap_borrow_roots(state_graph),
        remap_ownership_roots(state_graph),
    )
}

pub(crate) fn remap_semantic_roots_owned(
    semantics: SourceSemanticRoots,
) -> ControlFlowSemanticRoots {
    let SourceSemanticRoots {
        service_reach,
        facts,
        contracts,
        values,
        boundaries,
        borrow,
        ownership,
    } = semantics;

    ControlFlowSemanticRoots::with_roots(
        remap_service_reach_roots_owned(service_reach),
        remap_fact_roots_owned(facts),
        remap_contract_roots_owned(contracts),
        remap_value_roots_owned(values),
        remap_boundary_roots_owned(boundaries),
        remap_borrow_roots_owned(borrow),
        remap_ownership_roots_owned(ownership),
    )
}

fn remap_service_reach_roots_owned(
    service_reach: SourceServiceReachRoots,
) -> ControlFlowServiceReachRoots {
    ControlFlowServiceReachRoots::with_roots(service_reach.services, service_reach.rows)
}

fn remap_fact_roots(state_graph: &StateGraph) -> ControlFlowFactRoots {
    ControlFlowFactRoots::with_roots(
        remap_proof_obligations(state_graph),
        remap_invariants(state_graph),
        state_graph.semantics.facts.dynamic_conformances.clone(),
    )
}

fn remap_fact_roots_owned(facts: SourceFactRoots) -> ControlFlowFactRoots {
    ControlFlowFactRoots::with_roots(
        facts.proof_obligations.map(remap_proof_obligation_owned),
        facts.invariants.map(remap_invariant_owned),
        facts.dynamic_conformances,
    )
}

fn remap_contract_roots(state_graph: &StateGraph) -> ControlFlowContractRoots {
    ControlFlowContractRoots::with_roots(
        remap_contract_fact_refs(state_graph),
        remap_contract_calls(state_graph),
        remap_contract_exits(state_graph),
    )
}

fn remap_contract_roots_owned(contracts: SourceContractRoots) -> ControlFlowContractRoots {
    ControlFlowContractRoots::with_roots(
        contracts.fact_refs.map(remap_contract_fact_ref_owned),
        contracts.calls.map(remap_contract_call_owned),
        contracts.exits.map(remap_contract_exit_owned),
    )
}

fn remap_value_roots(state_graph: &StateGraph) -> ControlFlowValueRoots {
    ControlFlowValueRoots::with_roots(remap_values(state_graph))
}

fn remap_value_roots_owned(values: SourceValueRoots) -> ControlFlowValueRoots {
    ControlFlowValueRoots::with_roots(values.values.map(remap_value_owned))
}

fn remap_boundary_roots(state_graph: &StateGraph) -> ControlFlowBoundaryRoots {
    ControlFlowBoundaryRoots::with_roots(remap_boundary_edges(state_graph))
}

fn remap_boundary_roots_owned(boundaries: SourceBoundaryRoots) -> ControlFlowBoundaryRoots {
    ControlFlowBoundaryRoots::with_roots(boundaries.edges.map(remap_boundary_edge_owned))
}

fn remap_borrow_roots(state_graph: &StateGraph) -> ControlFlowBorrowRoots {
    ControlFlowBorrowRoots::with_roots(
        remap_borrow_writable_roots(state_graph),
        state_graph.semantics.borrow.access_segments.clone(),
        remap_borrow_argument_accesses(state_graph),
        remap_borrow_calls(state_graph),
        remap_borrow_loans(state_graph),
        remap_borrow_activations(state_graph),
        remap_borrow_weakenings(state_graph),
    )
}

fn remap_borrow_roots_owned(borrow: SourceBorrowRoots) -> ControlFlowBorrowRoots {
    ControlFlowBorrowRoots::with_roots(
        borrow.writable_roots.map(remap_borrow_writable_root_owned),
        borrow.access_segments,
        borrow
            .argument_accesses
            .map(remap_borrow_argument_access_owned),
        borrow.calls.map(remap_borrow_call_owned),
        borrow.loans.map(remap_borrow_loan_owned),
        borrow.activations.map(remap_borrow_activation_owned),
        borrow.weakenings.map(remap_borrow_weakening_owned),
    )
}

fn remap_ownership_roots(state_graph: &StateGraph) -> ControlFlowOwnershipRoots {
    ControlFlowOwnershipRoots::with_roots(
        state_graph.semantics.ownership.segments.clone(),
        remap_permission_events(state_graph),
    )
}

fn remap_ownership_roots_owned(ownership: SourceOwnershipRoots) -> ControlFlowOwnershipRoots {
    ControlFlowOwnershipRoots::with_roots(
        ownership.segments,
        ownership.permissions.map(remap_permission_event_owned),
    )
}
