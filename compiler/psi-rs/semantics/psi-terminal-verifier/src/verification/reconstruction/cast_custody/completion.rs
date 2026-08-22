//! Independent exact integer-cast witness and bound-conversion replay.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{
    IntegerCastChainWitness, check_integer_cast_bound_conversion, check_integer_cast_chain_witness,
};

use super::chain;

pub(in super::super) fn retained_from_root(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
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
            let Some(definition_axioms) = chain::definition_axioms(root, target, semantic_axioms)
            else {
                return false;
            };
            check_integer_cast_chain_witness(
                context,
                semantic_axioms,
                &IntegerCastChainWitness {
                    root: root.clone(),
                    target: target.clone(),
                    definition_axioms,
                },
            )
            .is_ok_and(|chain| {
                check_integer_cast_bound_conversion(&chain, root_bound, goal).is_ok()
            })
        })
}
