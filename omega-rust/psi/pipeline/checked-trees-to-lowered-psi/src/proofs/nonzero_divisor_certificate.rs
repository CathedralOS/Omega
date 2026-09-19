//! Focused certificates for canonical fixed-integer order propositions.
//!
//! This producer deliberately consumes only machine requirements and facts
//! reconstructed before the operation site. It never sees the operation's own
//! result equation, so the certificate cannot justify the operation with a
//! fact produced by that same operation.

use std::collections::BTreeSet;

use proof_admission::{ProofNode, accept_certificate_with_machine_parameters};
use semantic_vocabulary::{Proposition, PropositionContext, ValueId};

mod affine_custody;
mod affine_selection;
mod alias_transport;
mod cast_custody;
mod cast_selection;
mod certificate_entry;
mod integer_evidence;
mod integer_selection;
mod predicate_conversion;
mod value_transport;

/// Build the recursive certificate shape shared by canonical integer goals.
///
/// This is deliberately not an affine or interval analyzer. It composes exact
/// prior citations and the small checked order rules; producers for richer
/// families must still materialize proofs of the atomic leaves.
#[cfg(test)]
pub(crate) fn prove_canonical_integer_proposition(
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

/// Produce a kernel-checked proof through relaxed integer selection.
///
/// This is the last resort for operation obligations the canonical
/// certificate cannot reach: bounds whose operands meet cited facts only
/// through equality/definition chains. The relaxed search keeps every
/// canonical producer ahead of the closure at each goal, so ordered
/// definition-word custody is unchanged inside the canonical producers, and
/// conditional facts still discharge only inside their own premise scope.
/// The returned proof passes the same certificate acceptance as canonical
/// evidence before it is returned.
pub(crate) fn produce_relaxed_integer_proof(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<ProofNode> {
    let proof = integer_selection::prove_relaxed_with_machine_parameters(
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
    )?;
    accept_certificate_with_machine_parameters(
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
        &proof,
    )
    .ok()?;
    Some(proof)
}

#[cfg(test)]
mod tests;
