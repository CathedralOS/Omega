//! Verifier-local cast-to-affine replay for one retained root bound.

use psi_core::{IntegerType, Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::affine_custody::DefinitionIndex;
use super::super::super::super::{affine_custody, cast_custody};
use super::super::endpoint;

#[allow(clippy::too_many_arguments)]
pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    source: &ScalarTerm,
    cast_root: &ScalarTerm,
    cast_type: IntegerType,
    last_cast: usize,
    root_bound: &Proposition,
) -> bool {
    let Some(cast_goal) = endpoint::remap(root_bound, source, cast_root, cast_type) else {
        return false;
    };
    cast_custody::retained_from_root(context, &cast_goal, semantic_axioms, source, root_bound)
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
