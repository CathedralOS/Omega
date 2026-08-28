//! Independent inner-relation precedence for one endpoint substitution.

use psi_core::{Proposition, PropositionContext};

use super::super::super::{affine_custody::DefinitionIndex, affine_selection};
use super::super::order::{
    closed_transitive_integer_bound, retained_two_fact_transitive_integer_bound,
};

pub(super) fn retained(
    context: Option<&PropositionContext>,
    relation: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> bool {
    requirements
        .iter()
        .chain(semantic_axioms)
        .any(|fact| fact == relation || closed_transitive_integer_bound(relation, fact))
        || retained_two_fact_transitive_integer_bound(relation, requirements, semantic_axioms)
        || context.is_some_and(|context| {
            affine_selection::retained_with_definitions(
                context,
                relation,
                requirements,
                semantic_axioms,
                definitions,
            )
        })
}
