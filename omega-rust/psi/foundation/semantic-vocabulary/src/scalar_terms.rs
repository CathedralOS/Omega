/*
We define `ScalarType`, `ScalarTerm`, IEEE-754 float and byte-sequence
structural fields, canonical structural paths, and the validation (`validation`)
and value-reference query (`value_queries`) passes over scalar terms.
*/

use crate::proposition::{
    IntegerMathTerm, IntegerType, IntegerValue, Proposition, PropositionError,
};
use crate::{
    ByteSequencePredicate, PlaceId, StructuralCaseId, StructuralFieldId, StructuralTypeId, ValueId,
};

mod validation {
    use super::{
        ScalarTerm, ScalarType, Step, validate_exact_integer_operands,
        validate_integer_comparison_operands, validate_integer_operands,
        validate_integer_shift_operands,
    };
    use crate::proposition::{IntegerType, PropositionError};

    impl ScalarTerm {
        pub fn validate(&self) -> Result<(), PropositionError> {
            let mut pending = vec![Step::Visit(self)];
            while let Some(step) = pending.pop() {
                match step {
                    Step::Visit(term) => {
                        pending.push(Step::Finalize(term));
                        for operand in term.operands().into_iter().rev() {
                            pending.push(Step::Visit(operand));
                        }
                    }
                    Step::Finalize(term) => term.validate_after_operands()?,
                }
            }
            Ok(())
        }

        /// Checks this node alone, given operands that already passed this check.
        pub fn validate_after_operands(&self) -> Result<(), PropositionError> {
            if matches!(self.scalar_type(), ScalarType::Callable(_)) {
                return Err(PropositionError::OpaqueCallableInProof);
            }
            match self {
                Self::StructuralRead { path, .. } => {
                    if path.is_empty() {
                        return Err(PropositionError::EmptyStructuralReadPath);
                    }
                    if path.iter().any(|projection| {
                        matches!(
                            projection,
                            super::ScalarReadProjection::Static(
                                super::CanonicalStructuralPathSegment::Referent
                            )
                        )
                    }) {
                        return Err(PropositionError::NonCanonicalScalarReadProjection);
                    }
                    for operand in self.operands() {
                        if operand
                            .scalar_type()
                            .as_integer()
                            .is_none_or(|integer| integer.is_address())
                        {
                            return Err(PropositionError::StructuralReadIndexType(
                                operand.scalar_type(),
                            ));
                        }
                    }
                    Ok(())
                }
                Self::Application { .. }
                | Self::Value { .. }
                | Self::BooleanField { .. }
                | Self::IntegerField { .. }
                | Self::ViewExtent { .. }
                | Self::ByteSequencePredicate { .. }
                | Self::IeeeFloatField { .. }
                | Self::Boolean(_)
                | Self::IeeeFloatLiteral(_) => Ok(()),
                Self::BooleanNot { operand } => {
                    if operand.scalar_type() != ScalarType::Boolean {
                        return Err(PropositionError::BooleanNotTypeMismatch(
                            operand.scalar_type(),
                        ));
                    }
                    Ok(())
                }
                Self::IntegerExactCast {
                    source_type,
                    target_type,
                    operand,
                } => {
                    let expected = ScalarType::Integer(*source_type);
                    if operand.scalar_type() != expected
                        || !source_type.can_exact_cast_to(*target_type)
                    {
                        return Err(PropositionError::IntegerExactCastTypeMismatch {
                            source: expected,
                            target: ScalarType::Integer(*target_type),
                            operand: operand.scalar_type(),
                        });
                    }
                    Ok(())
                }
                Self::BooleanEqual { left, right } => {
                    if left.scalar_type() != ScalarType::Boolean
                        || right.scalar_type() != ScalarType::Boolean
                    {
                        return Err(PropositionError::BooleanEqualTypeMismatch {
                            left: left.scalar_type(),
                            right: right.scalar_type(),
                        });
                    }
                    Ok(())
                }
                Self::IntegerEqual {
                    scalar_type,
                    left,
                    right,
                }
                | Self::IntegerLessThan {
                    scalar_type,
                    left,
                    right,
                }
                | Self::IntegerLessOrEqual {
                    scalar_type,
                    left,
                    right,
                } => validate_integer_comparison_operands(*scalar_type, left, right),
                Self::IntegerBitwiseNot {
                    scalar_type,
                    operand,
                } => {
                    let expected = ScalarType::Integer(*scalar_type);
                    if operand.scalar_type() != expected {
                        return Err(PropositionError::IntegerBitwiseNotTypeMismatch {
                            expected,
                            operand: operand.scalar_type(),
                        });
                    }
                    Ok(())
                }
                Self::IntegerWiden {
                    source_type,
                    target_type,
                    operand,
                } => {
                    let expected = ScalarType::Integer(*source_type);
                    if operand.scalar_type() != expected || !source_type.can_widen_to(*target_type)
                    {
                        return Err(PropositionError::IntegerWidenTypeMismatch {
                            source: expected,
                            target: ScalarType::Integer(*target_type),
                            operand: operand.scalar_type(),
                        });
                    }
                    Ok(())
                }
                Self::IntegerBitwiseAnd {
                    scalar_type,
                    left,
                    right,
                }
                | Self::IntegerBitwiseOr {
                    scalar_type,
                    left,
                    right,
                }
                | Self::IntegerBitwiseXor {
                    scalar_type,
                    left,
                    right,
                } => validate_integer_operands(*scalar_type, left, right),
                Self::WrappingIntegerShiftLeft {
                    value_type,
                    count_type,
                    value,
                    count,
                }
                | Self::WrappingIntegerShiftRight {
                    value_type,
                    count_type,
                    value,
                    count,
                }
                | Self::ExactIntegerShiftLeft {
                    value_type,
                    count_type,
                    value,
                    count,
                }
                | Self::ExactIntegerShiftRight {
                    value_type,
                    count_type,
                    value,
                    count,
                } => validate_integer_shift_operands(*value_type, *count_type, value, count),
                Self::Integer { scalar_type, value } => {
                    if scalar_type.admits(*value) {
                        Ok(())
                    } else {
                        Err(PropositionError::IntegerLiteralOutsideType {
                            scalar_type: *scalar_type,
                            value: *value,
                        })
                    }
                }
                Self::ExactIntegerSubtract {
                    scalar_type,
                    left,
                    right,
                } => validate_exact_integer_operands(
                    *scalar_type,
                    left,
                    right,
                    IntegerType::admits_exact_subtract_operands,
                ),
                Self::ExactIntegerAdd {
                    scalar_type,
                    left,
                    right,
                } => validate_exact_integer_operands(
                    *scalar_type,
                    left,
                    right,
                    IntegerType::admits_exact_add_operands,
                ),
                Self::ExactIntegerMultiply {
                    scalar_type,
                    left,
                    right,
                }
                | Self::ExactIntegerDivide {
                    scalar_type,
                    left,
                    right,
                }
                | Self::ExactIntegerRemainder {
                    scalar_type,
                    left,
                    right,
                }
                | Self::WrappingIntegerDivide {
                    scalar_type,
                    left,
                    right,
                }
                | Self::WrappingIntegerRemainder {
                    scalar_type,
                    left,
                    right,
                }
                | Self::SaturatingIntegerDivide {
                    scalar_type,
                    left,
                    right,
                }
                | Self::SaturatingIntegerRemainder {
                    scalar_type,
                    left,
                    right,
                }
                | Self::WrappingIntegerAdd {
                    scalar_type,
                    left,
                    right,
                }
                | Self::SaturatingIntegerAdd {
                    scalar_type,
                    left,
                    right,
                }
                | Self::WrappingIntegerSubtract {
                    scalar_type,
                    left,
                    right,
                }
                | Self::SaturatingIntegerSubtract {
                    scalar_type,
                    left,
                    right,
                }
                | Self::WrappingIntegerMultiply {
                    scalar_type,
                    left,
                    right,
                }
                | Self::SaturatingIntegerMultiply {
                    scalar_type,
                    left,
                    right,
                } => validate_integer_operands(*scalar_type, left, right),
            }
        }
    }

