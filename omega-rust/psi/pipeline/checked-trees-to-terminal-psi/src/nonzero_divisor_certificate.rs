//! Focused certificates for canonical fixed-integer order propositions.
//!
//! This producer deliberately consumes only machine requirements and facts
//! reconstructed before the operation site. It never sees the operation's own
//! result equation, so the certificate cannot justify the operation with a
//! fact produced by that same operation.

use std::collections::BTreeSet;

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext, ValueId};

mod affine_custody;
mod affine_selection;
mod alias_transport;
mod cast_custody;
mod cast_selection;
mod certificate_entry;
mod integer_evidence;
mod integer_selection;

/// Build the recursive certificate shape shared by canonical integer goals.
///
/// This is deliberately not an affine or interval analyzer. It composes exact
/// prior citations and the small checked order rules; producers for richer
/// families must still materialize proofs of the atomic leaves.
#[cfg(test)]
pub(super) fn prove_canonical_integer_proposition(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    certificate_entry::prove(context, goal, assumptions, semantic_axioms)
}

/// Produce a kernel-checked proof for the canonical integer proposition subset.
///
/// This is a proof-search boundary, not an admission boundary. The returned
/// proof has already passed the proof kernel, and consumers must still submit
/// it through their normal artifact verification path.
pub fn produce_checked_canonical_integer_proof(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<ProofNode> {
    certificate_entry::prove_with_machine_parameters(
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
    )
}

#[cfg(test)]
mod tests;
