//! Value queries: the typed projections callers read off constructed
//! terms, extracted from `scalar_terms` so construction and traversal
//! stay the file's headline.

use super::{ScalarTerm, ScalarType, Step};
use crate::proposition::{IntegerType, IntegerValue};

impl ScalarTerm {
    pub fn scalar_type(&self) -> ScalarType {
        match self {
            Self::LayoutObservation { .. } => ScalarType::Integer(
                IntegerType::new(crate::IntegerSign::Unsigned, 64)
                    .expect("u64 is a fixed scalar carrier"),
            ),
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
            Self::IntegerWiden { target_type, .. } | Self::IntegerExactCast { target_type, .. } => {
                ScalarType::Integer(*target_type)
            }
            Self::WrappingIntegerShiftLeft { value_type, .. }
            | Self::WrappingIntegerShiftRight { value_type, .. }
            | Self::ExactIntegerShiftLeft { value_type, .. }
            | Self::ExactIntegerShiftRight { value_type, .. } => ScalarType::Integer(*value_type),
        }
    }

    /// Constant-folds an integer-producing term, `None` when any operand or
    /// typing rule fails. Runs on the same `Visit`/`Finalize` worklist shape
    /// as `validate`: a `Visit` pushes the node's `Finalize` behind its
    /// operands, so each node's own rule runs only after its operands produced
    /// values — the post-order position the recursive evaluator used. Operand
    /// results live on a second stack where every visited term contributes
    /// exactly one slot; non-integer forms finalize to `None` without visiting
    /// their children, matching the old `_ => None` arm.
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

    /// The node's own constant-folding rule, run once its operands' values sit
    /// atop `operands`: unary forms pop one slot, binary forms two (right
    /// above left), leaves and non-integer forms none. A `None` slot fails the
    /// node exactly like the recursive evaluator's `?` on that child's result.
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
                if operand_type != *source_type || !source_type.can_exact_cast_to(*target_type) {
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
                    Self::SaturatingIntegerAdd { .. } => scalar_type.saturating_add(left, right)?,
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
                // `count` finalized above `value`; both slots leave the stack
                // even when one is `None`, keeping the one-slot-per-term rule.
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
                    Self::WrappingIntegerDivide { .. } => scalar_type.wrapping_div(left, right)?,
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

    /// Pops the operand slot a `Finalize` step finds on top of the stack; a
    /// `None` slot fails the caller the way the recursive `?` did.
    fn pop_integer_operand(
        operands: &mut Vec<Option<(IntegerType, IntegerValue)>>,
    ) -> Option<(IntegerType, IntegerValue)> {
        operands.pop().flatten()
    }

    /// Pops a binary node's operand slots — `right` finalized above `left` —
    /// and enforces the shared operand-type agreement every binary integer
    /// form checks against its declared `scalar_type`. Both slots leave the
    /// stack even when one is `None`, so each visited term still contributes
    /// exactly one slot to its parent's `Finalize`.
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

    /// Constant-folds a boolean-producing term on the same `Visit`/`Finalize`
    /// worklist as `integer_value`. Only `BooleanNot` and `BooleanEqual` carry
    /// boolean operands, so just their children join the traversal; integer
    /// comparisons hold no boolean children and finalize by delegating to the
    /// iterative `integer_value`, keeping deep integer subtrees off the call
    /// stack too. Non-boolean forms finalize to `None` like the old
    /// `_ => None` arm.
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

    /// The node's own boolean rule once its operands' values sit atop
    /// `operands`: `BooleanNot` pops one slot, `BooleanEqual` two (right above
    /// left), every other form none. The integer comparisons evaluate their
    /// children through `integer_value` here rather than joining the boolean
    /// traversal, since their operand slots live on a different stack.
    fn boolean_value_after_operands(&self, operands: &mut Vec<Option<bool>>) -> Option<bool> {
        match self {
            Self::Boolean(value) => Some(*value),
            Self::BooleanNot { .. } => Some(!operands.pop().flatten()?),
            Self::BooleanEqual { .. } => {
                // Both slots leave the stack even when one is `None`, keeping
                // the one-slot-per-term rule for the parent's `Finalize`.
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
