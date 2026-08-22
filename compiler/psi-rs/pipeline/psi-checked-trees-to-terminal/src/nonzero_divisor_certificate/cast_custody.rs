//! Side-local custody and completion for an exact integer-cast chain.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{IntegerCastChainWitness, ProofNode, ProofRule, check_certificate};

mod chain;

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
        let Some(definition_axioms) = chain::definition_axioms(root, target, semantic_axioms)
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

pub(super) fn remap_integer_literal(
    literal: &ScalarTerm,
    target_type: psi_core::IntegerType,
) -> Option<ScalarTerm> {
    let (source_type, value) = literal.integer_value()?;
    let value = source_type.exact_cast_value_to(target_type, value)?;
    ScalarTerm::integer(target_type, value).ok()
}
