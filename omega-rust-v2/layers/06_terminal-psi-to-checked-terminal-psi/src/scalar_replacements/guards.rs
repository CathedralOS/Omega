use super::{Environment, Facts, Reason, value};
use omega_psi::*;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Guard {
    subject: ValueId,
    bound: PredicateStep,
    comparison: PredicateOperator,
}

impl Guard {
    pub fn read(
        unit: &LoweredPackage,
        body: &MachineBody,
        operation: &IntegerOperation,
        operands: &[ValueId],
        env: &Environment,
    ) -> Reason<Option<Self>> {
        use IntegerPrimitive as Int;
        use PredicateOperator as Op;
        let op = match operation.primitive {
            Int::LessThan => Op::LessThan,
            Int::LessOrEqual => Op::LessOrEqual,
            Int::GreaterThan => Op::GreaterThan,
            Int::GreaterOrEqual => Op::GreaterOrEqual,
            Int::Equal => Op::Equal,
            Int::NotEqual => Op::NotEqual,
            _ => return Err("unsupported integer condition"),
        };
        if operation.policy != ArithmeticPolicy::Exact || !operation.obligations.is_empty() {
            return Err("invalid integer comparison contract");
        }
        let [left, right] = operands else {
            return Err("integer comparison needs two arguments");
        };
        let left_ty = value(body, *left)?.ty;
        let right_ty = value(body, *right)?.ty;
        if left_ty != right_ty
            || ![left_ty, right_ty]
                .into_iter()
                .all(|ty| matches!(unit.types.get(ty), Some(TypeShape::PrimitiveScalar(ScalarType::Integer {width, ..})) if *width > 0))
        {
            return Err("integer comparison needs bare integer operands");
        }
        let left = env.get(left).ok_or("comparison lacks its left operand")?;
        let right = env.get(right).ok_or("comparison lacks its right operand")?;
        let guard = |subject: &Facts, literal: &Facts, comparison| match (subject.origin, literal.literal.as_ref()) {
            (Some(subject), Some(ScalarLiteral::Integer(bound))) => Some(Self {
                subject,
                bound: PredicateStep::Integer(bound.clone()),
                comparison,
            }),
            _ => None,
        };
        let reversed = match op {
            Op::LessThan => Op::GreaterThan,
            Op::LessOrEqual => Op::GreaterOrEqual,
            Op::GreaterThan => Op::LessThan,
            Op::GreaterOrEqual => Op::LessOrEqual,
            same => same,
        };
        Ok(guard(left, right, op).or_else(|| guard(right, left, reversed)))
    }

    pub fn apply(
        &self,
        truth: bool,
        env: &mut Environment,
    ) {
        use PredicateOperator as Op;
        let op = if truth {
            self.comparison
        } else {
            match self.comparison {
                Op::LessThan => Op::GreaterOrEqual,
                Op::LessOrEqual => Op::GreaterThan,
                Op::GreaterThan => Op::LessOrEqual,
                Op::GreaterOrEqual => Op::LessThan,
                Op::Equal => Op::NotEqual,
                Op::NotEqual => Op::Equal,
                _ => return,
            }
        };
        let predicate = vec![
            PredicateStep::SelfValue,
            self.bound.clone(),
            PredicateStep::Apply(op),
        ];
        for facts in env
            .values_mut()
            .filter(|facts| facts.origin == Some(self.subject))
        {
            facts.assume(predicate.clone());
        }
    }
}
