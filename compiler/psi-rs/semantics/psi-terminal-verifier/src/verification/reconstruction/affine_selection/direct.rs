//! Independent direct retained affine-root bound selection.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::affine_custody::DefinitionIndex;

mod completion;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> bool {
    requirements
        .iter()
        .chain(semantic_axioms)
        .filter_map(|fact| match fact {
            Proposition::LessOrEqual(left, right) => Some((fact, left, right)),
            _ => None,
        })
        .any(|(root_bound, root_left, root_right)| {
            [root_left, root_right]
                .into_iter()
                .filter(|root| matches!(root, ScalarTerm::Value { .. }))
                .any(|root| {
                    completion::retained(
                        context,
                        goal,
                        semantic_axioms,
                        definitions,
                        root,
                        root_bound,
                    )
                })
        })
}
