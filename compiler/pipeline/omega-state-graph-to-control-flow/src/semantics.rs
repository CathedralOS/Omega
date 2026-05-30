use omega_control_flow::{
    ControlFlowBorrowRoots, ControlFlowBoundaryRoots, ControlFlowContractRoots,
    ControlFlowFactRoots, ControlFlowOwnershipRoots, ControlFlowSemanticRoots,
    ControlFlowValueRoots,
};
use omega_state_graph::{
    StateGraph, StateGraphBorrowRoots as SourceBorrowRoots,
    StateGraphBoundaryRoots as SourceBoundaryRoots, StateGraphContractRoots as SourceContractRoots,
    StateGraphFactRoots as SourceFactRoots, StateGraphOwnershipRoots as SourceOwnershipRoots,
    StateGraphSemanticRoots as SourceSemanticRoots, StateGraphValueRoots as SourceValueRoots,
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
use crate::ownership::{
    remap_drop_event_owned, remap_drop_events, remap_move_event_owned, remap_move_events,
};
use crate::values::{remap_value_owned, remap_values};

pub(crate) fn remap_semantic_roots(state_graph: &StateGraph) -> ControlFlowSemanticRoots {
    ControlFlowSemanticRoots::with_roots(
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
        facts,
        contracts,
        values,
        boundaries,
        borrow,
        ownership,
    } = semantics;

    ControlFlowSemanticRoots::with_roots(
        remap_fact_roots_owned(facts),
        remap_contract_roots_owned(contracts),
        remap_value_roots_owned(values),
        remap_boundary_roots_owned(boundaries),
        remap_borrow_roots_owned(borrow),
        remap_ownership_roots_owned(ownership),
    )
}

fn remap_fact_roots(state_graph: &StateGraph) -> ControlFlowFactRoots {
    ControlFlowFactRoots {
        proof_obligations: remap_proof_obligations(state_graph),
        invariants: remap_invariants(state_graph),
    }
}

fn remap_fact_roots_owned(facts: SourceFactRoots) -> ControlFlowFactRoots {
    ControlFlowFactRoots {
        proof_obligations: facts.proof_obligations.map(remap_proof_obligation_owned),
        invariants: facts.invariants.map(remap_invariant_owned),
    }
}

fn remap_contract_roots(state_graph: &StateGraph) -> ControlFlowContractRoots {
    ControlFlowContractRoots {
        fact_refs: remap_contract_fact_refs(state_graph),
        calls: remap_contract_calls(state_graph),
        exits: remap_contract_exits(state_graph),
    }
}

fn remap_contract_roots_owned(contracts: SourceContractRoots) -> ControlFlowContractRoots {
    ControlFlowContractRoots {
        fact_refs: contracts.fact_refs.map(remap_contract_fact_ref_owned),
        calls: contracts.calls.map(remap_contract_call_owned),
        exits: contracts.exits.map(remap_contract_exit_owned),
    }
}

fn remap_value_roots(state_graph: &StateGraph) -> ControlFlowValueRoots {
    ControlFlowValueRoots {
        values: remap_values(state_graph),
    }
}

fn remap_value_roots_owned(values: SourceValueRoots) -> ControlFlowValueRoots {
    ControlFlowValueRoots {
        values: values.values.map(remap_value_owned),
    }
}

fn remap_boundary_roots(state_graph: &StateGraph) -> ControlFlowBoundaryRoots {
    ControlFlowBoundaryRoots {
        edges: remap_boundary_edges(state_graph),
    }
}

fn remap_boundary_roots_owned(boundaries: SourceBoundaryRoots) -> ControlFlowBoundaryRoots {
    ControlFlowBoundaryRoots {
        edges: boundaries.edges.map(remap_boundary_edge_owned),
    }
}

fn remap_borrow_roots(state_graph: &StateGraph) -> ControlFlowBorrowRoots {
    ControlFlowBorrowRoots {
        writable_roots: remap_borrow_writable_roots(state_graph),
        access_segments: state_graph.semantics.borrow.access_segments.clone(),
        argument_accesses: remap_borrow_argument_accesses(state_graph),
        calls: remap_borrow_calls(state_graph),
        loans: remap_borrow_loans(state_graph),
        activations: remap_borrow_activations(state_graph),
        weakenings: remap_borrow_weakenings(state_graph),
    }
}

fn remap_borrow_roots_owned(borrow: SourceBorrowRoots) -> ControlFlowBorrowRoots {
    ControlFlowBorrowRoots {
        writable_roots: borrow.writable_roots.map(remap_borrow_writable_root_owned),
        access_segments: borrow.access_segments,
        argument_accesses: borrow
            .argument_accesses
            .map(remap_borrow_argument_access_owned),
        calls: borrow.calls.map(remap_borrow_call_owned),
        loans: borrow.loans.map(remap_borrow_loan_owned),
        activations: borrow.activations.map(remap_borrow_activation_owned),
        weakenings: borrow.weakenings.map(remap_borrow_weakening_owned),
    }
}

fn remap_ownership_roots(state_graph: &StateGraph) -> ControlFlowOwnershipRoots {
    ControlFlowOwnershipRoots {
        segments: state_graph.semantics.ownership.segments.clone(),
        moves: remap_move_events(state_graph),
        drops: remap_drop_events(state_graph),
    }
}

fn remap_ownership_roots_owned(ownership: SourceOwnershipRoots) -> ControlFlowOwnershipRoots {
    ControlFlowOwnershipRoots {
        segments: ownership.segments,
        moves: ownership.moves.map(remap_move_event_owned),
        drops: ownership.drops.map(remap_drop_event_owned),
    }
}
