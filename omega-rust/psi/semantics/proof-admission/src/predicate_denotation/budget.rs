use semantic_vocabulary::{ContentTerm, IntegerMathTerm, Proposition, ScalarTerm};

use super::PredicateDenotationError;

pub(super) struct Budget {
    remaining: usize,
}

impl Budget {
    pub(super) fn new() -> Self {
        Self { remaining: 4096 }
    }

    pub(super) fn step(&mut self, depth: usize) -> Result<(), PredicateDenotationError> {
        if depth >= 64 {
            return Err(PredicateDenotationError::ResourceLimitExceeded);
        }
        self.count(1)
    }

    fn count(&mut self, count: usize) -> Result<(), PredicateDenotationError> {
        self.remaining = self
            .remaining
            .checked_sub(count)
            .ok_or(PredicateDenotationError::ResourceLimitExceeded)?;
        Ok(())
    }

    pub(super) fn proposition(
        &mut self,
        value: &Proposition,
        depth: usize,
    ) -> Result<(), PredicateDenotationError> {
        self.step(depth)?;
        match value {
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
                self.scalar(left, depth + 1)?;
                self.scalar(right, depth + 1)?;
            }
            Proposition::IntegerMathEqual(left, right)
            | Proposition::IntegerMathLessThan(left, right)
            | Proposition::IntegerMathLessOrEqual(left, right) => {
                self.mathematical(left, depth + 1)?;
                self.mathematical(right, depth + 1)?;
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                for child in children {
                    self.proposition(child, depth + 1)?;
                }
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                self.proposition(premise, depth + 1)?;
                self.proposition(conclusion, depth + 1)?;
            }
            Proposition::IeeeFloatComparison { left, right, .. } => {
                self.count(left.path().len())?;
                self.count(right.path().len())?;
            }
            Proposition::ByteSequenceEqual { left, right } => {
                self.count(left.path().len())?;
                self.count(right.path().len())?;
            }
            Proposition::StructuralCaseMembership { subject, .. } => {
                self.count(subject.path().len())?
            }
            Proposition::ContentConservation(value) => {
                self.content(value.left(), depth + 1)?;
                self.content(value.right(), depth + 1)?;
            }
            Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {}
        }
        Ok(())
    }

    fn content(
        &mut self,
        value: &ContentTerm,
        depth: usize,
    ) -> Result<(), PredicateDenotationError> {
        self.step(depth)?;
        match value {
            ContentTerm::Projection { subject, .. } => self.count(subject.segments.len())?,
            ContentTerm::Separate(children) => {
                for child in children {
                    self.content(child, depth + 1)?;
                }
            }
        }
        Ok(())
    }

    fn mathematical(
        &mut self,
        value: &IntegerMathTerm,
        depth: usize,
    ) -> Result<(), PredicateDenotationError> {
        self.step(depth)?;
        match value {
            IntegerMathTerm::Add(left, right)
            | IntegerMathTerm::Subtract(left, right)
            | IntegerMathTerm::Multiply(left, right)
            | IntegerMathTerm::ShiftLeft {
                value: left,
                count: right,
            } => {
                self.mathematical(left, depth + 1)?;
                self.mathematical(right, depth + 1)?;
            }
            IntegerMathTerm::IntegerLiteral(_) | IntegerMathTerm::MathValue { .. } => {}
        }
        Ok(())
    }

    fn scalar(&mut self, value: &ScalarTerm, depth: usize) -> Result<(), PredicateDenotationError> {
        self.step(depth)?;
        match value {
            ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
            ScalarTerm::BooleanField { path, .. } | ScalarTerm::IntegerField { path, .. } => {
                self.count(path.len())?
            }
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => self.scalar(operand, depth + 1)?,
            ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. }
            | ScalarTerm::WrappingIntegerShiftLeft {
                value: left,
                count: right,
                ..
            }
            | ScalarTerm::WrappingIntegerShiftRight {
                value: left,
                count: right,
                ..
            }
            | ScalarTerm::ExactIntegerShiftLeft {
                value: left,
                count: right,
                ..
            }
            | ScalarTerm::ExactIntegerShiftRight {
                value: left,
                count: right,
                ..
            }
            | ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::ExactIntegerDivide { left, right, .. }
            | ScalarTerm::ExactIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerDivide { left, right, .. }
            | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
            | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
            | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
                self.scalar(left, depth + 1)?;
                self.scalar(right, depth + 1)?;
            }
        }
        Ok(())
    }
}
