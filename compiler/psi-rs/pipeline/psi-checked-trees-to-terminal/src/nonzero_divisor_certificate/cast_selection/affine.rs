//! Affine-root custody for one following exact partial-cast spine.

use psi_core::{Proposition, PropositionContext, ScalarTerm, ScalarType};
use psi_proof_kernel::ProofNode;

use super::super::{affine_selection, cast_custody};

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(left, right) = goal else {
        return None;
    };
    for (target, literal, target_is_right) in [(right, left, true), (left, right, false)] {
        if !matches!(target, ScalarTerm::Value { .. }) {
            continue;
        }
        let Some((source, first_cast)) = cast_custody::source_root(target, semantic_axioms) else {
            continue;
        };
        let ScalarType::Integer(source_type) = source.scalar_type() else {
            continue;
        };
        let Some(literal) = cast_custody::remap_integer_literal(literal, source_type) else {
            continue;
        };
        let source_goal = if target_is_right {
            Proposition::LessOrEqual(literal, source.clone())
        } else {
            Proposition::LessOrEqual(source.clone(), literal)
        };
        let Some(root_bound) = affine_selection::prove(
            context,
            &source_goal,
            assumptions,
            &semantic_axioms[..first_cast],
        ) else {
            continue;
        };
        if let Some(proof) = cast_custody::prove_from_root(
            context,
            goal,
            assumptions,
            semantic_axioms,
            &source,
            root_bound,
        ) {
            return Some(proof);
        }
    }
    None
}