    #[cfg(test)]
    mod scalar_read_projection_tests {
        use super::*;
        use crate::{CanonicalStructuralPathSegment, PlaceId, ScalarReadProjection};

        #[test]
        fn predicate_reference_step_becomes_one_explicit_scalar_read_step() {
            let projection = ScalarReadProjection::from(CanonicalStructuralPathSegment::Referent);
            assert_eq!(projection, ScalarReadProjection::Referent);
            ScalarTerm::structural_read(
                PlaceId::new(1).unwrap(),
                vec![projection],
                ScalarType::Boolean,
            )
            .unwrap();
        }

        #[test]
        fn scalar_read_rejects_the_duplicate_static_reference_spelling() {
            let term = ScalarTerm::StructuralRead {
                root: PlaceId::new(1).unwrap(),
                path: vec![ScalarReadProjection::Static(
                    CanonicalStructuralPathSegment::Referent,
                )],
                scalar_type: ScalarType::Boolean,
            };
            assert_eq!(
                term.validate(),
                Err(PropositionError::NonCanonicalScalarReadProjection)
            );
        }
    }
}
mod value_queries {
    use super::{ScalarTerm, ScalarType, Step};
    use crate::proposition::{IntegerType, IntegerValue};

    impl ScalarTerm {
        pub fn scalar_type(&self) -> ScalarType {
            match self {
                Self::Application { scalar_type, .. } => *scalar_type,
                Self::IeeeFloatLiteral(value) => ScalarType::IeeeFloat(value.format()),
                Self::IeeeFloatField { format, .. } => ScalarType::IeeeFloat(*format),
                Self::Value { scalar_type, .. } | Self::StructuralRead { scalar_type, .. } => {
                    *scalar_type
                }
                Self::IntegerField { scalar_type, .. } | Self::ViewExtent { scalar_type, .. } => {
                    ScalarType::Integer(*scalar_type)
                }
                Self::Boolean(_)
                | Self::BooleanField { .. }
                | Self::ByteSequencePredicate { .. }
                | Self::BooleanNot { .. }
                | Self::BooleanEqual { .. }
                | Self::IntegerEqual { .. }
                | Self::IntegerLessThan { .. }
                | Self::IntegerLessOrEqual { .. } => ScalarType::Boolean,
                Self::Integer { scalar_type, .. }
                | Self::IntegerBitwiseNot { scalar_type, .. }
                | Self::IntegerBitwiseAnd { scalar_type, .. }
                | Self::IntegerBitwiseOr { scalar_type, .. }
                | Self::IntegerBitwiseXor { scalar_type, .. }
                | Self::ExactIntegerAdd { scalar_type, .. }
                | Self::ExactIntegerSubtract { scalar_type, .. }
                | Self::ExactIntegerMultiply { scalar_type, .. }
                | Self::ExactIntegerDivide { scalar_type, .. }
                | Self::ExactIntegerRemainder { scalar_type, .. }
                | Self::WrappingIntegerDivide { scalar_type, .. }
                | Self::WrappingIntegerRemainder { scalar_type, .. }
                | Self::SaturatingIntegerDivide { scalar_type, .. }
                | Self::SaturatingIntegerRemainder { scalar_type, .. }
                | Self::WrappingIntegerAdd { scalar_type, .. }
                | Self::SaturatingIntegerAdd { scalar_type, .. }
                | Self::WrappingIntegerSubtract { scalar_type, .. }
                | Self::SaturatingIntegerSubtract { scalar_type, .. }
                | Self::WrappingIntegerMultiply { scalar_type, .. }
                | Self::SaturatingIntegerMultiply { scalar_type, .. } => {
                    ScalarType::Integer(*scalar_type)
                }
                Self::IntegerWiden { target_type, .. }
                | Self::IntegerExactCast { target_type, .. } => ScalarType::Integer(*target_type),
                Self::WrappingIntegerShiftLeft { value_type, .. }
                | Self::WrappingIntegerShiftRight { value_type, .. }
                | Self::ExactIntegerShiftLeft { value_type, .. }
                | Self::ExactIntegerShiftRight { value_type, .. } => {
                    ScalarType::Integer(*value_type)
                }
            }
        }

