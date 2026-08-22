//! Side-local custody and completion for an exact integer-cast chain.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{IntegerCastChainWitness, ProofNode, ProofRule, check_certificate};

pub(super) fn prove_from_root(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    root_bound: ProofNode,
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    for target in [goal_left, goal_right]
        .into_iter()
        .filter(|target| matches!(target, ScalarTerm::Value { .. }))
    {
        let Some(definition_axioms) =
            exact_cast_chain_definition_axioms(root, target, semantic_axioms)
        else {
            continue;
        };
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerCastBound {
                root_bound: Box::new(root_bound.clone()),
                witness: IntegerCastChainWitness {
                    root: root.clone(),
                    target: target.clone(),
                    definition_axioms,
                },
            },
        };
        if check_certificate(context, goal, assumptions, semantic_axioms, &proof).is_ok() {
            return Some(proof);
        }
    }
    None
}

/// Follow the exact SSA definition spine backward from `target` to `root`.
///
/// This is intentionally not graph search: every reached target must have one
/// exact-cast definition, and the resulting source-ordered word must already be
/// canonical in the semantic ledger.
fn exact_cast_chain_definition_axioms(
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

pub(super) fn remap_integer_literal(
    literal: &ScalarTerm,
    target_type: psi_core::IntegerType,
) -> Option<ScalarTerm> {
    let (source_type, value) = literal.integer_value()?;
    let value = source_type.exact_cast_value_to(target_type, value)?;
    ScalarTerm::integer(target_type, value).ok()
}
