//! Ordered atomic integer-bound proof selection.

use psi_core::{Proposition, PropositionContext};
use psi_proof_admission::ProofNode;

use super::super::{affine_custody::DefinitionIndex, affine_selection, cast_selection};
use super::{order, substitution};

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    if let Some(proof) =
        order::prove_exact_or_closed_transitive_integer_bound(goal, assumptions, semantic_axioms)
    {
        return Some(proof);
    }

    if let Some(proof) =
        order::prove_two_fact_transitive_integer_bound(goal, assumptions, semantic_axioms)
    {
        return Some(proof);
    }

    if let Some(proof) =
        substitution::prove(context, goal, assumptions, semantic_axioms, definitions)
    {
        return Some(proof);
    }

    cast_selection::prove(context, goal, assumptions, semantic_axioms).or_else(|| {
        affine_selection::prove_with_definitions(
            context,
            goal,
            assumptions,
            semantic_axioms,
            definitions,
        )
    })
}
