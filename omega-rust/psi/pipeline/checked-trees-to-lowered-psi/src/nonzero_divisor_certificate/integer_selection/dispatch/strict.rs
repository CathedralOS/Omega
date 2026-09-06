//! Strict order transport through explicitly proved endpoint equalities.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType};

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
