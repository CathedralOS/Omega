//! Cast-specific alias transport facade.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext};

mod literal;
mod stronger;

pub(in super::super) fn prove_stronger_cast(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    stronger::prove(context, goal, assumptions, semantic_axioms)
}

pub(in super::super) fn prove_landed_literal_cast(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    literal::prove(context, goal, assumptions, semantic_axioms)
}
