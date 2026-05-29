mod conversions;

use omega_control_flow::{InvariantFact, ProofObligationFact};
use omega_core::arena::Arena;
use omega_state_graph::StateGraph;

pub(crate) use conversions::{remap_invariant_owned, remap_proof_obligation_owned};

pub(crate) fn remap_proof_obligations(state_graph: &StateGraph) -> Arena<ProofObligationFact> {
    let mut obligations = Arena::with_capacity(state_graph.semantics.proof_obligations.len());

    for (_, obligation) in state_graph.semantics.proof_obligations.iter() {
        obligations.append(remap_proof_obligation_owned(obligation.clone()));
    }

    obligations
}

pub(crate) fn remap_invariants(state_graph: &StateGraph) -> Arena<InvariantFact> {
    let mut invariants = Arena::with_capacity(state_graph.semantics.invariants.len());

    for (_, invariant) in state_graph.semantics.invariants.iter() {
        invariants.append(remap_invariant_owned(invariant.clone()));
    }

    invariants
}
