//! Side-local reconstruction of exact integer-cast chain custody.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{
    IntegerCastChainWitness, check_integer_cast_bound_conversion, check_integer_cast_chain_witness,
};

pub(super) fn retained_from_root(
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
            let Some(definition_axioms) =
                retained_exact_cast_chain_axioms(root, target, semantic_axioms)
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

/// Independently reconstruct the unique exact-cast SSA definition spine.
///
/// This follows one definition per reached target and never explores alternate
/// paths or permutations. The proof-kernel witness checker still owns all cast
/// legality, continuity, and carrier validation.
fn retained_exact_cast_chain_axioms(
    root: &ScalarTerm,
    target: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Option<Vec<usize>> {
    if root == target {
        return None;
    }
    let mut current = target.clone();
    let mut reversed = Vec::new();
    while &current != root {
        if reversed.len() >= semantic_axioms.len() {
            return None;
        }
        let mut definitions = semantic_axioms
            .iter()
            .enumerate()
            .filter_map(|(index, axiom)| {
                let Proposition::Equal(output, ScalarTerm::IntegerExactCast { operand, .. }) =
                    axiom
                else {
                    return None;
                };
                (output == &current).then(|| (index, operand.as_ref().clone()))
            });
        let (index, operand) = definitions.next()?;
        if definitions.next().is_some() || reversed.contains(&index) {
            return None;
        }
        reversed.push(index);
        current = operand;
    }
    reversed.reverse();
    reversed
        .windows(2)
        .all(|pair| pair[0] < pair[1])
        .then_some(reversed)
}
