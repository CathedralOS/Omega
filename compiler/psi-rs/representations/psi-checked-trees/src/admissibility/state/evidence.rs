use crate::{FlowCallFact, FlowExitFact, FlowFacts, FlowStateFact, FlowStatementFact};

use super::super::helpers::borrow_constraint_count;

pub(super) fn state_borrow_evidence_count(
    flow: &FlowFacts,
    state: &FlowStateFact,
    statements: &[FlowStatementFact],
    calls: &[FlowCallFact],
    exits: &[FlowExitFact],
) -> usize {
    borrow_constraint_count(flow, state.entry_constraints)
        + state.writable_roots.len()
        + statements
            .iter()
            .map(|statement| borrow_constraint_count(flow, statement.entry_constraints))
            .sum::<usize>()
        + calls
            .iter()
            .map(|call| {
                call.accesses.len()
                    + borrow_constraint_count(flow, call.entry_constraints)
                    + borrow_constraint_count(flow, call.requires_constraints)
                    + borrow_constraint_count(flow, call.exit_constraints)
            })
            .sum::<usize>()
        + exits
            .iter()
            .map(|exit| {
                borrow_constraint_count(flow, exit.entry_constraints)
                    + borrow_constraint_count(flow, exit.ensures_constraints)
            })
            .sum::<usize>()
}

pub(super) fn state_proof_evidence_count(calls: &[FlowCallFact], exits: &[FlowExitFact]) -> usize {
    calls
        .iter()
        .map(|call| call.requires.len() + call.ensures.len())
        .sum::<usize>()
        + exits.iter().map(|exit| exit.ensures.len()).sum::<usize>()
}

pub(super) fn state_boundary_evidence_count(
    state: &FlowStateFact,
    calls: &[FlowCallFact],
) -> usize {
    state.boundary_edges.len()
        + calls
            .iter()
            .map(|call| call.boundary_edges.len())
            .sum::<usize>()
}