        pub fn integer_value(&self) -> Option<(IntegerType, IntegerValue)> {
            let mut pending = vec![Step::Visit(self)];
            let mut values = Vec::new();
            while let Some(step) = pending.pop() {
                match step {
                    Step::Visit(term) => match term {
                        Self::IntegerBitwiseNot { operand, .. }
                        | Self::IntegerWiden { operand, .. }
                        | Self::IntegerExactCast { operand, .. } => {
                            pending.push(Step::Finalize(term));
                            pending.push(Step::Visit(operand));
                        }
                        Self::WrappingIntegerShiftLeft { value, count, .. }
                        | Self::WrappingIntegerShiftRight { value, count, .. }
                        | Self::ExactIntegerShiftLeft { value, count, .. }
                        | Self::ExactIntegerShiftRight { value, count, .. } => {
                            pending.push(Step::Finalize(term));
                            pending.push(Step::Visit(count));
                            pending.push(Step::Visit(value));
                        }
                        Self::IntegerBitwiseAnd { left, right, .. }
                        | Self::IntegerBitwiseOr { left, right, .. }
                        | Self::IntegerBitwiseXor { left, right, .. }
                        | Self::ExactIntegerAdd { left, right, .. }
                        | Self::ExactIntegerSubtract { left, right, .. }
                        | Self::ExactIntegerMultiply { left, right, .. }
                        | Self::ExactIntegerDivide { left, right, .. }
                        | Self::ExactIntegerRemainder { left, right, .. }
                        | Self::WrappingIntegerDivide { left, right, .. }
                        | Self::WrappingIntegerRemainder { left, right, .. }
                        | Self::SaturatingIntegerDivide { left, right, .. }
                        | Self::SaturatingIntegerRemainder { left, right, .. }
                        | Self::WrappingIntegerAdd { left, right, .. }
                        | Self::SaturatingIntegerAdd { left, right, .. }
                        | Self::WrappingIntegerSubtract { left, right, .. }
                        | Self::SaturatingIntegerSubtract { left, right, .. }
                        | Self::WrappingIntegerMultiply { left, right, .. }
                        | Self::SaturatingIntegerMultiply { left, right, .. } => {
                            pending.push(Step::Finalize(term));
                            pending.push(Step::Visit(right));
                            pending.push(Step::Visit(left));
                        }
                        _ => pending.push(Step::Finalize(term)),
                    },
                    Step::Finalize(term) => {
                        let value = term.integer_value_after_operands(&mut values);
                        values.push(value);
                    }
                }
            }
            debug_assert_eq!(values.len(), 1);
            values.pop().flatten()
        }

