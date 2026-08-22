//! Ordered affine/cast/affine candidates for independent reconstruction.

use psi_core::{Proposition, PropositionContext, ScalarTerm, ScalarType};

use super::super::super::super::affine_custody::DefinitionIndex;
use super::completion;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> bool {
    let cast_roots = definitions.cast_roots().cloned().collect::<Vec<_>>();
    cast_roots.iter().any(|cast_root| {
        let ScalarType::Integer(cast_type) = cast_root.scalar_type() else {
            return false;
        };
        let Some((source, cast_word)) = definitions.cast_spine(cast_root) else {
            return false;
        };
        let first_cast = cast_word[0];
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
