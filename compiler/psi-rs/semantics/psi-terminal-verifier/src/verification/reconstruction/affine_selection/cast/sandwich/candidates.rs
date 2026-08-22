//! Ordered affine/cast/affine candidates for independent reconstruction.

use psi_core::{Proposition, PropositionContext, ScalarTerm, ScalarType};

use super::super::super::super::affine_custody::DefinitionIndex;
use super::super::super::super::cast_custody;
use super::completion;

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
                completion::retained(
                    context,
                    goal,
                    semantic_axioms,
                    definitions,
                    root,
                    &source,
                    first_cast,
                    cast_root,
                    cast_type,
                    last_cast,
                    root_bound,
                )
            })
        })
    })
}
