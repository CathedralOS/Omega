//! Focused certificates for canonical fixed-integer order propositions.
//!
//! This producer deliberately consumes only machine requirements and facts
//! reconstructed before the operation site. It never sees the operation's own
//! result equation, so the certificate cannot justify the operation with a
//! fact produced by that same operation.

use psi_core::{Proposition, PropositionContext};
use psi_proof_admission::ProofNode;

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
pub(super) fn prove_canonical_integer_proposition(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    certificate_entry::prove(context, goal, assumptions, semantic_axioms)
}

#[cfg(test)]
mod tests;
