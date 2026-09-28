//! Select exact subtraction and positivity citations for strict integer order.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{IntegerValue, Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::affine_custody::DefinitionIndex;
use super::super::super::{bound, exact};
use crate::proofs::nonzero_divisor_certificate::integer_evidence::projected_facts;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let Proposition::LessThan(goal_left, goal_right) = goal else {
        return None;
    };
    // Each subtract candidate asks endpoint equalities against this same
    // unchanged roster; resolve the session once for all of them.
    let session = exact::session(context, assumptions, semantic_axioms);
    for fact in projected_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(
            measured,
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            },
        ) = fact.proposition
        else {
            continue;
        };
        let zero = ScalarTerm::integer(
            *scalar_type,
            match scalar_type.sign() {
                semantic_vocabulary::IntegerSign::Signed => IntegerValue::Signed(0),
                semantic_vocabulary::IntegerSign::Unsigned => IntegerValue::Unsigned(0),
            },
        )
        .ok()?;
        // Both completions below conclude `measured < _` and reach the goal's
        // left endpoint only through a session equality. A difference that
        // cannot reach it never completes, so skip its positivity and minuend
        // searches rather than discard them after the fact.
        if measured != goal_left
            && session
                .prove(&Proposition::Equal(measured.clone(), goal_left.clone()))
                .is_none()
        {
            continue;
        }
        // Positivity uses existing exact/discrete order rules, not recursive
        // subtraction search through potentially cyclic asserted equations.
        let Some(positive) = super::prove_without_subtract(
            context,
            &Proposition::LessThan(zero, right.as_ref().clone()),
            assumptions,
            semantic_axioms,
        ) else {
            continue;
        };
        let decrease = ProofNode {
            conclusion: Proposition::LessThan(measured.clone(), left.as_ref().clone()),
            rule: ProofRule::IntegerSubtractOrder {
                difference: Box::new(fact.proof()),
                positive: Box::new(positive),
            },
        };
        // Successor bindings preserve values through cited SSA equalities.
        // Prove the actual subtraction first, then transport its two endpoints
        // with the existing checked substitution rule, never by renaming them.
        if let Some(proof) = super::complete(&session, goal, decrease.clone()) {
            return Some(proof);
        }
        // The same decrease still bounds the difference when the minuend
        // reaches the goal's right endpoint on its own evidence:
        // `measured < left` joined with an independently proved
        // `left <= goal_right` is the checked mixed transitivity step.
        let extension = Proposition::LessOrEqual(left.as_ref().clone(), goal_right.clone());
        let Some(bound) = bound::prove(
            context,
            &extension,
            assumptions,
            semantic_axioms,
            definitions,
        ) else {
            continue;
        };
        let chained = ProofNode {
            conclusion: Proposition::LessThan(measured.clone(), goal_right.clone()),
            rule: ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle: Box::new(decrease),
                middle_to_right: Box::new(bound),
            },
        };
        if let Some(proof) = super::complete(&session, goal, chained) {
            return Some(proof);
        }
    }
    None
}

#[cfg(test)]
mod tests {}
