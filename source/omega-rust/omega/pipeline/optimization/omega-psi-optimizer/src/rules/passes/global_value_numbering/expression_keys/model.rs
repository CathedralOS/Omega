//! Expression-key models and identity-preserving operand translation.

use std::collections::{BTreeMap, BTreeSet};

use psi_core::{IntegerType, IntegerValue, ScalarType, ValueId};

use super::canonical_pair;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::rules::passes::global_value_numbering) enum TotalScalarExpressionKey {
    BooleanConstant(bool),
    IntegerConstant(ScalarType, IntegerValue),
    BooleanNot(ValueId),
    BooleanEqual(ValueId, ValueId),
    IntegerEqual(IntegerType, ValueId, ValueId),
    IntegerLessThan(IntegerType, ValueId, ValueId),
    IntegerLessOrEqual(IntegerType, ValueId, ValueId),
    IntegerBitwiseNot(IntegerType, ValueId),
    IntegerWiden(IntegerType, IntegerType, ValueId),
    IntegerBitwiseAnd(IntegerType, ValueId, ValueId),
    IntegerBitwiseOr(IntegerType, ValueId, ValueId),
    IntegerBitwiseXor(IntegerType, ValueId, ValueId),
    WrappingShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
    WrappingShiftRight(IntegerType, IntegerType, ValueId, ValueId),
    WrappingAdd(IntegerType, ValueId, ValueId),
    WrappingSubtract(IntegerType, ValueId, ValueId),
    WrappingMultiply(IntegerType, ValueId, ValueId),
    SaturatingAdd(IntegerType, ValueId, ValueId),
    SaturatingSubtract(IntegerType, ValueId, ValueId),
    SaturatingMultiply(IntegerType, ValueId, ValueId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::rules::passes) enum CompatiblePolicyScalarExpressionKey {
    ShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
    ShiftRight(IntegerType, IntegerType, ValueId, ValueId),
    Add(IntegerType, ValueId, ValueId),
    Subtract(IntegerType, ValueId, ValueId),
    Multiply(IntegerType, ValueId, ValueId),
}

impl CompatiblePolicyScalarExpressionKey {
    pub(in crate::rules::passes::global_value_numbering) fn references_any(
        self,
        values: &BTreeSet<ValueId>,
    ) -> bool {
        match self {
            Self::ShiftLeft(_, _, left, right)
            | Self::ShiftRight(_, _, left, right)
            | Self::Add(_, left, right)
            | Self::Subtract(_, left, right)
            | Self::Multiply(_, left, right) => values.contains(&left) || values.contains(&right),
        }
    }

    pub(in crate::rules::passes::global_value_numbering) fn translate(
        self,
        values: &BTreeMap<ValueId, ValueId>,
    ) -> Self {
        let value = |operand: ValueId| values.get(&operand).copied().unwrap_or(operand);
        match self {
            Self::ShiftLeft(value_type, count_type, operand, count) => {
                Self::ShiftLeft(value_type, count_type, value(operand), value(count))
            }
            Self::ShiftRight(value_type, count_type, operand, count) => {
                Self::ShiftRight(value_type, count_type, value(operand), value(count))
            }
            Self::Add(scalar_type, left, right) => {
                let (left, right) = canonical_pair(value(left), value(right));
                Self::Add(scalar_type, left, right)
            }
            Self::Subtract(scalar_type, left, right) => {
                Self::Subtract(scalar_type, value(left), value(right))
            }
            Self::Multiply(scalar_type, left, right) => {
                let (left, right) = canonical_pair(value(left), value(right));
                Self::Multiply(scalar_type, left, right)
            }
        }
    }
}

impl TotalScalarExpressionKey {
    pub(in crate::rules::passes::global_value_numbering) fn references_any(
        self,
        values: &BTreeSet<ValueId>,
    ) -> bool {
        match self {
            Self::BooleanConstant(_) | Self::IntegerConstant(_, _) => false,
            Self::BooleanNot(value)
            | Self::IntegerBitwiseNot(_, value)
            | Self::IntegerWiden(_, _, value) => values.contains(&value),
            Self::BooleanEqual(left, right)
            | Self::IntegerEqual(_, left, right)
            | Self::IntegerLessThan(_, left, right)
            | Self::IntegerLessOrEqual(_, left, right)
            | Self::IntegerBitwiseAnd(_, left, right)
            | Self::IntegerBitwiseOr(_, left, right)
            | Self::IntegerBitwiseXor(_, left, right)
            | Self::WrappingAdd(_, left, right)
            | Self::WrappingSubtract(_, left, right)
            | Self::WrappingMultiply(_, left, right)
            | Self::SaturatingAdd(_, left, right)
            | Self::SaturatingSubtract(_, left, right)
            | Self::SaturatingMultiply(_, left, right) => {
                values.contains(&left) || values.contains(&right)
            }
            Self::WrappingShiftLeft(_, _, value, count)
            | Self::WrappingShiftRight(_, _, value, count) => {
                values.contains(&value) || values.contains(&count)
            }
        }
    }

    pub(in crate::rules::passes::global_value_numbering) fn translate(
        self,
        values: &BTreeMap<ValueId, ValueId>,
    ) -> Option<Self> {
        let value = |operand: ValueId| Some(values.get(&operand).copied().unwrap_or(operand));
        let commutative = |left: ValueId, right: ValueId| {
            let left = value(left)?;
            let right = value(right)?;
            Some(canonical_pair(left, right))
        };
        Some(match self {
            Self::BooleanConstant(constant) => Self::BooleanConstant(constant),
            Self::IntegerConstant(scalar_type, constant) => {
                Self::IntegerConstant(scalar_type, constant)
            }
            Self::BooleanNot(operand) => Self::BooleanNot(value(operand)?),
            Self::BooleanEqual(left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::BooleanEqual(left, right)
            }
            Self::IntegerEqual(scalar_type, left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::IntegerEqual(scalar_type, left, right)
            }
            Self::IntegerLessThan(scalar_type, left, right) => {
                Self::IntegerLessThan(scalar_type, value(left)?, value(right)?)
            }
            Self::IntegerLessOrEqual(scalar_type, left, right) => {
                Self::IntegerLessOrEqual(scalar_type, value(left)?, value(right)?)
            }
            Self::IntegerBitwiseNot(scalar_type, operand) => {
                Self::IntegerBitwiseNot(scalar_type, value(operand)?)
            }
            Self::IntegerWiden(source_type, target_type, operand) => {
                Self::IntegerWiden(source_type, target_type, value(operand)?)
            }
            Self::IntegerBitwiseAnd(scalar_type, left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::IntegerBitwiseAnd(scalar_type, left, right)
            }
            Self::IntegerBitwiseOr(scalar_type, left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::IntegerBitwiseOr(scalar_type, left, right)
            }
            Self::IntegerBitwiseXor(scalar_type, left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::IntegerBitwiseXor(scalar_type, left, right)
            }
            Self::WrappingShiftLeft(value_type, count_type, operand, count) => {
                Self::WrappingShiftLeft(value_type, count_type, value(operand)?, value(count)?)
            }
            Self::WrappingShiftRight(value_type, count_type, operand, count) => {
                Self::WrappingShiftRight(value_type, count_type, value(operand)?, value(count)?)
            }
            Self::WrappingAdd(scalar_type, left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::WrappingAdd(scalar_type, left, right)
            }
            Self::WrappingSubtract(scalar_type, left, right) => {
                Self::WrappingSubtract(scalar_type, value(left)?, value(right)?)
            }
            Self::WrappingMultiply(scalar_type, left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::WrappingMultiply(scalar_type, left, right)
            }
            Self::SaturatingAdd(scalar_type, left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::SaturatingAdd(scalar_type, left, right)
            }
            Self::SaturatingSubtract(scalar_type, left, right) => {
                Self::SaturatingSubtract(scalar_type, value(left)?, value(right)?)
            }
            Self::SaturatingMultiply(scalar_type, left, right) => {
                let (left, right) = commutative(left, right)?;
                Self::SaturatingMultiply(scalar_type, left, right)
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::rules::passes) enum ProofCertifiedScalarExpressionKey {
    ExactCast(IntegerType, IntegerType, ValueId),
    ExactShiftLeft(IntegerType, IntegerType, ValueId, ValueId),
    ExactShiftRight(IntegerType, IntegerType, ValueId, ValueId),
    ExactAdd(IntegerType, ValueId, ValueId),
    ExactSubtract(IntegerType, ValueId, ValueId),
    ExactMultiply(IntegerType, ValueId, ValueId),
    ExactDivide(IntegerType, ValueId, ValueId),
    ExactRemainder(IntegerType, ValueId, ValueId),
    WrappingDivide(IntegerType, ValueId, ValueId),
    WrappingRemainder(IntegerType, ValueId, ValueId),
    SaturatingDivide(IntegerType, ValueId, ValueId),
    SaturatingRemainder(IntegerType, ValueId, ValueId),
}

impl ProofCertifiedScalarExpressionKey {
    pub(in crate::rules::passes::global_value_numbering) fn references_any(
        self,
        values: &BTreeSet<ValueId>,
    ) -> bool {
        match self {
            Self::ExactCast(_, _, operand) => values.contains(&operand),
            Self::ExactShiftLeft(_, _, left, right)
            | Self::ExactShiftRight(_, _, left, right)
            | Self::ExactAdd(_, left, right)
            | Self::ExactSubtract(_, left, right)
            | Self::ExactMultiply(_, left, right)
            | Self::ExactDivide(_, left, right)
            | Self::ExactRemainder(_, left, right)
            | Self::WrappingDivide(_, left, right)
            | Self::WrappingRemainder(_, left, right)
            | Self::SaturatingDivide(_, left, right)
            | Self::SaturatingRemainder(_, left, right) => {
                values.contains(&left) || values.contains(&right)
            }
        }
    }

    pub(in crate::rules::passes::global_value_numbering) fn translate(
        self,
        values: &BTreeMap<ValueId, ValueId>,
    ) -> Self {
        let value = |operand: ValueId| values.get(&operand).copied().unwrap_or(operand);
        match self {
            Self::ExactCast(source_type, target_type, operand) => {
                Self::ExactCast(source_type, target_type, value(operand))
            }
            Self::ExactShiftLeft(value_type, count_type, operand, count) => {
                Self::ExactShiftLeft(value_type, count_type, value(operand), value(count))
            }
            Self::ExactShiftRight(value_type, count_type, operand, count) => {
                Self::ExactShiftRight(value_type, count_type, value(operand), value(count))
            }
            Self::ExactAdd(scalar_type, left, right) => {
                let (left, right) = canonical_pair(value(left), value(right));
                Self::ExactAdd(scalar_type, left, right)
            }
            Self::ExactSubtract(scalar_type, left, right) => {
                Self::ExactSubtract(scalar_type, value(left), value(right))
            }
            Self::ExactMultiply(scalar_type, left, right) => {
                let (left, right) = canonical_pair(value(left), value(right));
                Self::ExactMultiply(scalar_type, left, right)
            }
            Self::ExactDivide(scalar_type, left, right) => {
                Self::ExactDivide(scalar_type, value(left), value(right))
            }
            Self::ExactRemainder(scalar_type, left, right) => {
                Self::ExactRemainder(scalar_type, value(left), value(right))
            }
            Self::WrappingDivide(scalar_type, left, right) => {
                Self::WrappingDivide(scalar_type, value(left), value(right))
            }
            Self::WrappingRemainder(scalar_type, left, right) => {
                Self::WrappingRemainder(scalar_type, value(left), value(right))
            }
            Self::SaturatingDivide(scalar_type, left, right) => {
                Self::SaturatingDivide(scalar_type, value(left), value(right))
            }
            Self::SaturatingRemainder(scalar_type, left, right) => {
                Self::SaturatingRemainder(scalar_type, value(left), value(right))
            }
        }
    }
}
