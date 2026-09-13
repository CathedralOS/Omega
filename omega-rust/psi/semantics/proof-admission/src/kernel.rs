use crate::closed_integer::{
    ClosedIntegerEvaluationError, check_integer_math_term_size, compare_integer_math_terms,
};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};
pub use terminal_psi::PrimitiveJudgment;

#[cfg(test)]
mod carrier_bounds;

pub fn decide_primitive(
    context: &PropositionContext,
    proposition: &Proposition,
    judgment: PrimitiveJudgment,
) -> Result<(), KernelError> {
    // Formation and derived equality still recurse through mathematical terms.
    // Bound their input before either traversal, not only before arithmetic.
    if let Proposition::IntegerMathEqual(left, right)
    | Proposition::IntegerMathLessThan(left, right)
    | Proposition::IntegerMathLessOrEqual(left, right) = proposition
    {
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
        (PrimitiveJudgment::ReflexiveEquality, Proposition::Equal(left, right)) => left == right,
        (PrimitiveJudgment::ReflexiveEquality, Proposition::IntegerMathEqual(left, right)) => {
            left == right
        }
        (PrimitiveJudgment::ReflexiveEquality, Proposition::ContentConservation(conservation)) => {
            conservation.left() == conservation.right()
        }
        (PrimitiveJudgment::ClosedIntegerRelation, Proposition::Equal(left, right)) => {
            compare_integer_literals(left, right).is_some_and(|ordering| ordering.is_eq())
        }
        (PrimitiveJudgment::ClosedIntegerRelation, Proposition::LessThan(left, right)) => {
            compare_integer_literals(left, right).is_some_and(|ordering| ordering.is_lt())
        }
        (PrimitiveJudgment::ClosedIntegerRelation, Proposition::LessOrEqual(left, right)) => {
            compare_integer_literals(left, right).is_some_and(|ordering| !ordering.is_gt())
        }
        (PrimitiveJudgment::ClosedIntegerRelation, Proposition::IntegerMathEqual(left, right)) => {
            compare_integer_math_terms(left, right)
                .map_err(KernelError::ClosedIntegerEvaluation)?
                .is_some_and(|ordering| ordering.is_eq())
        }
        (
            PrimitiveJudgment::ClosedIntegerRelation,
            Proposition::IntegerMathLessThan(left, right),
        ) => compare_integer_math_terms(left, right)
            .map_err(KernelError::ClosedIntegerEvaluation)?
            .is_some_and(|ordering| ordering.is_lt()),
        (
            PrimitiveJudgment::ClosedIntegerRelation,
            Proposition::IntegerMathLessOrEqual(left, right),
        ) => compare_integer_math_terms(left, right)
            .map_err(KernelError::ClosedIntegerEvaluation)?
            .is_some_and(|ordering| !ordering.is_gt()),
        _ => false,
    };
    accepted
        .then_some(())
        .ok_or(KernelError::JudgmentDoesNotEstablishGoal { judgment })
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
    integer_type.carrier() == IntegerCarrier::Fixed
        && literal_type == *integer_type
        && literal_value
            == if lower {
                integer_type.minimum_value()
            } else {
                integer_type.maximum_value()
            }
}

fn compare_integer_literals(left: &ScalarTerm, right: &ScalarTerm) -> Option<std::cmp::Ordering> {
    let (left_type, left) = left.integer_value()?;
    let (right_type, right) = right.integer_value()?;
    if left_type != right_type {
        return None;
    }
    left_type.compare(left, right)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelError {
    MalformedProposition(semantic_vocabulary::PropositionError),
    ClosedIntegerEvaluation(ClosedIntegerEvaluationError),
    JudgmentDoesNotEstablishGoal { judgment: PrimitiveJudgment },
}

impl std::fmt::Display for KernelError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for KernelError {}

#[cfg(test)]
mod tests {
    use super::*;
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
