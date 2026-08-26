//! Fixed two-equality affine endpoint substitution.

use psi_core::{Proposition, PropositionContext};
use psi_proof_admission::ProofNode;

use super::super::super::affine_custody::DefinitionIndex;

mod selection;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    selection::prove(context, goal, assumptions, semantic_axioms, definitions)
}
