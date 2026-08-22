//! Verifier-local affine-to-cast replay for one oriented target.

use psi_core::{Proposition, PropositionContext, ScalarTerm, ScalarType};

use super::super::super::{affine_selection, cast_custody};

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    target: &ScalarTerm,
    literal: &ScalarTerm,
    target_is_right: bool,
) -> bool {
    let Some((source, first_cast)) = cast_custody::source_root(target, semantic_axioms) else {
        return false;
    };
    let ScalarType::Integer(source_type) = source.scalar_type() else {
        return false;
    };
    let Some(literal) = cast_custody::remap_integer_literal(literal, source_type) else {
        return false;
    };
    let source_goal = if target_is_right {
        Proposition::LessOrEqual(literal, source.clone())
    } else {
        Proposition::LessOrEqual(source.clone(), literal)
    };
    affine_selection::retained(
        context,
        &source_goal,
        requirements,
        &semantic_axioms[..first_cast],
    ) && cast_custody::retained_from_root(context, goal, semantic_axioms, &source, &source_goal)
}
