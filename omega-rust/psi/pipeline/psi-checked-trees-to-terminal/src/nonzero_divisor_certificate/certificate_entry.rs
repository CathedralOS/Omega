//! Fail-closed entry from untrusted integer proof selection to the kernel.

use std::collections::BTreeSet;

use psi_core::{Proposition, PropositionContext, ValueId};
use psi_proof_admission::{ProofNode, accept_certificate_with_machine_parameters};

#[cfg(test)]
use psi_proof_admission::check_certificate;

use super::integer_selection;

#[cfg(test)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let proof = integer_selection::build(context, goal, assumptions, semantic_axioms)?;
    check_certificate(context, goal, assumptions, semantic_axioms, &proof)
        .is_ok()
        .then_some(proof)
}

pub(super) fn prove_with_machine_parameters(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<ProofNode> {
    let proof = integer_selection::build_with_machine_parameters(
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
    .is_ok()
    .then_some(proof)
}
