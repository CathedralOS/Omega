//! Strict order from exact endpoint equalities and checked discrete bounds.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{
    IntegerCarrier, IntegerValue, Proposition, PropositionContext, ScalarTerm, ScalarType,
};

use super::super::super::affine_custody::DefinitionIndex;
use super::super::super::integer_evidence::{closed_integer_relation, projected_facts};
use super::super::{bound, exact, wrapping};

mod subtract;
mod transitive;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    prove_without_subtract(goal, assumptions, semantic_axioms)
        .or_else(|| subtract::prove(goal, assumptions, semantic_axioms))
        // Strict endpoints traverse checked wrapping-update chains that the
        // non-strict affine selection cannot cite.
        .or_else(|| wrapping::prove(context, goal, assumptions, semantic_axioms, definitions))
        .or_else(|| {
            prove_discrete_endpoint(context, goal, assumptions, semantic_axioms, definitions)
        })
        .or_else(|| transitive::prove(context, goal, assumptions, semantic_axioms, definitions))
}

/// A literal endpoint, including a separately observed equal value, turns a
/// strict goal into an adjacent non-strict bound. Let ordinary bound selection
/// retain the operand's cast/definition evidence, then replay discreteness and
/// the exact endpoint equality. No source-specific index facts are introduced.
fn prove_discrete_endpoint(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let Proposition::LessThan(left, right) = goal else {
        return None;
    };
    let facts = projected_facts(assumptions, semantic_axioms);
    let mut literals = Vec::new();
    for candidate in [left, right].into_iter().chain(
        facts
            .iter()
            .filter_map(|fact| match fact.proposition {
                Proposition::Equal(left, right) => Some([left, right]),
                _ => None,
            })
            .flatten(),
    ) {
        if candidate.integer_value().is_some() && !literals.contains(candidate) {
            literals.push(candidate.clone());
        }
    }
    for (endpoint, target) in [left, right].into_iter().enumerate() {
        for literal in &literals {
            if literal.scalar_type() != target.scalar_type()
                || exact::prove(
                    &Proposition::Equal(target.clone(), literal.clone()),
                    assumptions,
                    semantic_axioms,
                )
                .is_none()
            {
                continue;
            }
            let Some(adjacent) = adjacent(literal, endpoint == 0) else {
                continue;
            };
            let (nonstrict, strict) = if endpoint == 0 {
                (
                    Proposition::LessOrEqual(adjacent, right.clone()),
                    Proposition::LessThan(literal.clone(), right.clone()),
                )
            } else {
                (
                    Proposition::LessOrEqual(left.clone(), adjacent),
                    Proposition::LessThan(left.clone(), literal.clone()),
                )
            };
            // Call the non-strict selector directly: the proposition dispatcher
            // itself falls back to strict order and would recurse here.
            let Some(relation) = bound::prove(
                context,
                &nonstrict,
                assumptions,
                semantic_axioms,
                definitions,
            ) else {
                continue;
            };
            let discrete = ProofNode {
                conclusion: strict,
                rule: ProofRule::IntegerOrderDiscreteness {
                    relation: Box::new(relation),
                },
            };
            if let Some(proof) = complete(goal, discrete, assumptions, semantic_axioms) {
                return Some(proof);
            }
        }
    }
    None
}

