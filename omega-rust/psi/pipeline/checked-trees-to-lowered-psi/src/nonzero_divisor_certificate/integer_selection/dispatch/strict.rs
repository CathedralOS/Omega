//! Strict order from exact endpoint equalities and checked discrete bounds.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{IntegerCarrier, IntegerValue, Proposition, ScalarTerm, ScalarType};

use super::super::super::integer_evidence::{closed_integer_relation, projected_facts};
use super::super::exact;

pub(super) fn prove(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessThan(left, right) = goal else {
        return None;
    };
    let ScalarType::Integer(integer_type) = left.scalar_type() else {
        return None;
    };
    if integer_type.is_address() || right.scalar_type() != left.scalar_type() {
        return None;
    }
    if let Some(closed) = closed_integer_relation(goal.clone()) {
        return Some(closed);
    }
    let facts = projected_facts(assumptions, semantic_axioms);
    let mut equalities = Vec::new();
    for fact in facts.iter().rev() {
        match fact.proposition {
            Proposition::LessThan(_, _) => {
                if let Some(proof) = complete(goal, fact.proof(), assumptions, semantic_axioms) {
                    return Some(proof);
                }
            }
            Proposition::LessOrEqual(left, right) => {
                let conclusions = [
                    adjacent(left, false)
                        .map(|previous| Proposition::LessThan(previous, right.clone())),
                    adjacent(right, true).map(|next| Proposition::LessThan(left.clone(), next)),
                ];
                for conclusion in conclusions.into_iter().flatten() {
                    let discrete = ProofNode {
                        conclusion,
                        rule: ProofRule::IntegerOrderDiscreteness {
                            relation: Box::new(fact.proof()),
                        },
                    };
                    if let Some(proof) = complete(goal, discrete, assumptions, semantic_axioms) {
                        return Some(proof);
                    }
                }
            }
            Proposition::Equal(left, right) => equalities.push((left, right)),
            _ => {}
        }
    }

    let literal = |term: &ScalarTerm| {
        if term.integer_value().is_some() {
            return Some(term.clone());
        }
        equalities
            .iter()
            .flat_map(|(left, right)| [*left, *right])
            .find_map(|candidate| {
                (candidate.integer_value().is_some()
                    && candidate.scalar_type() == term.scalar_type())
                .then(|| {
                    exact::prove(
                        &Proposition::Equal(term.clone(), candidate.clone()),
                        assumptions,
                        semantic_axioms,
                    )
                })
                .flatten()
                .map(|_| candidate.clone())
            })
    };
    let closed = closed_integer_relation(Proposition::LessThan(literal(left)?, literal(right)?))?;
    complete(goal, closed, assumptions, semantic_axioms)
}

fn adjacent(literal: &ScalarTerm, increasing: bool) -> Option<ScalarTerm> {
    let (integer_type, value) = literal.integer_value()?;
    if integer_type.carrier() != IntegerCarrier::Fixed || integer_type.is_address() {
        return None;
    }
    let adjacent = match (value, increasing) {
        (IntegerValue::Signed(value), true) => IntegerValue::Signed(value.checked_add(1)?),
        (IntegerValue::Signed(value), false) => IntegerValue::Signed(value.checked_sub(1)?),
        (IntegerValue::Unsigned(value), true) => IntegerValue::Unsigned(value.checked_add(1)?),
        (IntegerValue::Unsigned(value), false) => IntegerValue::Unsigned(value.checked_sub(1)?),
    };
    ScalarTerm::integer(integer_type, adjacent).ok()
}

fn complete(
    goal: &Proposition,
    mut proof: ProofNode,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessThan(goal_left, goal_right) = goal else {
        return None;
    };
    for (endpoint, target) in [goal_left, goal_right].into_iter().enumerate() {
        let Proposition::LessThan(left, right) = &proof.conclusion else {
            return None;
        };
        let old = if endpoint == 0 { left } else { right };
        if old == target {
            continue;
        }
        let equality = exact::prove(
            &Proposition::Equal(old.clone(), target.clone()),
            assumptions,
            semantic_axioms,
        )?;
        let conclusion = if endpoint == 0 {
            Proposition::LessThan(target.clone(), right.clone())
        } else {
            Proposition::LessThan(left.clone(), target.clone())
        };
        proof = ProofNode {
            conclusion,
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(proof),
                equality: Box::new(equality),
                endpoint,
            },
        };
    }
    (proof.conclusion == *goal).then_some(proof)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proof_admission::check_certificate;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, PropositionContext, ValueId,
    };

    #[test]
    fn discrete_bound_transports_the_index_literal_without_inventing_a_strict_fact() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let scalar_type = ScalarType::Integer(integer_type);
        let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
        let literal =
            |number| ScalarTerm::integer(integer_type, IntegerValue::Unsigned(number)).unwrap();
        let context = PropositionContext::from_value_types(
            (1..=2).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
        )
        .unwrap();
        let axioms = [
            Proposition::Equal(value(1), literal(0)),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(literal(1), value(2)),
                Proposition::LessOrEqual(value(2), literal(255)),
            ]),
        ];
        let goal = Proposition::LessThan(value(1), value(2));
        let proof = prove(&goal, &[], &axioms).expect("discrete bound plus index equality");
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        assert!(prove(&goal, &[], &axioms[..1]).is_none());
        assert!(check_certificate(&context, &goal, &[], &axioms[..1], &proof).is_err());
        assert!(
            prove(
                &goal,
                &[],
                &[
                    axioms[0].clone(),
                    Proposition::LessOrEqual(literal(0), value(2))
                ]
            )
            .is_none()
        );
    }

    #[test]
    fn strict_literal_transport_replays_nested_and_reversed_equalities() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
        let scalar_type = ScalarType::Integer(integer_type);
        let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
        let literal =
            |integer| ScalarTerm::integer(integer_type, IntegerValue::Unsigned(integer)).unwrap();
        let context = PropositionContext::from_value_types(
            (1..=3).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
        )
        .unwrap();
        let axioms = [
            Proposition::Equal(literal(1), value(3)),
            Proposition::Conjunction(vec![
                Proposition::Equal(value(3), value(1)),
                Proposition::Equal(value(2), literal(2)),
            ]),
        ];
        let goal = Proposition::LessThan(value(1), value(2));
        let proof = prove(&goal, &[], &axioms).expect("strict order uses exact literal equalities");
        assert!(matches!(
            proof.rule,
            ProofRule::IntegerOrderSubstitution { endpoint: 1, .. }
        ));
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        assert!(check_certificate(&context, &goal, &[], &axioms[..1], &proof).is_err());
        assert!(prove(&goal, &[], &axioms[..1]).is_none());
        assert!(prove(&Proposition::LessThan(value(2), value(1)), &[], &axioms).is_none());
        assert!(prove(&goal, &[Proposition::LessOrEqual(value(1), value(2))], &[]).is_none());
    }
}
