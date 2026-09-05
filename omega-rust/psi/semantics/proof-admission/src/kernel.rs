use numerics::bignum::BigInt;
use semantic_vocabulary::{
    IntegerMathLiteral, IntegerMathTerm, Proposition, PropositionContext, ScalarTerm,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveJudgment {
    Truth,
    ReflexiveEquality,
    ClosedIntegerRelation,
}

pub fn decide_primitive(
    context: &PropositionContext,
    proposition: &Proposition,
    judgment: PrimitiveJudgment,
) -> Result<(), KernelError> {
    context
        .validate(proposition)
        .map_err(KernelError::MalformedProposition)?;
    let accepted = match (judgment, proposition) {
        (PrimitiveJudgment::Truth, Proposition::Truth) => true,
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
            compare_integer_math_terms(left, right).is_some_and(|ordering| ordering.is_eq())
        }
        (
            PrimitiveJudgment::ClosedIntegerRelation,
            Proposition::IntegerMathLessThan(left, right),
        ) => compare_integer_math_terms(left, right).is_some_and(|ordering| ordering.is_lt()),
        (
            PrimitiveJudgment::ClosedIntegerRelation,
            Proposition::IntegerMathLessOrEqual(left, right),
        ) => compare_integer_math_terms(left, right).is_some_and(|ordering| !ordering.is_gt()),
        _ => false,
    };
    accepted
        .then_some(())
        .ok_or(KernelError::JudgmentDoesNotEstablishGoal { judgment })
}

fn compare_integer_math_terms(
    left: &IntegerMathTerm,
    right: &IntegerMathTerm,
) -> Option<std::cmp::Ordering> {
    Some(evaluate_integer_math_term(left)?.cmp(&evaluate_integer_math_term(right)?))
}

fn evaluate_integer_math_term(term: &IntegerMathTerm) -> Option<BigInt> {
    match term {
        IntegerMathTerm::MathValue { .. } => None,
        IntegerMathTerm::IntegerLiteral(literal) => Some(big_integer_literal(*literal)),
        IntegerMathTerm::Add(left, right) => {
            Some(evaluate_integer_math_term(left)?.add(&evaluate_integer_math_term(right)?))
        }
        IntegerMathTerm::Subtract(left, right) => {
            Some(evaluate_integer_math_term(left)?.sub(&evaluate_integer_math_term(right)?))
        }
        IntegerMathTerm::Multiply(left, right) => {
            Some(evaluate_integer_math_term(left)?.mul(&evaluate_integer_math_term(right)?))
        }
        IntegerMathTerm::ShiftLeft { value, count } => {
            let count = usize::try_from(evaluate_integer_math_term(count)?.to_u64()?).ok()?;
            Some(evaluate_integer_math_term(value)?.shl_bits(count))
        }
    }
}

fn big_integer_literal(literal: IntegerMathLiteral) -> BigInt {
    let magnitude = BigInt::from_u128(literal.magnitude());
    if literal.negative() {
        BigInt::zero().sub(&magnitude)
    } else {
        magnitude
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
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

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
}