        fn integer_value_after_operands(
            &self,
            operands: &mut Vec<Option<(IntegerType, IntegerValue)>>,
        ) -> Option<(IntegerType, IntegerValue)> {
            match self {
                Self::Integer { scalar_type, value } => Some((*scalar_type, *value)),
                Self::IntegerBitwiseNot { scalar_type, .. } => {
                    let (operand_type, operand) = Self::pop_integer_operand(operands)?;
                    if operand_type != *scalar_type {
                        return None;
                    }
                    Some((*scalar_type, scalar_type.bitwise_not(operand)?))
                }
                Self::IntegerWiden {
                    source_type,
                    target_type,
                    ..
                } => {
                    let (operand_type, operand) = Self::pop_integer_operand(operands)?;
                    if operand_type != *source_type || !source_type.can_widen_to(*target_type) {
                        return None;
                    }
                    Some((
                        *target_type,
                        source_type.widen_value_to(*target_type, operand)?,
                    ))
                }
                Self::IntegerExactCast {
                    source_type,
                    target_type,
                    ..
                } => {
                    let (operand_type, operand) = Self::pop_integer_operand(operands)?;
                    if operand_type != *source_type || !source_type.can_exact_cast_to(*target_type)
                    {
                        return None;
                    }
                    Some((
                        *target_type,
                        source_type.exact_cast_value_to(*target_type, operand)?,
                    ))
                }
                Self::WrappingIntegerAdd { scalar_type, .. }
                | Self::SaturatingIntegerAdd { scalar_type, .. }
                | Self::WrappingIntegerSubtract { scalar_type, .. }
                | Self::SaturatingIntegerSubtract { scalar_type, .. }
                | Self::WrappingIntegerMultiply { scalar_type, .. }
                | Self::SaturatingIntegerMultiply { scalar_type, .. } => {
                    let (left, right) = Self::pop_typed_integer_operands(operands, *scalar_type)?;
                    let value = match self {
                        Self::WrappingIntegerAdd { .. } => scalar_type.wrapping_add(left, right)?,
                        Self::SaturatingIntegerAdd { .. } => {
                            scalar_type.saturating_add(left, right)?
                        }
                        Self::WrappingIntegerSubtract { .. } => {
                            scalar_type.wrapping_sub(left, right)?
                        }
                        Self::SaturatingIntegerSubtract { .. } => {
                            scalar_type.saturating_sub(left, right)?
                        }
                        Self::WrappingIntegerMultiply { .. } => {
                            scalar_type.wrapping_mul(left, right)?
                        }
                        Self::SaturatingIntegerMultiply { .. } => {
                            scalar_type.saturating_mul(left, right)?
                        }
                        _ => unreachable!(),
                    };
                    Some((*scalar_type, value))
                }
                Self::IntegerBitwiseAnd { scalar_type, .. }
                | Self::IntegerBitwiseOr { scalar_type, .. }
                | Self::IntegerBitwiseXor { scalar_type, .. } => {
                    let (left, right) = Self::pop_typed_integer_operands(operands, *scalar_type)?;
                    let value = match self {
                        Self::IntegerBitwiseAnd { .. } => scalar_type.bitwise_and(left, right)?,
                        Self::IntegerBitwiseOr { .. } => scalar_type.bitwise_or(left, right)?,
                        Self::IntegerBitwiseXor { .. } => scalar_type.bitwise_xor(left, right)?,
                        _ => unreachable!(),
                    };
                    Some((*scalar_type, value))
                }
                Self::WrappingIntegerShiftLeft {
                    value_type,
                    count_type,
                    ..
                }
                | Self::WrappingIntegerShiftRight {
                    value_type,
                    count_type,
                    ..
                }
                | Self::ExactIntegerShiftLeft {
                    value_type,
                    count_type,
                    ..
                }
                | Self::ExactIntegerShiftRight {
                    value_type,
                    count_type,
                    ..
                } => {
                    let count_slot = operands.pop().flatten();
                    let value_slot = operands.pop().flatten();
                    let (actual_count_type, count) = count_slot?;
                    let (actual_value_type, value) = value_slot?;
                    if actual_value_type != *value_type || actual_count_type != *count_type {
                        return None;
                    }
                    let result = match self {
                        Self::WrappingIntegerShiftLeft { .. } => {
                            value_type.wrapping_shift_left(value, *count_type, count)?
                        }
                        Self::WrappingIntegerShiftRight { .. } => {
                            value_type.wrapping_shift_right(value, *count_type, count)?
                        }
                        Self::ExactIntegerShiftLeft { .. } => {
                            value_type.exact_shift_left(value, *count_type, count)?
                        }
                        Self::ExactIntegerShiftRight { .. } => {
                            value_type.exact_shift_right(value, *count_type, count)?
                        }
                        _ => unreachable!(),
                    };
                    Some((*value_type, result))
                }
                Self::ExactIntegerAdd { scalar_type, .. }
                | Self::ExactIntegerSubtract { scalar_type, .. } => {
                    let right = operands.pop().flatten();
                    let left = operands.pop().flatten();
                    let (left_type, left) = left?;
                    let (right_type, right) = right?;
                    let value = if matches!(self, Self::ExactIntegerAdd { .. }) {
                        scalar_type.exact_add_values(left_type, left, right_type, right)?
                    } else {
                        scalar_type.exact_subtract_values(left_type, left, right_type, right)?
                    };
                    Some((*scalar_type, value))
                }
                Self::ExactIntegerMultiply { scalar_type, .. }
                | Self::ExactIntegerDivide { scalar_type, .. }
                | Self::ExactIntegerRemainder { scalar_type, .. }
                | Self::WrappingIntegerDivide { scalar_type, .. }
                | Self::WrappingIntegerRemainder { scalar_type, .. }
                | Self::SaturatingIntegerDivide { scalar_type, .. }
                | Self::SaturatingIntegerRemainder { scalar_type, .. } => {
                    let (left, right) = Self::pop_typed_integer_operands(operands, *scalar_type)?;
                    let value = match self {
                        Self::ExactIntegerMultiply { .. } => scalar_type.exact_mul(left, right)?,
                        Self::ExactIntegerDivide { .. } => scalar_type.exact_div(left, right)?,
                        Self::ExactIntegerRemainder { .. } => scalar_type.exact_rem(left, right)?,
                        Self::WrappingIntegerDivide { .. } => {
                            scalar_type.wrapping_div(left, right)?
                        }
                        Self::WrappingIntegerRemainder { .. } => {
                            scalar_type.wrapping_rem(left, right)?
                        }
                        Self::SaturatingIntegerDivide { .. } => {
                            scalar_type.saturating_div(left, right)?
                        }
                        Self::SaturatingIntegerRemainder { .. } => {
                            scalar_type.saturating_rem(left, right)?
                        }
                        _ => unreachable!(),
                    };
                    Some((*scalar_type, value))
                }
                _ => None,
            }
        }

        fn pop_integer_operand(
            operands: &mut Vec<Option<(IntegerType, IntegerValue)>>,
        ) -> Option<(IntegerType, IntegerValue)> {
            operands.pop().flatten()
        }

        fn pop_typed_integer_operands(
            operands: &mut Vec<Option<(IntegerType, IntegerValue)>>,
            scalar_type: IntegerType,
        ) -> Option<(IntegerValue, IntegerValue)> {
            let right = operands.pop().flatten();
            let left = operands.pop().flatten();
            let (right_type, right) = right?;
            let (left_type, left) = left?;
            if left_type != scalar_type || right_type != scalar_type {
                return None;
            }
            Some((left, right))
        }

        pub fn boolean_value(&self) -> Option<bool> {
            let mut pending = vec![Step::Visit(self)];
            let mut values = Vec::new();
            while let Some(step) = pending.pop() {
                match step {
                    Step::Visit(term) => match term {
                        Self::BooleanNot { operand } => {
                            pending.push(Step::Finalize(term));
                            pending.push(Step::Visit(operand));
                        }
                        Self::BooleanEqual { left, right } => {
                            pending.push(Step::Finalize(term));
                            pending.push(Step::Visit(right));
                            pending.push(Step::Visit(left));
                        }
                        _ => pending.push(Step::Finalize(term)),
                    },
                    Step::Finalize(term) => {
                        let value = term.boolean_value_after_operands(&mut values);
                        values.push(value);
                    }
                }
            }
            debug_assert_eq!(values.len(), 1);
            values.pop().flatten()
        }

