//! Select exact subtraction and positivity citations for strict integer order.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{IntegerValue, Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::affine_custody::DefinitionIndex;
use super::super::super::bound;
use crate::proofs::nonzero_divisor_certificate::integer_evidence::projected_facts;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let Proposition::LessThan(_, goal_right) = goal else {
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
        if let Some(proof) = super::complete(goal, decrease.clone(), assumptions, semantic_axioms) {
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
        if let Some(proof) = super::complete(goal, chained, assumptions, semantic_axioms) {
            return Some(proof);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{DefinitionIndex, IntegerValue, Proposition, ScalarTerm, prove};
    use proof_admission::check_certificate;
    use semantic_vocabulary::{IntegerSign, IntegerType, PropositionContext, ScalarType, ValueId};

    fn prove_goal(
        context: &PropositionContext,
        goal: &Proposition,
        assumptions: &[Proposition],
        semantic_axioms: &[Proposition],
    ) -> Option<proof_admission::ProofNode> {
        prove(
            context,
            goal,
            assumptions,
            semantic_axioms,
            &mut DefinitionIndex::new(semantic_axioms),
        )
    }

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
        let proof = prove_goal(&context, &goal, &[], &axioms)
            .expect("subtraction plus both exact alias chains");
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        for missing in 0..axioms.len() {
            let mut incomplete = axioms.to_vec();
            incomplete.remove(missing);
            assert!(
                prove_goal(&context, &goal, &[], &incomplete).is_none(),
                "missing citation {missing}"
            );
            assert!(check_certificate(&context, &goal, &[], &incomplete, &proof).is_err());
        }
        let mut zero = axioms.clone();
        zero[1] = Proposition::Equal(value(3), literal(0));
        assert!(
            prove_goal(&context, &goal, &[], &zero).is_none(),
            "zero decrement cannot prove strict descent"
        );
        assert!(check_certificate(&context, &goal, &[], &zero, &proof).is_err());
        assert!(
            prove_goal(
                &context,
                &Proposition::LessThan(value(5), value(4)),
                &[],
                &axioms
            )
            .is_none()
        );
    }

    #[test]
    fn decrease_joins_the_minuend_bound_through_mixed_transitivity() {
        // `remaining = count - index` with `0 < index` proves
        // `remaining < count`; a cited `count <= 11` and the closed
        // `11 <= bound` then bound `remaining` without any literal on the
        // difference itself. This is the dependent-subtract call leg: the
        // decrement's strictness arrives through an equality chain, the
        // minuend's ceiling through a cited range.
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        let scalar_type = ScalarType::Integer(integer_type);
        let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
        let literal =
            |number| ScalarTerm::integer(integer_type, IntegerValue::Unsigned(number)).unwrap();
        let context = PropositionContext::from_value_types(
            (1..=4).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
        )
        .unwrap();
        let axioms = [
            Proposition::Equal(value(2), literal(6)),
            Proposition::Equal(value(1), value(2)),
            Proposition::LessOrEqual(value(3), literal(11)),
            Proposition::Equal(
                value(4),
                ScalarTerm::exact_integer_subtract(integer_type, value(3), value(1)).unwrap(),
            ),
        ];
        let goal = Proposition::LessThan(value(4), literal(2_147_483_647));
        let proof = prove_goal(&context, &goal, &[], &axioms)
            .expect("decrease joins the minuend's cited bound");
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        for missing in 0..axioms.len() {
            let mut incomplete = axioms.to_vec();
            incomplete.remove(missing);
            assert!(
                prove_goal(&context, &goal, &[], &incomplete).is_none(),
                "missing citation {missing}"
            );
            assert!(check_certificate(&context, &goal, &[], &incomplete, &proof).is_err());
        }
        // A zero decrement collapses the strict edge: `remaining` can equal
        // `count`, so the bound is no longer a theorem of these facts.
        let mut zero = axioms.clone();
        zero[0] = Proposition::Equal(value(2), literal(0));
        assert!(prove_goal(&context, &goal, &[], &zero).is_none());
        assert!(check_certificate(&context, &goal, &[], &zero, &proof).is_err());
        // A bound the minuend does not reach stays unproved: the same route
        // cannot pretend `remaining < 2` when `count` may be 11.
        let tight = Proposition::LessThan(value(4), literal(2));
        assert!(prove_goal(&context, &tight, &[], &axioms).is_none());
    }
}