fn prove_without_subtract(
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
    use super::{
        DefinitionIndex, ProofNode, ProofRule, Proposition, ScalarTerm, ScalarType, prove,
    };
    use proof_admission::check_certificate;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, PropositionContext, ValueId,
    };

    fn prove_with_definitions(
        context: &PropositionContext,
        goal: &Proposition,
        assumptions: &[Proposition],
        semantic_axioms: &[Proposition],
    ) -> Option<ProofNode> {
        prove(
            context,
            goal,
            assumptions,
            semantic_axioms,
            &mut DefinitionIndex::new(semantic_axioms),
        )
    }

    #[test]
    fn strict_endpoint_composes_integer_conversion_and_live_equality() {
        let target_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let target_scalar = ScalarType::Integer(target_type);
        let target = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), target_scalar);
        let literal =
            |number| ScalarTerm::integer(target_type, IntegerValue::Unsigned(number)).unwrap();
        for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            let source_type = IntegerType::new(sign, 32).unwrap();
            let source_scalar = ScalarType::Integer(source_type);
            let source = ScalarTerm::value(ValueId::new(1).unwrap(), source_scalar);
            let source_literal = |number| {
                ScalarTerm::integer(
                    source_type,
                    match sign {
                        IntegerSign::Signed => IntegerValue::Signed(number as i128),
                        IntegerSign::Unsigned => IntegerValue::Unsigned(number),
                    },
                )
                .unwrap()
            };
            let context = PropositionContext::from_value_types([
                (ValueId::new(1).unwrap(), source_scalar),
                (ValueId::new(2).unwrap(), target_scalar),
                (ValueId::new(3).unwrap(), target_scalar),
                (ValueId::new(4).unwrap(), target_scalar),
            ])
            .unwrap();
            let converted = if sign == IntegerSign::Signed {
                ScalarTerm::integer_exact_cast(source_type, target_type, source.clone()).unwrap()
            } else {
                ScalarTerm::integer_widen(source_type, target_type, source.clone()).unwrap()
            };
            let assumptions = [Proposition::Conjunction(vec![
                Proposition::LessOrEqual(source_literal(0), source.clone()),
                Proposition::LessOrEqual(source.clone(), source_literal(2)),
            ])];
            let axioms = [
                Proposition::Equal(target(2), converted),
                Proposition::Equal(target(3), target(4)),
                Proposition::Equal(target(4), literal(3)),
            ];
            let goal = Proposition::LessThan(target(2), target(3));
            let proof = prove_with_definitions(&context, &goal, &assumptions, &axioms)
                .expect("converted upper bound and current endpoint equality");
            check_certificate(&context, &goal, &assumptions, &axioms, &proof).unwrap();
            for missing in 0..axioms.len() {
                let mut incomplete = axioms.to_vec();
                incomplete.remove(missing);
                assert!(
                    prove_with_definitions(&context, &goal, &assumptions, &incomplete).is_none()
                );
                assert!(
                    check_certificate(&context, &goal, &assumptions, &incomplete, &proof).is_err()
                );
            }
            let past_end = [Proposition::Conjunction(vec![
                Proposition::LessOrEqual(source_literal(0), source.clone()),
                Proposition::LessOrEqual(source.clone(), source_literal(3)),
            ])];
            assert!(prove_with_definitions(&context, &goal, &past_end, &axioms).is_none());
            assert!(check_certificate(&context, &goal, &past_end, &axioms, &proof).is_err());

            // A live observation need not equal a constant: its guard supplies
            // the strict leg, while the index bound still crosses conversion.
            for guard in [
                Proposition::LessThan(literal(2), target(4)),
                Proposition::LessOrEqual(literal(3), target(4)),
            ] {
                let guarded = [axioms[0].clone(), axioms[1].clone(), guard];
                let proof = prove_with_definitions(&context, &goal, &assumptions, &guarded)
                    .expect("converted bound joins the live strict guard");
                check_certificate(&context, &goal, &assumptions, &guarded, &proof).unwrap();
                for missing in 0..guarded.len() {
                    let mut incomplete = guarded.to_vec();
                    incomplete.remove(missing);
                    assert!(
                        prove_with_definitions(&context, &goal, &assumptions, &incomplete)
                            .is_none()
                    );
                    assert!(
                        check_certificate(&context, &goal, &assumptions, &incomplete, &proof)
                            .is_err()
                    );
                }
                let weak = [
                    axioms[0].clone(),
                    axioms[1].clone(),
                    Proposition::LessOrEqual(literal(2), target(4)),
                ];
                assert!(prove_with_definitions(&context, &goal, &assumptions, &weak).is_none());
                assert!(check_certificate(&context, &goal, &assumptions, &weak, &proof).is_err());
            }
        }
    }

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
        let proof = prove_with_definitions(&context, &goal, &[], &axioms)
            .expect("discrete bound plus index equality");
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        assert!(prove_with_definitions(&context, &goal, &[], &axioms[..1]).is_none());
        assert!(check_certificate(&context, &goal, &[], &axioms[..1], &proof).is_err());
        assert!(
            prove_with_definitions(
                &context,
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
        let proof = prove_with_definitions(&context, &goal, &[], &axioms)
            .expect("strict order uses exact literal equalities");
        assert!(matches!(
            proof.rule,
            ProofRule::IntegerOrderSubstitution { endpoint: 1, .. }
        ));
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        assert!(check_certificate(&context, &goal, &[], &axioms[..1], &proof).is_err());
        assert!(prove_with_definitions(&context, &goal, &[], &axioms[..1]).is_none());
        assert!(
            prove_with_definitions(
                &context,
                &Proposition::LessThan(value(2), value(1)),
                &[],
                &axioms,
            )
            .is_none()
        );
        assert!(
            prove_with_definitions(
                &context,
                &goal,
                &[Proposition::LessOrEqual(value(1), value(2))],
                &[],
            )
            .is_none()
        );
    }

    fn wrapping_add_definition(
        integer_type: IntegerType,
        defined: &ScalarTerm,
        operand: &ScalarTerm,
        addend: u128,
    ) -> Proposition {
        Proposition::Equal(
            defined.clone(),
            ScalarTerm::wrapping_integer_add(
                integer_type,
                operand.clone(),
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(addend)).unwrap(),
            )
            .unwrap(),
        )
    }

    #[test]
    fn wrapping_add_backward_derives_the_strict_guard_with_cited_headroom() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let scalar_type = ScalarType::Integer(integer_type);
        let defined = ScalarTerm::value(ValueId::new(1).unwrap(), scalar_type);
        let operand = ScalarTerm::value(ValueId::new(2).unwrap(), scalar_type);
        let literal =
            |number| ScalarTerm::integer(integer_type, IntegerValue::Unsigned(number)).unwrap();
        let context = PropositionContext::from_value_types(
            (1..=2).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
        )
        .unwrap();
        let axioms = [
            wrapping_add_definition(integer_type, &defined, &operand, 1),
            Proposition::LessThan(defined.clone(), literal(2)),
            Proposition::LessOrEqual(operand.clone(), literal(u64::MAX as u128 - 1)),
        ];
        let goal = Proposition::LessThan(operand.clone(), literal(1));
        let proof = prove_with_definitions(&context, &goal, &[], &axioms)
            .expect("strict backward traversal with cited headroom");
        assert!(matches!(proof.rule, ProofRule::IntegerAffineBound { .. }));
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        // Without the headroom conjunct the step is not admitted and the
        // stale proof no longer replays.
        assert!(prove_with_definitions(&context, &goal, &[], &axioms[..2]).is_none());
        assert!(check_certificate(&context, &goal, &[], &axioms[..2], &proof).is_err());
        // The carrier bound `operand <= MAX` supplies no headroom at all, so
        // `v_old` could wrap and the checked requirement `operand <= MAX - 1`
        // is unprovable.
        let no_headroom = [
            axioms[0].clone(),
            axioms[1].clone(),
            Proposition::LessOrEqual(operand.clone(), literal(u64::MAX as u128)),
        ];
        assert!(prove_with_definitions(&context, &goal, &[], &no_headroom).is_none());
    }

    #[test]
    fn wrapping_add_backward_derives_headroom_from_the_strict_guard() {
        // `v_new = wrapping(v_old + 1)`; the guard `v_old < 3` supplies the
        // `v_old <= MAX - 1` headroom through weakening plus closed
        // transitivity, so `v_new < 2` yields `v_old < 1`.
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let scalar_type = ScalarType::Integer(integer_type);
        let defined = ScalarTerm::value(ValueId::new(1).unwrap(), scalar_type);
        let operand = ScalarTerm::value(ValueId::new(2).unwrap(), scalar_type);
        let literal =
            |number| ScalarTerm::integer(integer_type, IntegerValue::Unsigned(number)).unwrap();
        let context = PropositionContext::from_value_types(
            (1..=2).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
        )
        .unwrap();
        let axioms = [
            wrapping_add_definition(integer_type, &defined, &operand, 1),
            Proposition::LessThan(defined.clone(), literal(2)),
            Proposition::LessThan(operand.clone(), literal(3)),
        ];
        let goal = Proposition::LessThan(operand.clone(), literal(1));
        let proof = prove_with_definitions(&context, &goal, &[], &axioms)
            .expect("strict backward traversal with derived headroom");
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        assert!(prove_with_definitions(&context, &goal, &[], &axioms[..2]).is_none());
        // The same equation must not over-derive: `operand < 2` does not
        // follow, since a wrapped sum can satisfy `v_new < 2` at any operand.
        assert!(
            prove_with_definitions(
                &context,
                &Proposition::LessThan(operand.clone(), literal(2)),
                &[],
                &axioms,
            )
            .is_none()
        );
    }

    #[test]
    fn wrapping_steps_drive_nonstrict_bounds_through_full_selection() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let scalar_type = ScalarType::Integer(integer_type);
        let defined = ScalarTerm::value(ValueId::new(1).unwrap(), scalar_type);
        let operand = ScalarTerm::value(ValueId::new(2).unwrap(), scalar_type);
        let dividend = ScalarTerm::value(ValueId::new(3).unwrap(), scalar_type);
        let quotient = ScalarTerm::value(ValueId::new(4).unwrap(), scalar_type);
        let literal =
            |number| ScalarTerm::integer(integer_type, IntegerValue::Unsigned(number)).unwrap();
        let context = PropositionContext::from_value_types(
            (1..=4).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
        )
        .unwrap();
        let axioms = [
            wrapping_add_definition(integer_type, &defined, &operand, 1),
            Proposition::Equal(
                quotient.clone(),
                ScalarTerm::wrapping_integer_divide(integer_type, dividend.clone(), literal(10))
                    .unwrap(),
            ),
            Proposition::LessOrEqual(defined.clone(), literal(2)),
            Proposition::LessThan(operand.clone(), literal(3)),
            Proposition::LessOrEqual(literal(100), dividend.clone()),
        ];
        // Backward through the wrapping add with derived headroom:
        // `defined <= 2` yields `operand <= 1`.
        let goal = Proposition::LessOrEqual(operand.clone(), literal(1));
        let proof = super::super::super::build(&context, &goal, &[], &axioms)
            .expect("non-strict backward traversal with derived headroom");
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
        // Forward through the wrapping divide needs no evidence:
        // `dividend >= 100` yields `quotient >= 10`.
        let goal = Proposition::LessOrEqual(literal(10), quotient.clone());
        let proof = super::super::super::build(&context, &goal, &[], &axioms)
            .expect("forward wrapping-divide bound");
        check_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
    }
}
