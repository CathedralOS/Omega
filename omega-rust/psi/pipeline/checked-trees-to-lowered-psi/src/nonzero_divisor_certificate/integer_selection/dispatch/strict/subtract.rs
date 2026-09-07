//! Select exact subtraction and positivity citations for strict integer order.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm};

use crate::nonzero_divisor_certificate::integer_evidence::projected_facts;

pub(super) fn prove(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessThan(result, original) = goal else {
        return None;
    };
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
        if measured != result || left.as_ref() != original {
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
        // Positivity uses existing exact/discrete order rules, not recursive
        // subtraction search through potentially cyclic asserted equations.
        let Some(positive) = super::prove_without_subtract(
            &Proposition::LessThan(zero, right.as_ref().clone()),
            assumptions,
            semantic_axioms,
        ) else {
            continue;
        };
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerSubtractOrder {
                difference: Box::new(fact.proof()),
                positive: Box::new(positive),
            },
        });
    }
    None
}
