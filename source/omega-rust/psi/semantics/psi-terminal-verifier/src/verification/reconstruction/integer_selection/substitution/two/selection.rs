//! Verifier-local fixed two-equality affine endpoint selection.

use psi_core::{Proposition, PropositionContext};

use super::super::super::super::affine_custody::DefinitionIndex;

mod aliases;
mod completion;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    let facts = || requirements.iter().chain(semantic_axioms);
    facts()
        .filter_map(|outer_equality| match outer_equality {
            Proposition::Equal(left, right) => Some((outer_equality, left, right)),
            _ => None,
        })
        .any(|(outer_equality, outer_left, outer_right)| {
            [(outer_left, outer_right), (outer_right, outer_left)]
                .into_iter()
                .filter_map(|(old, middle_alias)| {
                    aliases::outer(goal_left, goal_right, old, middle_alias)
                        .map(|endpoint| (old, middle_alias, endpoint))
                })
                .any(|(old, middle_alias, endpoint)| {
                    facts()
                        .filter(|inner_equality| !std::ptr::eq(outer_equality, *inner_equality))
                        .filter_map(|inner_equality| match inner_equality {
                            Proposition::Equal(left, right) => Some((left, right)),
                            _ => None,
                        })
                        .any(|(inner_left, inner_right)| {
                            let Some(target_alias) =
                                aliases::inner(old, middle_alias, inner_left, inner_right)
                            else {
                                return false;
                            };
                            completion::retained(
                                context,
                                goal_left,
                                goal_right,
                                target_alias,
                                endpoint,
                                requirements,
                                semantic_axioms,
                                definitions,
                            )
                        })
                })
        })
}
