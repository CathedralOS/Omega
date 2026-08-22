//! Independent direct-root affine/cast/affine replay.

use psi_core::{Proposition, PropositionContext, ScalarTerm, ScalarType};

use super::super::super::affine_custody::DefinitionIndex;
use super::super::super::{affine_custody, cast_custody};
use super::endpoint;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> bool {
    semantic_axioms.iter().any(|axiom| {
        let Proposition::Equal(cast_root, ScalarTerm::IntegerExactCast { .. }) = axiom else {
            return false;
        };
        let ScalarType::Integer(cast_type) = cast_root.scalar_type() else {
            return false;
        };
        let Some((source, first_cast)) = cast_custody::source_root(cast_root, semantic_axioms)
        else {
            return false;
        };
        let Some(cast_word) = cast_custody::definition_axioms(&source, cast_root, semantic_axioms)
        else {
            return false;
        };
        let Some(&last_cast) = cast_word.last() else {
            return false;
        };
        requirements.iter().any(|root_bound| {
            let Proposition::LessOrEqual(left, right) = root_bound else {
                return false;
            };
            [left, right].into_iter().any(|root| {
                if !matches!(root, ScalarTerm::Value { .. }) || root == &source {
                    return false;
                }
                let Some(source_bound) = affine_custody::retained_mapped_to_target_before(
                    context,
                    semantic_axioms,
                    definitions,
                    root,
                    &source,
                    first_cast,
                    root_bound,
                ) else {
                    return false;
                };
                let Some(cast_goal) = endpoint::remap(&source_bound, &source, cast_root, cast_type)
                else {
                    return false;
                };
                cast_custody::retained_from_root(
                    context,
                    &cast_goal,
                    semantic_axioms,
                    &source,
                    &source_bound,
                ) && affine_custody::retained_from_root_after(
                    context,
                    goal,
                    semantic_axioms,
                    definitions,
                    cast_root,
                    last_cast,
                    &cast_goal,
                )
            })
        })
    })
}
