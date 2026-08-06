use psi_core::{Proposition, PropositionContext, ScalarTerm};

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
        _ => false,
    };
    accepted
        .then_some(())
        .ok_or(KernelError::JudgmentDoesNotEstablishGoal { judgment })
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
    MalformedProposition(psi_core::PropositionError),
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
    use psi_core::{IntegerSign, IntegerType, IntegerValue};

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
}
