//! Verifier-local affine/cast/affine replay for one resolved root.

use psi_core::{IntegerType, Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::affine_custody::DefinitionIndex;
use super::super::super::super::{affine_custody, cast_custody};
use super::super::endpoint;

#[allow(clippy::too_many_arguments)]
pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    source: &ScalarTerm,
    first_cast: usize,
    cast_root: &ScalarTerm,
    cast_type: IntegerType,
    last_cast: usize,
    root_bound: &Proposition,
) -> bool {
    let Some(source_bound) = affine_custody::retained_mapped_to_target_before(
        context,
        semantic_axioms,
        definitions,
        root,
        source,
        first_cast,
        root_bound,
    ) else {
        return false;
    };
    let Some(cast_goal) = endpoint::remap(&source_bound, source, cast_root, cast_type) else {
        return false;
    };
    cast_custody::retained_from_root(context, &cast_goal, semantic_axioms, source, &source_bound)
        && affine_custody::retained_from_root_after(
            context,
            goal,
            semantic_axioms,
            definitions,
            cast_root,
            last_cast,
            &cast_goal,
        )
}
