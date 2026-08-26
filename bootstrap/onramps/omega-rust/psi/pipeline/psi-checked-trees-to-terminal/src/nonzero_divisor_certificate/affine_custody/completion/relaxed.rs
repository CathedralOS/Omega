//! Producer-local relaxed completion of one checked affine witness.

use psi_core::{Proposition, PropositionContext};
use psi_proof_admission::{
    CheckedIntegerAffineForm, IntegerAffineWitness, ProofNode, check_certificate,
};

use super::super::relaxation;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    form: &CheckedIntegerAffineForm,
    root_bound: &ProofNode,
    witness: IntegerAffineWitness,
) -> Option<ProofNode> {
    let relaxed = relaxation::prove(goal, form, root_bound, witness)?;
    check_certificate(context, goal, assumptions, semantic_axioms, &relaxed)
        .is_ok()
        .then_some(relaxed)
}
