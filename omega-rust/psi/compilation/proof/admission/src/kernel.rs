/*
    Closed primitive judgment decider and fixed-carrier integer bound checks.

    We re-decide closed reflexivity, constant comparisons, carrier minimum and
    maximum bounds, and closed mathematical integer relations without proof
    search.
*/

use crate::integer_rules::closed_integer::{
    ClosedIntegerEvaluationError, check_integer_math_term_size, compare_integer_math_terms,
};
use crate::proof::integer_math_normalization::lift_exact_integer_relation;
use semantic_vocabulary::IntegerMathLiteral;
use semantic_vocabulary::{IntegerMathTerm, Proposition, PropositionContext, ScalarTerm};
use std::cmp::Ordering;
pub use terminal_psi::PrimitiveJudgment;

pub fn decide_primitive(
    context: &PropositionContext,
    proposition: &Proposition,
    judgment: PrimitiveJudgment,
) -> Result<(), KernelError> {
    let relation = match proposition {
        Proposition::Implication {
            premise,
            conclusion,
        } if **conclusion == Proposition::Falsehood => premise.as_ref(),
        _ => proposition,
    };
    if let semantic_vocabulary::integer_math_comparison_operands!(left, right) = relation {
        check_integer_math_term_size(left, right).map_err(KernelError::ClosedIntegerEvaluation)?;
    }
    context
        .validate(proposition)
        .map_err(KernelError::MalformedProposition)?;
    let accepted = match (judgment, proposition) {
        (PrimitiveJudgment::Truth, Proposition::Truth) => true,
        (PrimitiveJudgment::IntegerCarrierBound, Proposition::LessOrEqual(left, right)) => {
            integer_carrier_bound(left, right)
        }
        (
            PrimitiveJudgment::IntegerCarrierBound,
            Proposition::IntegerMathLessOrEqual(left, right),
        ) => mathematical_integer_carrier_bound(left, right),
        (PrimitiveJudgment::IntegerCarrierBound, Proposition::IntegerMathLessThan(left, right)) => {
            mathematical_integer_carrier_bound_exclusive(left, right)?
        }
        (PrimitiveJudgment::ReflexiveEquality, Proposition::Equal(left, right)) => left == right,
        (PrimitiveJudgment::ReflexiveEquality, Proposition::IntegerMathEqual(left, right)) => {
            left == right
        }
        (PrimitiveJudgment::ReflexiveEquality, Proposition::ContentConservation(conservation)) => {
            conservation.left() == conservation.right()
        }
        (PrimitiveJudgment::ClosedIntegerRelation, Proposition::Equal(..))
        | (PrimitiveJudgment::ClosedIntegerRelation, Proposition::LessThan(..))
        | (PrimitiveJudgment::ClosedIntegerRelation, Proposition::LessOrEqual(..)) => {
            let exact = compare_fixed_integer_relation(proposition)?.is_some_and(|ordering| {
                match proposition {
                    Proposition::Equal(..) => ordering.is_eq(),
                    Proposition::LessThan(..) => ordering.is_lt(),
                    _ => !ordering.is_gt(),
                }
            });
            if exact {
                true
            } else if let Some((left, right)) = proposition.as_equal() {
                crate::integer_rules::wrapping_equality::equal(left, right)
                    .map_err(KernelError::ClosedIntegerEvaluation)?
            } else {
                false
            }
        }
        (
            PrimitiveJudgment::ClosedIntegerRelation,
            Proposition::Implication {
                premise,
                conclusion,
            },
        ) if **conclusion == Proposition::Falsehood => {
            // An unavailable evaluation is not evidence of falsity. Only a
            // closed comparison decided false proves its logical negation.
            closed_mathematical_relation(premise)? == Some(false)
        }
        (PrimitiveJudgment::ClosedIntegerRelation, relation) => {
            closed_mathematical_relation(relation)? == Some(true)
        }
        _ => false,
    };
    accepted
        .then_some(())
        .ok_or(KernelError::JudgmentDoesNotEstablishGoal { judgment })
}