        fn boolean_value_after_operands(&self, operands: &mut Vec<Option<bool>>) -> Option<bool> {
            match self {
                Self::Boolean(value) => Some(*value),
                Self::BooleanNot { .. } => Some(!operands.pop().flatten()?),
                Self::BooleanEqual { .. } => {
                    let right = operands.pop().flatten();
                    let left = operands.pop().flatten();
                    Some(left? == right?)
                }
                Self::IntegerEqual {
                    scalar_type,
                    left,
                    right,
                } => {
                    let (left_type, left) = left.integer_value()?;
                    let (right_type, right) = right.integer_value()?;
                    (left_type == *scalar_type).then_some(())?;
                    Some(left_type.compare_with(left, right_type, right)?.is_eq())
                }
                Self::IntegerLessThan {
                    scalar_type,
                    left,
                    right,
                } => {
                    let (left_type, left) = left.integer_value()?;
                    let (right_type, right) = right.integer_value()?;
                    if left_type != *scalar_type {
                        return None;
                    }
                    Some(left_type.compare_with(left, right_type, right)?.is_lt())
                }
                Self::IntegerLessOrEqual {
                    scalar_type,
                    left,
                    right,
                } => {
                    let (left_type, left) = left.integer_value()?;
                    let (right_type, right) = right.integer_value()?;
                    if left_type != *scalar_type {
                        return None;
                    }
                    Some(!left_type.compare_with(left, right_type, right)?.is_gt())
                }
                _ => None,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum ScalarType {
    Boolean,
    Integer(IntegerType),
    IeeeFloat(IeeeFloatFormat),
    Callable(crate::CallableContractId),
}

impl ScalarType {
    pub fn as_integer(&self) -> Option<IntegerType> {
        let ScalarType::Integer(integer) = self else {
            return None;
        };
        Some(*integer)
    }

    pub fn as_ieee_float(&self) -> Option<IeeeFloatFormat> {
        let ScalarType::IeeeFloat(format) = self else {
            return None;
        };
        Some(*format)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum CanonicalStructuralPathSegment {
    Field(StructuralFieldId),
    FixedIndex(u64),
    Case(StructuralCaseId),
    Referent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum IeeeFloatFormat {
    Binary32,
    Binary64,
}

impl IeeeFloatFormat {
    pub const fn byte_size(self) -> u16 {
        match self {
            Self::Binary32 => 4,
            Self::Binary64 => 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum IeeeFloatValue {
    Binary32(u32),
    Binary64(u64),
}

impl IeeeFloatValue {
    pub const fn format(self) -> IeeeFloatFormat {
        match self {
            Self::Binary32(_) => IeeeFloatFormat::Binary32,
            Self::Binary64(_) => IeeeFloatFormat::Binary64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum IeeeFloatComparisonKind {
    Equal,
    NotEqual,
    LessThan,
    LessOrEqual,
    NotLessThan,
    NotLessOrEqual,
}

impl IeeeFloatComparisonKind {
    pub const fn negated(self) -> Self {
        match self {
            Self::Equal => Self::NotEqual,
            Self::NotEqual => Self::Equal,
            Self::LessThan => Self::NotLessThan,
            Self::LessOrEqual => Self::NotLessOrEqual,
            Self::NotLessThan => Self::LessThan,
            Self::NotLessOrEqual => Self::LessOrEqual,
        }
    }

    pub const fn is_symmetric(self) -> bool {
        matches!(self, Self::Equal | Self::NotEqual)
    }
}

macro_rules! canonical_field_accessors {
    () => {
        pub const fn root(&self) -> PlaceId {
            self.root
        }

        pub fn path(&self) -> &[CanonicalStructuralPathSegment] {
            &self.path
        }

        pub fn rebase(&self, root: PlaceId, prefix: &[CanonicalStructuralPathSegment]) -> Self {
            let mut path = Vec::with_capacity(prefix.len() + self.path.len());
            path.extend_from_slice(prefix);
            path.extend_from_slice(&self.path);
            Self { root, path }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct IeeeFloatStructuralField {
    pub(crate) root: PlaceId,
    pub(crate) path: Vec<CanonicalStructuralPathSegment>,
}

impl IeeeFloatStructuralField {
    pub fn new(
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
    ) -> Result<Self, PropositionError> {
        if path.is_empty() {
            return Err(PropositionError::EmptyIeeeFloatStructuralFieldPath);
        }
        Ok(Self { root, path })
    }

    canonical_field_accessors!();
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct ByteSequenceStructuralField {
    pub(crate) root: PlaceId,
    pub(crate) path: Vec<CanonicalStructuralPathSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct StructuralCaseSubject {
    pub(crate) root: PlaceId,
    path: Vec<CanonicalStructuralPathSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub struct StructuralIdentitySubject {
    pub(crate) root: PlaceId,
    path: Vec<CanonicalStructuralPathSegment>,
    structural_type: StructuralTypeId,
}

impl StructuralIdentitySubject {
    pub fn new(
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        structural_type: StructuralTypeId,
    ) -> Self {
        Self {
            root,
            path,
            structural_type,
        }
    }

    pub fn root(&self) -> PlaceId {
        self.root
    }
    pub fn path(&self) -> &[CanonicalStructuralPathSegment] {
        &self.path
    }
    pub fn structural_type(&self) -> StructuralTypeId {
        self.structural_type
    }

    pub fn rebase(&self, root: PlaceId, prefix: &[CanonicalStructuralPathSegment]) -> Self {
        let mut path = Vec::with_capacity(prefix.len() + self.path.len());
        path.extend_from_slice(prefix);
        path.extend_from_slice(&self.path);
        Self {
            root,
            path,
            structural_type: self.structural_type,
        }
    }
}

impl StructuralCaseSubject {
    pub fn new(root: PlaceId, path: Vec<CanonicalStructuralPathSegment>) -> Self {
        Self { root, path }
    }

    canonical_field_accessors!();
}

impl ByteSequenceStructuralField {
    pub fn new(
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
    ) -> Result<Self, PropositionError> {
        if path.is_empty() {
            return Err(PropositionError::EmptyByteSequenceStructuralFieldPath);
        }
        Ok(Self { root, path })
    }

    canonical_field_accessors!();
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum ScalarReadProjection {
    Referent,
    Extent,
    Static(CanonicalStructuralPathSegment),
    Index(Box<ScalarTerm>),
}

impl From<CanonicalStructuralPathSegment> for ScalarReadProjection {
    fn from(segment: CanonicalStructuralPathSegment) -> Self {
        match segment {
            CanonicalStructuralPathSegment::Referent => Self::Referent,
            segment => Self::Static(segment),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum ScalarTerm {
    Application {
        application: crate::ScalarApplicationId,
        scalar_type: ScalarType,
    },
    StructuralRead {
        root: PlaceId,
        path: Vec<ScalarReadProjection>,
        scalar_type: ScalarType,
    },
    IeeeFloatLiteral(IeeeFloatValue),
    Value {
        id: ValueId,
        scalar_type: ScalarType,
    },
    BooleanField {
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
    },
    IntegerField {
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        scalar_type: IntegerType,
    },
    IeeeFloatField {
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        format: IeeeFloatFormat,
    },
    ViewExtent {
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        scalar_type: IntegerType,
    },
    Boolean(bool),
    BooleanNot {
        operand: Box<ScalarTerm>,
    },
    BooleanEqual {
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerEqual {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerLessThan {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerLessOrEqual {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerBitwiseAnd {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerBitwiseNot {
        scalar_type: IntegerType,
        operand: Box<ScalarTerm>,
    },
    IntegerWiden {
        source_type: IntegerType,
        target_type: IntegerType,
        operand: Box<ScalarTerm>,
    },
    IntegerExactCast {
        source_type: IntegerType,
        target_type: IntegerType,
        operand: Box<ScalarTerm>,
    },
    IntegerBitwiseOr {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerBitwiseXor {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    WrappingIntegerShiftLeft {
        value_type: IntegerType,
        count_type: IntegerType,
        value: Box<ScalarTerm>,
        count: Box<ScalarTerm>,
    },
    WrappingIntegerShiftRight {
        value_type: IntegerType,
        count_type: IntegerType,
        value: Box<ScalarTerm>,
        count: Box<ScalarTerm>,
    },
    ExactIntegerShiftLeft {
        value_type: IntegerType,
        count_type: IntegerType,
        value: Box<ScalarTerm>,
        count: Box<ScalarTerm>,
    },
    ExactIntegerShiftRight {
        value_type: IntegerType,
        count_type: IntegerType,
        value: Box<ScalarTerm>,
        count: Box<ScalarTerm>,
    },
    ExactIntegerAdd {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    ExactIntegerSubtract {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    ExactIntegerMultiply {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    ExactIntegerDivide {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    ExactIntegerRemainder {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    WrappingIntegerDivide {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    WrappingIntegerRemainder {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    SaturatingIntegerDivide {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    SaturatingIntegerRemainder {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    Integer {
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    WrappingIntegerAdd {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    SaturatingIntegerAdd {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    WrappingIntegerSubtract {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    SaturatingIntegerSubtract {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    WrappingIntegerMultiply {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    SaturatingIntegerMultiply {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    ByteSequencePredicate {
        predicate: ByteSequencePredicate,
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
    },
}

#[macro_export]
macro_rules! scalar_term_unary_operand {
    ($operand:ident) => {
        $crate::ScalarTerm::BooleanNot { operand: $operand }
            | $crate::ScalarTerm::IntegerBitwiseNot {
                operand: $operand,
                ..
            }
            | $crate::ScalarTerm::IntegerWiden {
                operand: $operand,
                ..
            }
            | $crate::ScalarTerm::IntegerExactCast {
                operand: $operand,
                ..
            }
    };
}

#[macro_export]
macro_rules! scalar_term_binary_operands {
    ($left:ident, $right:ident) => {
        $crate::ScalarTerm::BooleanEqual {
            left: $left,
            right: $right,
        } | $crate::ScalarTerm::IntegerEqual {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::IntegerLessThan {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::IntegerLessOrEqual {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::IntegerBitwiseAnd {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::IntegerBitwiseOr {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::IntegerBitwiseXor {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::WrappingIntegerShiftLeft {
            value: $left,
            count: $right,
            ..
        } | $crate::ScalarTerm::WrappingIntegerShiftRight {
            value: $left,
            count: $right,
            ..
        } | $crate::ScalarTerm::ExactIntegerShiftLeft {
            value: $left,
            count: $right,
            ..
        } | $crate::ScalarTerm::ExactIntegerShiftRight {
            value: $left,
            count: $right,
            ..
        } | $crate::ScalarTerm::ExactIntegerAdd {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::ExactIntegerSubtract {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::ExactIntegerMultiply {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::ExactIntegerDivide {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::ExactIntegerRemainder {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::WrappingIntegerDivide {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::WrappingIntegerRemainder {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::SaturatingIntegerDivide {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::SaturatingIntegerRemainder {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::WrappingIntegerAdd {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::SaturatingIntegerAdd {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::WrappingIntegerSubtract {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::SaturatingIntegerSubtract {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::WrappingIntegerMultiply {
            left: $left,
            right: $right,
            ..
        } | $crate::ScalarTerm::SaturatingIntegerMultiply {
            left: $left,
            right: $right,
            ..
        }
    };
}

macro_rules! integer_binary_term {
    ($name:ident => $variant:ident) => {
        integer_binary_term!(@valid $name => $variant, validate_integer_operands);
    };
    (@valid $name:ident => $variant:ident, $validate:ident) => {
        pub fn $name(
            scalar_type: IntegerType,
            left: ScalarTerm,
            right: ScalarTerm,
        ) -> Result<Self, PropositionError> {
            $validate(scalar_type, &left, &right)?;
            Ok(Self::$variant {
                scalar_type,
                left: Box::new(left),
                right: Box::new(right),
            })
        }
    };
}

macro_rules! integer_shift_term {
    ($name:ident => $variant:ident) => {
        pub fn $name(
            value_type: IntegerType,
            count_type: IntegerType,
            value: ScalarTerm,
            count: ScalarTerm,
        ) -> Result<Self, PropositionError> {
            validate_integer_shift_operands(value_type, count_type, &value, &count)?;
            Ok(Self::$variant {
                value_type,
                count_type,
                value: Box::new(value),
                count: Box::new(count),
            })
        }
    };
}

macro_rules! scalar_term_operand_table {
    ($self:expr, $borrow:ident) => {
        match $self {
            scalar_term_unary_operand!(operand) => vec![operand.$borrow()],
            scalar_term_binary_operands!(left, right) => vec![left.$borrow(), right.$borrow()],
            _ => Vec::new(),
        }
    };
}

impl ScalarTerm {
    pub fn structural_read(
        root: PlaceId,
        path: Vec<ScalarReadProjection>,
        scalar_type: ScalarType,
    ) -> Result<Self, PropositionError> {
        let term = Self::StructuralRead {
            root,
            path,
            scalar_type,
        };
        term.validate()?;
        Ok(term)
    }

    pub fn value(id: ValueId, scalar_type: ScalarType) -> Self {
        Self::Value { id, scalar_type }
    }

    pub fn boolean_field(root: PlaceId, field: StructuralFieldId) -> Self {
        Self::boolean_field_path(root, vec![CanonicalStructuralPathSegment::Field(field)])
    }

    pub fn boolean_field_path(root: PlaceId, path: Vec<CanonicalStructuralPathSegment>) -> Self {
        Self::BooleanField { root, path }
    }

    pub fn integer_field_path(
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        scalar_type: IntegerType,
    ) -> Self {
        Self::IntegerField {
            root,
            path,
            scalar_type,
        }
    }

    pub fn ieee_float_field_path(
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        format: IeeeFloatFormat,
    ) -> Self {
        Self::IeeeFloatField { root, path, format }
    }

    pub fn view_extent(
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        scalar_type: IntegerType,
    ) -> Self {
        Self::ViewExtent {
            root,
            path,
            scalar_type,
        }
    }

    pub fn byte_sequence_predicate(
        predicate: ByteSequencePredicate,
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
    ) -> Self {
        Self::ByteSequencePredicate {
            predicate,
            root,
            path,
        }
    }

    pub const fn boolean(value: bool) -> Self {
        Self::Boolean(value)
    }

    pub fn boolean_not(operand: ScalarTerm) -> Result<Self, PropositionError> {
        if operand.scalar_type() != ScalarType::Boolean {
            return Err(PropositionError::BooleanNotTypeMismatch(
                operand.scalar_type(),
            ));
        }
        Ok(Self::BooleanNot {
            operand: Box::new(operand),
        })
    }

    pub fn boolean_equal(left: ScalarTerm, right: ScalarTerm) -> Result<Self, PropositionError> {
        if left.scalar_type() != ScalarType::Boolean || right.scalar_type() != ScalarType::Boolean {
            return Err(PropositionError::BooleanEqualTypeMismatch {
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(Self::BooleanEqual {
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    integer_binary_term!(@valid integer_equal => IntegerEqual, validate_integer_comparison_operands);

    integer_binary_term!(@valid integer_less_than => IntegerLessThan, validate_integer_comparison_operands);

    integer_binary_term!(@valid integer_less_or_equal => IntegerLessOrEqual, validate_integer_comparison_operands);

    pub fn mathematical_comparison(&self, positive: bool) -> Option<Proposition> {
        let (kind, left_type, left, right) = match self {
            Self::IntegerEqual {
                scalar_type,
                left,
                right,
            } => (0, *scalar_type, left, right),
            Self::IntegerLessThan {
                scalar_type,
                left,
                right,
            } => (1, *scalar_type, left, right),
            Self::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            } => (2, *scalar_type, left, right),
            _ => return None,
        };
        if left.scalar_type() == right.scalar_type()
            || !integer_comparison_operands_match(left_type, left, right)
        {
            return None;
        }
        let mathematical = |term: &Self| {
            if let Self::Value {
                id,
                scalar_type: ScalarType::Integer(source_type),
            } = term
            {
                return Some(IntegerMathTerm::MathValue {
                    source_type: *source_type,
                    value: *id,
                });
            }
            term.integer_value()
                .map(|(_, value)| IntegerMathTerm::literal(value))
        };
        let mut left = mathematical(left)?;
        let mut right = mathematical(right)?;
        Some(match (kind, positive) {
            (0, true) => {
                if left > right {
                    std::mem::swap(&mut left, &mut right);
                }
                Proposition::IntegerMathEqual(left, right)
            }
            (0, false) => Proposition::Disjunction(vec![
                Proposition::IntegerMathLessThan(left.clone(), right.clone()),
                Proposition::IntegerMathLessThan(right, left),
            ]),
            (1, true) => Proposition::IntegerMathLessThan(left, right),
            (1, false) => Proposition::IntegerMathLessOrEqual(right, left),
            (_, true) => Proposition::IntegerMathLessOrEqual(left, right),
            (_, false) => Proposition::IntegerMathLessThan(right, left),
        })
    }

    integer_binary_term!(integer_bitwise_and => IntegerBitwiseAnd);

    pub fn integer_bitwise_not(
        scalar_type: IntegerType,
        operand: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let expected = ScalarType::Integer(scalar_type);
        if operand.scalar_type() != expected {
            return Err(PropositionError::IntegerBitwiseNotTypeMismatch {
                expected,
                operand: operand.scalar_type(),
            });
        }
        Ok(Self::IntegerBitwiseNot {
            scalar_type,
            operand: Box::new(operand),
        })
    }

    pub fn integer_widen(
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let actual = operand.scalar_type();
        let expected = ScalarType::Integer(source_type);
        if actual != expected || !source_type.can_widen_to(target_type) {
            return Err(PropositionError::IntegerWidenTypeMismatch {
                source: expected,
                target: ScalarType::Integer(target_type),
                operand: actual,
            });
        }
        Ok(Self::IntegerWiden {
            source_type,
            target_type,
            operand: Box::new(operand),
        })
    }

    pub fn integer_exact_cast(
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let actual = operand.scalar_type();
        let expected = ScalarType::Integer(source_type);
        if actual != expected || !source_type.can_exact_cast_to(target_type) {
            return Err(PropositionError::IntegerExactCastTypeMismatch {
                source: expected,
                target: ScalarType::Integer(target_type),
                operand: actual,
            });
        }
        Ok(Self::IntegerExactCast {
            source_type,
            target_type,
            operand: Box::new(operand),
        })
    }

    integer_binary_term!(integer_bitwise_or => IntegerBitwiseOr);

    integer_binary_term!(integer_bitwise_xor => IntegerBitwiseXor);

    integer_shift_term!(wrapping_integer_shift_left => WrappingIntegerShiftLeft);

    integer_shift_term!(wrapping_integer_shift_right => WrappingIntegerShiftRight);

    integer_shift_term!(exact_integer_shift_right => ExactIntegerShiftRight);

    integer_shift_term!(exact_integer_shift_left => ExactIntegerShiftLeft);

    pub fn exact_integer_add(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_exact_integer_operands(
            scalar_type,
            &left,
            &right,
            IntegerType::admits_exact_add_operands,
        )?;
        Ok(Self::ExactIntegerAdd {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn exact_integer_subtract(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_exact_integer_operands(
            scalar_type,
            &left,
            &right,
            IntegerType::admits_exact_subtract_operands,
        )?;
        Ok(Self::ExactIntegerSubtract {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    integer_binary_term!(exact_integer_multiply => ExactIntegerMultiply);

    integer_binary_term!(exact_integer_divide => ExactIntegerDivide);

    integer_binary_term!(exact_integer_remainder => ExactIntegerRemainder);

    integer_binary_term!(wrapping_integer_divide => WrappingIntegerDivide);

    integer_binary_term!(wrapping_integer_remainder => WrappingIntegerRemainder);

    integer_binary_term!(saturating_integer_divide => SaturatingIntegerDivide);

    integer_binary_term!(saturating_integer_remainder => SaturatingIntegerRemainder);

    pub fn integer(
        scalar_type: IntegerType,
        value: IntegerValue,
    ) -> Result<Self, PropositionError> {
        if !scalar_type.admits(value) {
            return Err(PropositionError::IntegerLiteralOutsideType { scalar_type, value });
        }
        Ok(Self::Integer { scalar_type, value })
    }

    integer_binary_term!(wrapping_integer_add => WrappingIntegerAdd);

    integer_binary_term!(saturating_integer_add => SaturatingIntegerAdd);

    integer_binary_term!(wrapping_integer_subtract => WrappingIntegerSubtract);

    integer_binary_term!(saturating_integer_subtract => SaturatingIntegerSubtract);

    integer_binary_term!(wrapping_integer_multiply => WrappingIntegerMultiply);

    integer_binary_term!(saturating_integer_multiply => SaturatingIntegerMultiply);

    pub fn operands(&self) -> Vec<&ScalarTerm> {
        if let Self::StructuralRead { path, .. } = self {
            return path
                .iter()
                .filter_map(|segment| match segment {
                    ScalarReadProjection::Index(index) => Some(index.as_ref()),
                    ScalarReadProjection::Static(_)
                    | ScalarReadProjection::Referent
                    | ScalarReadProjection::Extent => None,
                })
                .collect();
        }
        scalar_term_operand_table!(self, as_ref)
    }

    pub fn operands_mut(&mut self) -> Vec<&mut ScalarTerm> {
        if let Self::StructuralRead { path, .. } = self {
            return path
                .iter_mut()
                .filter_map(|segment| match segment {
                    ScalarReadProjection::Index(index) => Some(index.as_mut()),
                    ScalarReadProjection::Static(_)
                    | ScalarReadProjection::Referent
                    | ScalarReadProjection::Extent => None,
                })
                .collect();
        }
        scalar_term_operand_table!(self, as_mut)
    }

    pub fn visit_value_ids(&self, mut visit: impl FnMut(ValueId) -> bool) -> bool {
        let mut pending = vec![self];
        while let Some(term) = pending.pop() {
            match term {
                Self::Value { id, .. } => {
                    if !visit(*id) {
                        return false;
                    }
                }
                _ => pending.extend(term.operands()),
            }
        }
        true
    }

    pub fn any_value_id(&self, mut predicate: impl FnMut(ValueId) -> bool) -> bool {
        !self.visit_value_ids(|id| !predicate(id))
    }
}

fn integer_comparison_operands_match(
    left_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> bool {
    let Some(right_type) = right.scalar_type().as_integer() else {
        return false;
    };
    left.scalar_type() == ScalarType::Integer(left_type)
        && (left_type == right_type || (!left_type.is_address() && !right_type.is_address()))
}

fn validate_integer_comparison_operands(
    left_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> Result<(), PropositionError> {
    if !integer_comparison_operands_match(left_type, left, right) {
        return Err(PropositionError::IntegerOperandTypeMismatch {
            expected: ScalarType::Integer(left_type),
            left: left.scalar_type(),
            right: right.scalar_type(),
        });
    }
    Ok(())
}

fn validate_exact_integer_operands(
    result: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
    admits: fn(IntegerType, IntegerType, IntegerType) -> bool,
) -> Result<(), PropositionError> {
    if !matches!((left.scalar_type(), right.scalar_type()),
        (ScalarType::Integer(left), ScalarType::Integer(right))
            if admits(result, left, right))
    {
        return Err(PropositionError::IntegerOperandTypeMismatch {
            expected: ScalarType::Integer(result),
            left: left.scalar_type(),
            right: right.scalar_type(),
        });
    }
    Ok(())
}

fn validate_integer_operands(
    integer_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> Result<(), PropositionError> {
    let expected = ScalarType::Integer(integer_type);
    if left.scalar_type() != expected || right.scalar_type() != expected {
        return Err(PropositionError::IntegerOperandTypeMismatch {
            expected,
            left: left.scalar_type(),
            right: right.scalar_type(),
        });
    }
    Ok(())
}

fn validate_integer_shift_operands(
    value_type: IntegerType,
    count_type: IntegerType,
    value: &ScalarTerm,
    count: &ScalarTerm,
) -> Result<(), PropositionError> {
    let expected_value = ScalarType::Integer(value_type);
    let expected_count = ScalarType::Integer(count_type);
    if value.scalar_type() != expected_value || count.scalar_type() != expected_count {
        return Err(PropositionError::IntegerShiftOperandTypeMismatch {
            expected_value,
            actual_value: value.scalar_type(),
            expected_count,
            actual_count: count.scalar_type(),
        });
    }
    Ok(())
}

enum Step<'a> {
    Visit(&'a ScalarTerm),
    Finalize(&'a ScalarTerm),
}

#[macro_export]
macro_rules! scalar_term_predicate_quint_pattern {
    ($($fields:tt)*) => {
        $crate::ScalarTerm::BooleanField { $($fields)* }
            | $crate::ScalarTerm::ByteSequencePredicate { $($fields)* }
            | $crate::ScalarTerm::IeeeFloatField { $($fields)* }
            | $crate::ScalarTerm::IntegerField { $($fields)* }
            | $crate::ScalarTerm::ViewExtent { $($fields)* }
    };
}
