//! Producer-local completion of one ordered exact-cast target.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_admission::{
    IntegerCastChainWitness, ProofNode, ProofRule, check_certificate,
    check_integer_cast_chain_witness, integer_cast_truth_bounds,
};

use super::super::chain;
use crate::nonzero_divisor_certificate::integer_evidence::closed_integer_relation;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    root_bound: &ProofNode,
    target: &ScalarTerm,
) -> Option<ProofNode> {
    let definition_axioms = chain::definition_axioms(root, target, semantic_axioms)?;
    let witness = IntegerCastChainWitness {
        root: root.clone(),
        target: target.clone(),
        definition_axioms,
    };
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerCastBound {
            root_bound: Box::new(root_bound.clone()),
            witness: witness.clone(),
        },
    };
    if check_certificate(context, goal, assumptions, semantic_axioms, &proof).is_ok() {
        return Some(proof);
    }
    if root_bound.conclusion != Proposition::Truth {
        return None;
    }
    let chain = check_integer_cast_chain_witness(context, semantic_axioms, &witness).ok()?;
    for mapped in integer_cast_truth_bounds(&chain).ok()? {
        let mapped_proof = ProofNode {
            conclusion: mapped.clone(),
            rule: ProofRule::IntegerCastBound {
                root_bound: Box::new(root_bound.clone()),
                witness: witness.clone(),
            },
        };
        if let Some(relaxed) = relax(goal, mapped_proof)
            && check_certificate(context, goal, assumptions, semantic_axioms, &relaxed).is_ok()
        {
            return Some(relaxed);
        }
    }
    None
}

fn relax(goal: &Proposition, mapped: ProofNode) -> Option<ProofNode> {
    let (
        Proposition::LessOrEqual(goal_left, goal_right),
        Proposition::LessOrEqual(mapped_left, mapped_right),
    ) = (goal, &mapped.conclusion)
    else {
        return None;
    };
    if goal_left == mapped_left {
        let tail = closed_integer_relation(Proposition::LessOrEqual(
            mapped_right.clone(),
            goal_right.clone(),
        ))?;
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(mapped),
                middle_less_or_equal_right: Box::new(tail),
            },
        });
    }
    if goal_right == mapped_right {
        let head = closed_integer_relation(Proposition::LessOrEqual(
            goal_left.clone(),
            mapped_left.clone(),
        ))?;
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(head),
                middle_less_or_equal_right: Box::new(mapped),
            },
        });
    }
    None
}
