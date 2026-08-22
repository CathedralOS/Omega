//! Independent affine-witness completion for obligation reconstruction.
//!
//! Evidence selection remains in the parent verifier. This module owns the
//! bounded witness frontier, exact mapped bound, and closed relaxation replay.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{
    IntegerAffineWitness, check_integer_affine_bound_conversion, check_integer_affine_witness,
};

mod definition_index;
mod frontier;
mod relaxation;

pub(super) use definition_index::DefinitionIndex;

pub(super) fn retained_from_root(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    root_bound: &Proposition,
) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    [goal_left, goal_right]
        .into_iter()
        .filter(|target| matches!(target, ScalarTerm::Value { .. }))
        .any(|target| {
            frontier::definition_words(context, semantic_axioms, definitions, root)
                .into_iter()
                .any(|definition_axioms| {
                    let witness = IntegerAffineWitness {
                        root: root.clone(),
                        target: target.clone(),
                        definition_axioms,
                    };
                    check_integer_affine_witness(context, semantic_axioms, &witness).is_ok_and(
                        |form| {
                            check_integer_affine_bound_conversion(&form, root_bound, goal).is_ok()
                                || relaxation::retained(&form, root_bound, goal)
                        },
                    )
                })
        })
}
