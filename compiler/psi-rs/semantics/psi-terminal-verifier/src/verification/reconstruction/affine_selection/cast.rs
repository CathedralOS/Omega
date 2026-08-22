//! Independent direct cast-root replay for one following affine word.

use psi_core::{Proposition, PropositionContext, ScalarTerm, ScalarType};

use super::super::affine_custody::DefinitionIndex;
use super::super::{affine_custody, cast_custody};

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
        let Some((source, _)) = cast_custody::source_root(cast_root, semantic_axioms) else {
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
            let Some(cast_goal) = remap_direct_bound(root_bound, &source, cast_root, cast_type)
            else {
                return false;
            };
            cast_custody::retained_from_root(
                context,
                &cast_goal,
                semantic_axioms,
                &source,
                root_bound,
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
}

fn remap_direct_bound(
    bound: &Proposition,
    source: &ScalarTerm,
    cast_root: &ScalarTerm,
    cast_type: psi_core::IntegerType,
) -> Option<Proposition> {
    let Proposition::LessOrEqual(left, right) = bound else {
        return None;
    };
    if left == source {
        Some(Proposition::LessOrEqual(
            cast_root.clone(),
            cast_custody::remap_integer_literal(right, cast_type)?,
        ))
    } else if right == source {
        Some(Proposition::LessOrEqual(
            cast_custody::remap_integer_literal(left, cast_type)?,
            cast_root.clone(),
        ))
    } else {
        None
    }
}
