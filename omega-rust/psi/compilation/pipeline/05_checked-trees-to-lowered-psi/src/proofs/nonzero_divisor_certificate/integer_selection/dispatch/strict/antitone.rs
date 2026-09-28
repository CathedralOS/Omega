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
        value == target
            || session
                .prove(&Proposition::Equal(value.clone(), target.clone()))
                .is_some()
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
            value == target
                || session
                    .prove(&Proposition::Equal(value.clone(), target.clone()))
                    .is_some()
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

#[cfg(test)]
mod tests {
    use super::super::super::super::super::affine_custody::DefinitionIndex;
    use super::super::prove;
    use proof_admission::check_certificate;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
        ScalarType, ValueId,
    };

    #[test]
    fn ceiling_distance_edge_proves_through_its_advance_and_every_citation() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let scalar = ScalarType::Integer(integer);
        let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar);
        let literal =
            |number| ScalarTerm::integer(integer, IntegerValue::Unsigned(number)).unwrap();
        let context = PropositionContext::from_value_types(
            (1..=7).map(|identity| (ValueId::new(identity).unwrap(), scalar)),
        )
        .unwrap();
        // ceiling = MAX, before = ceiling - index, one = 1,
        // advanced = index + one, after = ceiling - advanced.
        let (ceiling, index, one, advanced, before, after) =
            (value(1), value(2), value(3), value(4), value(5), value(6));
        let axioms = [
            Proposition::Equal(ceiling.clone(), literal(u128::from(u64::MAX))),
            Proposition::Equal(
                before.clone(),
                ScalarTerm::exact_integer_subtract(integer, ceiling.clone(), index.clone())
                    .unwrap(),
            ),
            Proposition::Equal(one.clone(), literal(1)),
            Proposition::Equal(
                advanced.clone(),
                ScalarTerm::exact_integer_add(integer, index.clone(), one.clone()).unwrap(),
            ),
            Proposition::Equal(
                after.clone(),
                ScalarTerm::exact_integer_subtract(integer, ceiling.clone(), advanced.clone())
                    .unwrap(),
            ),
        ];
        let prove_goal = |goal: &Proposition, axioms: &[Proposition]| {
            prove(
                &context,
                goal,
                &[],
                axioms,
                &mut DefinitionIndex::new(&context, &[], axioms),
            )
        };
        let goal = Proposition::LessThan(after.clone(), before.clone());
        let proof = prove_goal(&goal, &axioms).expect("the advanced subtrahend lowers the rank");
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        // The ceiling's own value is not needed: one shared minuend is.
        for missing in 1..axioms.len() {
            let mut incomplete = axioms.to_vec();
            incomplete.remove(missing);
            assert!(
                prove_goal(&goal, &incomplete).is_none(),
                "missing citation {missing}"
            );
        }
        assert!(prove_goal(&Proposition::LessThan(before.clone(), after), &axioms).is_none());
        // Another minuend for the arrival is not the same rank.
        let mut rerooted = axioms.to_vec();
        rerooted[4] = Proposition::Equal(
            value(6),
            ScalarTerm::exact_integer_subtract(integer, value(7), advanced).unwrap(),
        );
        assert!(prove_goal(&goal, &rerooted).is_none());
    }
}