fn closed_mathematical_relation(proposition: &Proposition) -> Result<Option<bool>, KernelError> {
    let semantic_vocabulary::integer_math_comparison_operands!(left, right) = proposition else {
        return Ok(None);
    };
    Ok(compare_integer_math_terms(left, right)
        .map_err(KernelError::ClosedIntegerEvaluation)?
        .map(|ordering| match proposition {
            Proposition::IntegerMathEqual(..) => ordering.is_eq(),
            Proposition::IntegerMathLessThan(..) => ordering.is_lt(),
            _ => !ordering.is_gt(),
        }))
}

fn integer_carrier_bound(left: &ScalarTerm, right: &ScalarTerm) -> bool {
    use semantic_vocabulary::{IntegerCarrier, ScalarType};
    let (literal, value, lower) = match (left, right) {
        (literal @ ScalarTerm::Integer { .. }, value @ ScalarTerm::Value { .. }) => {
            (literal, value, true)
        }
        (value @ ScalarTerm::Value { .. }, literal @ ScalarTerm::Integer { .. }) => {
            (literal, value, false)
        }
        _ => return false,
    };
    let ScalarTerm::Value {
        scalar_type: ScalarType::Integer(integer_type),
        ..
    } = value
    else {
        return false;
    };
    let Some((literal_type, literal_value)) = literal.integer_value() else {
        return false;
    };
    // A parameter width has no literal endpoint, so no literal is one.
    integer_type.carrier() == IntegerCarrier::Fixed
        && literal_type == *integer_type
        && Some(literal_value)
            == if lower {
                integer_type.minimum_value()
            } else {
                integer_type.maximum_value()
            }
}

fn mathematical_integer_carrier_bound(left: &IntegerMathTerm, right: &IntegerMathTerm) -> bool {
    let (literal, source_type, lower) = match (left, right) {
        (
            IntegerMathTerm::IntegerLiteral(literal),
            IntegerMathTerm::MathValue { source_type, .. },
        ) => (literal, source_type, true),
        (
            IntegerMathTerm::MathValue { source_type, .. },
            IntegerMathTerm::IntegerLiteral(literal),
        ) => (literal, source_type, false),
        _ => return false,
    };
    // Both sides are `None` at a parameter width; that is no bound.
    let bound = if lower {
        source_type.minimum_value()
    } else {
        source_type.maximum_value()
    };
    bound.is_some() && literal.as_integer_value(*source_type) == bound
}

fn mathematical_integer_carrier_bound_exclusive(
    left: &IntegerMathTerm,
    right: &IntegerMathTerm,
) -> Result<bool, KernelError> {
    let IntegerMathTerm::MathValue { source_type, .. } = left else {
        return Ok(false);
    };
    if !source_type.is_address() {
        return Ok(false);
    }
    let Some(maximum) = source_type.maximum_value() else {
        return Ok(false);
    };
    let bound = IntegerMathTerm::Add(
        Box::new(IntegerMathTerm::IntegerLiteral(
            IntegerMathLiteral::from_integer_value(maximum),
        )),
        Box::new(IntegerMathTerm::IntegerLiteral(
            IntegerMathLiteral::from_integer_value(semantic_vocabulary::IntegerValue::Unsigned(1)),
        )),
    );
    compare_integer_math_terms(right, &bound)
        .map(|comparison| comparison.is_some_and(|ordering| ordering.is_eq()))
        .map_err(KernelError::ClosedIntegerEvaluation)
}

pub(crate) fn compare_integer_literals(left: &ScalarTerm, right: &ScalarTerm) -> Option<Ordering> {
    let (left_type, left) = left.integer_value()?;
    let (right_type, right) = right.integer_value()?;
    if left_type != right_type {
        return None;
    }
    left_type.compare(left, right)
}

