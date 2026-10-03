//! Value substitution preserves the surrounding proposition and term operators.

use super::{IntegerMathTerm, Proposition, ScalarTerm, ValueId};

enum Pending<'a> {
    Proposition(&'a mut Proposition),
    Scalar(&'a mut ScalarTerm),
    IntegerMath(&'a mut IntegerMathTerm),
}

impl Proposition {
    /// Rewrite value coordinates, including mathematical embeddings, without
    /// changing structural or opaque subjects. The caller owns the namespace
    /// correspondence and must revalidate canonical ordering if it changes.
    pub fn remap_value_ids(&mut self, mut map: impl FnMut(ValueId) -> ValueId) {
        let mut pending = vec![Pending::Proposition(self)];
        while let Some(node) = pending.pop() {
            match node {
                Pending::Proposition(proposition) => match proposition {
                    Self::Truth
                    | Self::Falsehood
                    | Self::Atom(_)
                    | Self::IeeeFloatComparison { .. }
                    | Self::ByteSequenceEqual { .. }
                    | Self::StructuralCaseMembership { .. }
                    | Self::ContentConservation(_) => {}
                    Self::Equal(left, right)
                    | Self::LessThan(left, right)
                    | Self::LessOrEqual(left, right)
                    | Self::ScalarIeeeFloatComparison { left, right, .. } => {
                        pending.push(Pending::Scalar(right));
                        pending.push(Pending::Scalar(left));
                    }
                    Self::IntegerMathEqual(left, right)
                    | Self::IntegerMathLessThan(left, right)
                    | Self::IntegerMathLessOrEqual(left, right) => {
                        pending.push(Pending::IntegerMath(right));
                        pending.push(Pending::IntegerMath(left));
                    }
                    Self::Conjunction(parts) | Self::Disjunction(parts) => {
                        pending.extend(parts.iter_mut().rev().map(Pending::Proposition));
                    }
                    Self::Implication {
                        premise,
                        conclusion,
                    } => {
                        pending.push(Pending::Proposition(conclusion));
                        pending.push(Pending::Proposition(premise));
                    }
                },
                Pending::IntegerMath(term) => match term {
                    IntegerMathTerm::IntegerLiteral(_) => {}
                    IntegerMathTerm::MathValue { value, .. } => *value = map(*value),
                    IntegerMathTerm::Add(left, right)
                    | IntegerMathTerm::Subtract(left, right)
                    | IntegerMathTerm::Multiply(left, right)
                    | IntegerMathTerm::ShiftLeft {
                        value: left,
                        count: right,
                    } => {
                        pending.push(Pending::IntegerMath(right));
                        pending.push(Pending::IntegerMath(left));
                    }
                },
                Pending::Scalar(term) => match term {
                    ScalarTerm::Value { id, .. } => *id = map(*id),
                    ScalarTerm::BooleanField { .. }
                    | ScalarTerm::IntegerField { .. }
                    | ScalarTerm::IeeeFloatField { .. }
                    | ScalarTerm::ViewExtent { .. }
                    | ScalarTerm::ByteSequencePredicate { .. }
                    | ScalarTerm::Boolean(_)
                    | ScalarTerm::Integer { .. }
                    | ScalarTerm::IeeeFloatLiteral(_) => {}
                    ScalarTerm::BooleanNot { operand }
                    | ScalarTerm::IntegerBitwiseNot { operand, .. }
                    | ScalarTerm::IntegerWiden { operand, .. }
                    | ScalarTerm::IntegerExactCast { operand, .. } => {
                        pending.push(Pending::Scalar(operand));
                    }
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
                        pending.push(Pending::Scalar(right));
                        pending.push(Pending::Scalar(left));
                    }
                },
            }
        }
    }
}
