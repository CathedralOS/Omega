//! Exact affine proof precedence for certificate production.

use psi_core::{Proposition, PropositionContext};
use psi_proof_admission::ProofNode;

use super::super::affine_custody::DefinitionIndex;
use super::{alias, cast, direct, literal, transitive};

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    direct::prove(context, goal, assumptions, semantic_axioms, definitions)
        .or_else(|| {
            literal::prove_landed_literal_affine_bound(
                context,
                goal,
                assumptions,
                semantic_axioms,
                definitions,
            )
        })
        .or_else(|| alias::prove_one(context, goal, assumptions, semantic_axioms, definitions))
        .or_else(|| {
            transitive::prove_transitively_reconstructed_affine_bound(
                context,
                goal,
                assumptions,
                semantic_axioms,
                definitions,
            )
        })
        .or_else(|| {
            transitive::prove_transitively_alias_substituted_affine_bound(
                context,
                goal,
                assumptions,
                semantic_axioms,
                definitions,
            )
        })
        .or_else(|| alias::prove_two(context, goal, assumptions, semantic_axioms, definitions))
        .or_else(|| cast::prove(context, goal, assumptions, semantic_axioms, definitions))
}