fn compare_fixed_integer_relation(
    proposition: &Proposition,
) -> Result<Option<Ordering>, KernelError> {
    let (left, right) = match proposition {
        semantic_vocabulary::ordered_comparison_operands!(left, right) => (left, right),
        _ => return Ok(None),
    };
    if let Some(ordering) = compare_integer_literals(left, right) {
        return Ok(Some(ordering));
    }
    let Some((left, right)) = lift_exact_integer_relation(proposition) else {
        return Ok(None);
    };
    compare_integer_math_terms(&left, &right).map_err(KernelError::ClosedIntegerEvaluation)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelError {
    MalformedProposition(semantic_vocabulary::PropositionError),
    ClosedIntegerEvaluation(ClosedIntegerEvaluationError),
    JudgmentDoesNotEstablishGoal { judgment: PrimitiveJudgment },
}

crate::debug_display!(KernelError);

impl std::error::Error for KernelError {}

#[cfg(test)]
mod tests {
    use super::{
        ClosedIntegerEvaluationError, KernelError, PrimitiveJudgment, Proposition,
        PropositionContext, ScalarTerm, decide_primitive,
    };
    use semantic_vocabulary::{IntegerMathTerm, IntegerSign, IntegerType, IntegerValue};

    #[test]
    fn closed_integer_judgment_is_total_and_refuses_false_relations() {
        let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 type");
        let three = ScalarTerm::integer(i32_type, IntegerValue::Signed(3)).expect("3:i32");
        let four = ScalarTerm::integer(i32_type, IntegerValue::Signed(4)).expect("4:i32");
        assert!(
            decide_primitive(
                &PropositionContext::default(),
                &Proposition::LessThan(three.clone(), four.clone()),
                PrimitiveJudgment::ClosedIntegerRelation,
            )
            .is_ok()
        );
        assert!(
            decide_primitive(
                &PropositionContext::default(),
                &Proposition::LessThan(four, three),
                PrimitiveJudgment::ClosedIntegerRelation,
            )
            .is_err()
        );
    }

    #[test]
    fn closed_mathematical_integer_relations_use_unbounded_evaluation() {
        let maximum = IntegerMathTerm::literal(IntegerValue::Unsigned(u128::MAX));
        let one = IntegerMathTerm::literal(IntegerValue::Unsigned(1));
        let sum = IntegerMathTerm::Add(Box::new(maximum.clone()), Box::new(one.clone()));
        let product = IntegerMathTerm::Multiply(Box::new(maximum), Box::new(one));
        assert!(
            decide_primitive(
                &PropositionContext::default(),
                &Proposition::IntegerMathLessThan(product, sum),
                PrimitiveJudgment::ClosedIntegerRelation,
            )
            .is_ok()
        );
    }

    #[test]
    fn closed_mathematical_negation_requires_a_false_relation() {
        use semantic_vocabulary::{ScalarType, ValueId};
        let literal = |value| IntegerMathTerm::literal(IntegerValue::Unsigned(value));
        let negate = |premise| Proposition::Implication {
            premise: Box::new(premise),
            conclusion: Box::new(Proposition::Falsehood),
        };
        for (relation, accepted) in [
            (Proposition::IntegerMathEqual(literal(2), literal(3)), true),
            (Proposition::IntegerMathEqual(literal(2), literal(2)), false),
            (
                Proposition::IntegerMathLessThan(literal(3), literal(2)),
                true,
            ),
            (
                Proposition::IntegerMathLessThan(literal(2), literal(3)),
                false,
            ),
            (
                Proposition::IntegerMathLessOrEqual(literal(3), literal(2)),
                true,
            ),
            (
                Proposition::IntegerMathLessOrEqual(literal(2), literal(2)),
                false,
            ),
        ] {
            let goal = negate(relation);
            assert_eq!(
                decide_primitive(
                    &PropositionContext::default(),
                    &goal,
                    PrimitiveJudgment::ClosedIntegerRelation
                )
                .is_ok(),
                accepted
            );
            let proof = crate::ProofNode {
                conclusion: goal.clone(),
                rule: crate::ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            };
            assert_eq!(
                crate::accept_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
                    .is_ok(),
                accepted
            );
        }
        let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
        let value = ValueId::new(1).unwrap();
        let context =
            PropositionContext::from_value_types([(value, ScalarType::Integer(integer))]).unwrap();
        let open = Proposition::IntegerMathEqual(
            IntegerMathTerm::MathValue {
                source_type: integer,
                value,
            },
            literal(2),
        );
        assert!(
            decide_primitive(
                &context,
                &negate(open),
                PrimitiveJudgment::ClosedIntegerRelation
            )
            .is_err()
        );
        let over_budget = IntegerMathTerm::ShiftLeft {
            value: Box::new(literal(1)),
            count: Box::new(literal(u128::MAX)),
        };
        assert_eq!(
            decide_primitive(
                &context,
                &negate(Proposition::IntegerMathLessThan(over_budget, literal(0))),
                PrimitiveJudgment::ClosedIntegerRelation
            ),
            Err(KernelError::ClosedIntegerEvaluation(
                ClosedIntegerEvaluationError::ResourceLimitExceeded
            ))
        );
        let forged = Proposition::Implication {
            premise: Box::new(Proposition::IntegerMathEqual(literal(2), literal(3))),
            conclusion: Box::new(Proposition::IntegerMathEqual(literal(2), literal(3))),
        };
        assert!(
            decide_primitive(&context, &forged, PrimitiveJudgment::ClosedIntegerRelation).is_err()
        );
    }

    #[test]
    fn resource_refusal_is_not_a_false_primitive_judgment() {
        let term = IntegerMathTerm::ShiftLeft {
            value: Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(1))),
            count: Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(u128::MAX))),
        };
        assert_eq!(
            decide_primitive(
                &PropositionContext::default(),
                &Proposition::IntegerMathLessThan(
                    term,
                    IntegerMathTerm::literal(IntegerValue::Unsigned(0))
                ),
                PrimitiveJudgment::ClosedIntegerRelation,
            ),
            Err(KernelError::ClosedIntegerEvaluation(
                ClosedIntegerEvaluationError::ResourceLimitExceeded
            )),
        );
    }

    #[test]
    fn mathematical_term_depth_is_refused_before_recursive_formation() {
        let mut term = IntegerMathTerm::literal(IntegerValue::Unsigned(1));
        for _ in 0..1_024 {
            term = IntegerMathTerm::Add(
                Box::new(term),
                Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(0))),
            );
        }
        let proposition = Proposition::IntegerMathLessThan(
            term,
            IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
        );
        assert_eq!(
            decide_primitive(
                &PropositionContext::default(),
                &proposition,
                PrimitiveJudgment::ClosedIntegerRelation
            ),
            Err(KernelError::ClosedIntegerEvaluation(
                ClosedIntegerEvaluationError::ResourceLimitExceeded
            )),
        );
    }
}

