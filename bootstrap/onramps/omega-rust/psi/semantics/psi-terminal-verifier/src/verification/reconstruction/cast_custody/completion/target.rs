//! Independent replay of one ordered exact-cast target.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_admission::{
    IntegerCastChainWitness, check_integer_cast_bound_conversion, check_integer_cast_chain_witness,
};

use super::super::chain;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    root_bound: &Proposition,
    target: &ScalarTerm,
) -> bool {
    let Some(definition_axioms) = chain::definition_axioms(root, target, semantic_axioms) else {
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
    .is_ok_and(|chain| check_integer_cast_bound_conversion(&chain, root_bound, goal).is_ok())
}
