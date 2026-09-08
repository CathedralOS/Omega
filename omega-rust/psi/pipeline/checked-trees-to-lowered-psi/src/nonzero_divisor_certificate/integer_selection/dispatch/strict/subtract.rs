//! Select exact subtraction and positivity citations for strict integer order.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm};

use crate::nonzero_divisor_certificate::integer_evidence::projected_facts;

pub(super) fn prove(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessThan(_, _) = goal else {
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
        if let Some(proof) = super::complete(goal, decrease, assumptions, semantic_axioms) {
            return Some(proof);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use proof_admission::check_certificate;
    use semantic_vocabulary::{IntegerSign, IntegerType, PropositionContext, ScalarType, ValueId};

    #[test]
    fn decrease_transports_both_endpoints_only_through_cited_equalities() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let scalar_type = ScalarType::Integer(integer_type);
        let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
        let literal =
            |number| ScalarTerm::integer(integer_type, IntegerValue::Unsigned(number)).unwrap();
        let context = PropositionContext::from_value_types(
            (1..=6).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
        )
        .unwrap();
        let axioms = [
            Proposition::Equal(
                value(1),
                ScalarTerm::exact_integer_subtract(integer_type, value(2), value(3)).unwrap(),
            ),
            Proposition::Equal(value(3), literal(1)),
            Proposition::Equal(value(4), value(1)),
            Proposition::Equal(value(2), value(6)),
            Proposition::Equal(value(6), value(5)),
        ];
        let goal = Proposition::LessThan(value(4), value(5));
        let proof = prove(&goal, &[], &axioms).expect("subtraction plus both exact alias chains");
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        for missing in 0..axioms.len() {
            let mut incomplete = axioms.to_vec();
            incomplete.remove(missing);
            assert!(
                prove(&goal, &[], &incomplete).is_none(),
                "missing citation {missing}"
            );
            assert!(check_certificate(&context, &goal, &[], &incomplete, &proof).is_err());
        }
        let mut zero = axioms.clone();
        zero[1] = Proposition::Equal(value(3), literal(0));
        assert!(
            prove(&goal, &[], &zero).is_none(),
            "zero decrement cannot prove strict descent"
        );
        assert!(check_certificate(&context, &goal, &[], &zero, &proof).is_err());
        assert!(prove(&Proposition::LessThan(value(5), value(4)), &[], &axioms).is_none());
    }
}
