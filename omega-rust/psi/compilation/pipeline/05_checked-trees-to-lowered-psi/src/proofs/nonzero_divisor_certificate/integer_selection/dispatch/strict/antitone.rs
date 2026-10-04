//! Order two exact differences through their subtrahends, and a sum above
//! its original operand.
//!
//! A climbing subject ranks as its distance below a fixed minuend
//! (`MAX - lower`): one edge's successor difference is smaller exactly when
//! its subtrahend is larger. The subtrahend order is usually an exact
//! `lower + k` step. Both legs cite one definition each; endpoint transport
//! goes through the session's exact equalities, never by renaming.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{IntegerValue, Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::affine_custody::DefinitionIndex;
use super::super::super::exact;
use crate::proofs::nonzero_divisor_certificate::integer_evidence::projected_facts;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    _definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let Proposition::LessThan(goal_left, goal_right) = goal else {
        return None;
    };
    let session = exact::session(context, assumptions, semantic_axioms);
    let facts = projected_facts(assumptions, semantic_axioms);
    let reaches = |value: &ScalarTerm, target: &ScalarTerm| {
        value == target || session.prove_equality(value, target).is_some()
    };
    let differences = facts
        .iter()
        .filter_map(|fact| match fact.proposition {
            Proposition::Equal(result, ScalarTerm::ExactIntegerSubtract { left, right, .. }) => {
                Some((fact, result, left.as_ref(), right.as_ref()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    for (smaller_fact, smaller, minuend, large) in &differences {
        if !reaches(smaller, goal_left) {
            continue;
        }
        for (larger_fact, larger, larger_minuend, small) in &differences {
            if larger_minuend != minuend || !reaches(larger, goal_right) {
                continue;
            }
            let order_goal = Proposition::LessThan((*small).clone(), (*large).clone());
            let Some(order) =
                super::prove_without_subtract(context, &order_goal, assumptions, semantic_axioms)
                    .or_else(|| increase(context, &order_goal, assumptions, semantic_axioms))
            else {
                continue;
            };
            let antitone = ProofNode {
                conclusion: Proposition::LessThan((*smaller).clone(), (*larger).clone()),
                rule: ProofRule::IntegerSubtractAntitone {
                    smaller: Box::new(smaller_fact.proof()),
                    larger: Box::new(larger_fact.proof()),
                    order: Box::new(order),
                },
            };
            if let Some(proof) = super::complete(&session, goal, antitone) {
                return Some(proof);
            }
        }
    }
    None
}

/// `original < result` from a cited exact `result = original + increment`
/// and a proved positive increment, transported onto the goal's endpoints.
pub(super) fn increase(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessThan(goal_left, goal_right) = goal else {
        return None;
    };
    let session = exact::session(context, assumptions, semantic_axioms);
    for fact in projected_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(
            result,
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            },
        ) = fact.proposition
        else {
            continue;
        };
        let reaches = |value: &ScalarTerm, target: &ScalarTerm| {
            value == target || session.prove_equality(value, target).is_some()
        };
        if !reaches(left, goal_left) || !reaches(result, goal_right) {
            continue;
        }
        let zero = ScalarTerm::integer(
            *scalar_type,
            match scalar_type.sign() {
                semantic_vocabulary::IntegerSign::Signed => IntegerValue::Signed(0),
                semantic_vocabulary::IntegerSign::Unsigned => IntegerValue::Unsigned(0),
            },
        )
        .ok()?;
        let Some(positive) = super::prove_without_subtract(
            context,
            &Proposition::LessThan(zero, right.as_ref().clone()),
            assumptions,
            semantic_axioms,
        ) else {
            continue;
        };
        let increase = ProofNode {
            conclusion: Proposition::LessThan(left.as_ref().clone(), result.clone()),
            rule: ProofRule::IntegerAddOrder {
                sum: Box::new(fact.proof()),
                positive: Box::new(positive),
            },
        };
        if let Some(proof) = super::complete(&session, goal, increase) {
            return Some(proof);
        }
    }
    None
}