#[cfg(test)]
mod carrier_bounds {
    use super::{PrimitiveJudgment, Proposition, PropositionContext, ScalarTerm, decide_primitive};
    use semantic_vocabulary::{
        IntegerMathLiteral, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, ScalarType,
        ValueId,
    };

    #[test]
    fn fixed_integer_carrier_bounds_use_exact_declared_sign_width_and_context() {
        for bits in 1..=128 {
            for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
                let integer = IntegerType::new(sign, bits).unwrap();
                let scalar = ScalarType::Integer(integer);
                let identity = ValueId::new(1).unwrap();
                let value = ScalarTerm::value(identity, scalar);
                let context = PropositionContext::from_value_types([(identity, scalar)]).unwrap();
                let minimum = ScalarTerm::integer(integer, integer.minimum_value().unwrap()).unwrap();
                let maximum = ScalarTerm::integer(integer, integer.maximum_value().unwrap()).unwrap();
                for goal in [
                    Proposition::LessOrEqual(minimum.clone(), value.clone()),
                    Proposition::LessOrEqual(value.clone(), maximum.clone()),
                ] {
                    decide_primitive(&context, &goal, PrimitiveJudgment::IntegerCarrierBound)
                        .unwrap();
                    assert!(
                        decide_primitive(
                            &PropositionContext::default(),
                            &goal,
                            PrimitiveJudgment::IntegerCarrierBound
                        )
                        .is_err()
                    );
                }
                for goal in [
                    Proposition::LessThan(minimum.clone(), value.clone()),
                    Proposition::LessThan(value.clone(), maximum.clone()),
                    Proposition::LessOrEqual(maximum.clone(), value.clone()),
                    Proposition::LessOrEqual(value.clone(), minimum.clone()),
                    Proposition::Equal(minimum, value.clone()),
                    Proposition::LessOrEqual(value.clone(), value),
                ] {
                    assert!(
                        decide_primitive(&context, &goal, PrimitiveJudgment::IntegerCarrierBound)
                            .is_err()
                    );
                }
            }
        }
    }

    #[test]
    fn carrier_bounds_do_not_bless_wrong_types_addresses_or_unevaluated_arithmetic() {
        let identity = ValueId::new(1).unwrap();
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let scalar = ScalarType::Integer(integer);
        let value = ScalarTerm::value(identity, scalar);
        let zero = ScalarTerm::integer(integer, IntegerValue::Unsigned(0)).unwrap();
        let goal = Proposition::LessOrEqual(zero.clone(), value.clone());
        for wrong in [
            ScalarType::Boolean,
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap()),
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap()),
        ] {
            let context = PropositionContext::from_value_types([(identity, wrong)]).unwrap();
            assert!(
                decide_primitive(&context, &goal, PrimitiveJudgment::IntegerCarrierBound).is_err()
            );
        }
        let address = IntegerType::address(64).unwrap();
        let context =
            PropositionContext::from_value_types([(identity, ScalarType::Integer(address))])
                .unwrap();
        let address_goal = Proposition::LessOrEqual(
            ScalarTerm::integer(address, IntegerValue::Unsigned(0)).unwrap(),
            ScalarTerm::value(identity, ScalarType::Integer(address)),
        );
        assert!(
            decide_primitive(
                &context,
                &address_goal,
                PrimitiveJudgment::IntegerCarrierBound
            )
            .is_err()
        );
        let context = PropositionContext::from_value_types([(identity, scalar)]).unwrap();
        let arithmetic = ScalarTerm::ExactIntegerSubtract {
            scalar_type: integer,
            left: Box::new(zero.clone()),
            right: Box::new(value),
        };
        assert!(
            decide_primitive(
                &context,
                &Proposition::LessOrEqual(zero, arithmetic),
                PrimitiveJudgment::IntegerCarrierBound
            )
            .is_err()
        );
    }

    #[test]
    fn mathematical_address_bounds_retain_the_exact_model_and_admit_evidence() {
        for bits in [16, 32, 64, 128] {
            let address = IntegerType::address(bits).unwrap();
            let identity = ValueId::new(1).unwrap();
            let context =
                PropositionContext::from_value_types([(identity, ScalarType::Integer(address))])
                    .unwrap();
            let value = IntegerMathTerm::MathValue {
                source_type: address,
                value: identity,
            };
            let minimum = IntegerMathTerm::literal(address.minimum_value().unwrap());
            let maximum = IntegerMathTerm::literal(address.maximum_value().unwrap());
            for goal in [
                Proposition::IntegerMathLessOrEqual(minimum.clone(), value.clone()),
                Proposition::IntegerMathLessOrEqual(value.clone(), maximum.clone()),
            ] {
                let proof = crate::ProofNode {
                    conclusion: goal.clone(),
                    rule: crate::ProofRule::Primitive(PrimitiveJudgment::IntegerCarrierBound),
                };
                crate::accept_certificate(&context, &goal, &[], &[], &proof).unwrap();
                assert!(
                    decide_primitive(
                        &PropositionContext::default(),
                        &goal,
                        PrimitiveJudgment::IntegerCarrierBound,
                    )
                    .is_err()
                );
            }
            for goal in [
                Proposition::IntegerMathLessOrEqual(maximum.clone(), value.clone()),
                Proposition::IntegerMathLessOrEqual(value.clone(), minimum.clone()),
                Proposition::IntegerMathLessThan(minimum.clone(), value.clone()),
                Proposition::IntegerMathLessThan(value.clone(), maximum),
                Proposition::IntegerMathEqual(minimum.clone(), value.clone()),
                Proposition::IntegerMathLessOrEqual(value.clone(), value.clone()),
                Proposition::IntegerMathLessOrEqual(
                    minimum.clone(),
                    IntegerMathTerm::Subtract(Box::new(value), Box::new(minimum)),
                ),
            ] {
                assert!(
                    decide_primitive(&context, &goal, PrimitiveJudgment::IntegerCarrierBound)
                        .is_err(),
                    "the carrier judgment must not establish {goal:?}",
                );
            }
        }
    }

    #[test]
    fn mathematical_address_bounds_reject_carrier_identity_drift_and_count_narrowing() {
        let address = IntegerType::address(128).unwrap();
        let identity = ValueId::new(1).unwrap();
        let value = IntegerMathTerm::MathValue {
            source_type: address,
            value: identity,
        };
        let goal = Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(address.minimum_value().unwrap()),
            value.clone(),
        );
        for (declared_identity, declared_type) in [
            (
                identity,
                ScalarType::Integer(IntegerType::address(64).unwrap()),
            ),
            (
                identity,
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 128).unwrap()),
            ),
            (identity, ScalarType::Boolean),
            (ValueId::new(2).unwrap(), ScalarType::Integer(address)),
        ] {
            let context =
                PropositionContext::from_value_types([(declared_identity, declared_type)]).unwrap();
            assert!(
                decide_primitive(&context, &goal, PrimitiveJudgment::IntegerCarrierBound).is_err()
            );
        }
        let context =
            PropositionContext::from_value_types([(identity, ScalarType::Integer(address))])
                .unwrap();
        let count_maximum = IntegerMathTerm::literal(IntegerValue::Unsigned(u128::from(u64::MAX)));
        let narrowed = Proposition::IntegerMathLessOrEqual(value, count_maximum);
        assert!(
            decide_primitive(&context, &narrowed, PrimitiveJudgment::IntegerCarrierBound).is_err()
        );
    }

    #[test]
    fn exclusive_address_bounds_use_unbounded_math_and_exact_model_custody() {
        for bits in [16, 32, 64, 128] {
            let address = IntegerType::address(bits).expect("address model");
            let value = ValueId::new(1).expect("value identity");
            let context =
                PropositionContext::from_value_types([(value, ScalarType::Integer(address))])
                    .expect("exact model context");
            let endpoint = IntegerMathTerm::Add(
                Box::new(IntegerMathTerm::IntegerLiteral(
                    IntegerMathLiteral::from_integer_value(address.maximum_value().unwrap()),
                )),
                Box::new(IntegerMathTerm::IntegerLiteral(
                    IntegerMathLiteral::from_integer_value(IntegerValue::Unsigned(1)),
                )),
            );
            let goal = Proposition::IntegerMathLessThan(
                IntegerMathTerm::MathValue {
                    source_type: address,
                    value,
                },
                endpoint,
            );
            assert!(
                decide_primitive(&context, &goal, PrimitiveJudgment::IntegerCarrierBound).is_ok()
            );
            let wrong_model =
                IntegerType::address(if bits == 16 { 32 } else { 16 }).expect("other model");
            let wrong_bound = IntegerMathTerm::Add(
                Box::new(IntegerMathTerm::IntegerLiteral(
                    IntegerMathLiteral::from_integer_value(wrong_model.maximum_value().unwrap()),
                )),
                Box::new(IntegerMathTerm::IntegerLiteral(
                    IntegerMathLiteral::from_integer_value(IntegerValue::Unsigned(1)),
                )),
            );
            assert!(
                decide_primitive(
                    &context,
                    &Proposition::IntegerMathLessThan(
                        IntegerMathTerm::MathValue {
                            source_type: address,
                            value
                        },
                        wrong_bound,
                    ),
                    PrimitiveJudgment::IntegerCarrierBound
                )
                .is_err()
            );
        }
    }
}
