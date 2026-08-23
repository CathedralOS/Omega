mod conversions;

use omega_control_flow::{InvariantFact, ProofObligationFact};
use omega_state_graph::StateGraph;
use psi_arena::Arena;

use crate::arena_remap::remap_arena;

pub(crate) use conversions::{remap_invariant_owned, remap_proof_obligation_owned};

pub(crate) fn remap_proof_obligations(state_graph: &StateGraph) -> Arena<ProofObligationFact> {
    remap_arena(
        &state_graph.semantics.facts.proof_obligations,
        remap_proof_obligation_owned,
    )
}

pub(crate) fn remap_invariants(state_graph: &StateGraph) -> Arena<InvariantFact> {
    remap_arena(
        &state_graph.semantics.facts.invariants,
        remap_invariant_owned,
    )
}
