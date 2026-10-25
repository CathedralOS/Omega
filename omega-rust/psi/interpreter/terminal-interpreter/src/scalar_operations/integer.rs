//! Integer operations of the interpreter loop: constants, comparisons,
//! bitwise and width conversions, and the exact, wrapping and saturating
//! arithmetic families.

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{OperationFlow, TerminalExecution};
use crate::values::TerminalScalarValue;
use semantic_vocabulary::ScalarType;
use terminal_psi::{Operation, OperationKind};

/// The integer scalar type the verified result declares, or malformed when it
/// is not an integer.
pub(crate) fn integer_scalar_type(
    operation: &Operation,
) -> Result<semantic_vocabulary::IntegerType, TerminalInterpretError> {
    let Some(scalar_type) = operation.result.expect_scalar().scalar_type.as_integer() else {
        return Err(malformed());
    };
    Ok(scalar_type)
}

macro_rules! saturating_integer_op {
    ($( $name:ident : $variant:ident => $method:ident ; )+) => {$(
        pub(crate) fn $name(
            &mut self,
            operation: &terminal_psi::Operation,
        ) -> Result<OperationFlow, TerminalInterpretError> {
            let OperationKind::$variant { left, right } = operation.kind else {
                unreachable!("dispatched {}", stringify!($name))
            };
            let (scalar_type, left, right, left_type, right_type) =
                self.integer_operands(operation, left, right)?;
            if left_type != scalar_type || right_type != scalar_type {
                return Err(malformed());
            }
            let value = scalar_type.$method(left, right).ok_or(malformed())?;
            self.store_scalar(
                operation,
                TerminalScalarValue::Integer { scalar_type, value },
            )
        }
    )+}
}

impl TerminalExecution<'_> {
    /// One operand's declared integer type and value, or malformed when the
    /// bound scalar is not an integer.
    pub(crate) fn integer_operand(
        &self,
        operand: semantic_vocabulary::ValueId,
    ) -> Result<
        (
            semantic_vocabulary::IntegerType,
            semantic_vocabulary::IntegerValue,
        ),
        TerminalInterpretError,
    > {
        match self.scalar_value(operand)? {
            TerminalScalarValue::Integer { scalar_type, value } => Ok((scalar_type, value)),
            _ => Err(malformed()),
        }
    }

    /// Both operands of a binary integer operation: the declared integer
    /// carrier plus each operand's integer type and value, resolved and
    /// unwrapped. Callers own the operand-to-carrier conformance check, which
    /// admits a subtraction's widened operands at the exact-arithmetic sites.
    fn integer_operands(
        &self,
        operation: &terminal_psi::Operation,
        left: semantic_vocabulary::ValueId,
        right: semantic_vocabulary::ValueId,
    ) -> Result<
        (
            semantic_vocabulary::IntegerType,
            semantic_vocabulary::IntegerValue,
            semantic_vocabulary::IntegerValue,
            semantic_vocabulary::IntegerType,
            semantic_vocabulary::IntegerType,
        ),
        TerminalInterpretError,
    > {
        let scalar_type = integer_scalar_type(operation)?;
        let (left_type, left) = self.integer_operand(left)?;
        let (right_type, right) = self.integer_operand(right)?;
        Ok((scalar_type, left, right, left_type, right_type))
    }

    pub(crate) fn execute_integer_constant(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::IntegerConstant { value } = operation.kind else {
            unreachable!("dispatched execute_integer_constant")
        };
        let scalar_type = integer_scalar_type(operation)?;
        self.store_scalar(
            operation,
            TerminalScalarValue::Integer { scalar_type, value },
        )
    }

    pub(crate) fn execute_integer_equal(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::IntegerEqual { left, right } = operation.kind else {
            unreachable!("dispatched execute_integer_equal")
        };
        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
            return Err(malformed());
        }
        let (left_type, left) = self.integer_operand(left)?;
        let (right_type, right) = self.integer_operand(right)?;
        let ordering = left_type
            .compare_with(left, right_type, right)
            .ok_or(malformed())?;
        self.store_scalar(operation, TerminalScalarValue::Boolean(ordering.is_eq()))
    }

    pub(crate) fn execute_integer_less_than(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let (terminal_psi::integer_less_or_equal_pair_pattern!(left, right, ..)) = operation.kind
        else {
            unreachable!("dispatched execute_integer_less_than")
        };
        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
            return Err(malformed());
        }
        let (left_type, left_value) = self.integer_operand(left)?;
        let (right_type, right_value) = self.integer_operand(right)?;
        let ordering = left_type
            .compare_with(left_value, right_type, right_value)
            .ok_or(malformed())?;
        let result = match operation.kind {
            OperationKind::IntegerLessThan { .. } => ordering.is_lt(),
            OperationKind::IntegerLessOrEqual { .. } => !ordering.is_gt(),
            _ => unreachable!(),
        };
        self.store_scalar(operation, TerminalScalarValue::Boolean(result))
    }

    pub(crate) fn execute_integer_bitwise_not(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::IntegerBitwiseNot { operand } = operation.kind else {
            unreachable!("dispatched execute_integer_bitwise_not")
        };
        let scalar_type = integer_scalar_type(operation)?;
        let (operand_type, operand) = self.integer_operand(operand)?;
        if operand_type != scalar_type {
            return Err(malformed());
        }
        let value = scalar_type.bitwise_not(operand).ok_or(malformed())?;
        self.store_scalar(
            operation,
            TerminalScalarValue::Integer { scalar_type, value },
        )
    }

    fn execute_integer_unary_cast(
        &mut self,
        operation: &terminal_psi::Operation,
        operand: semantic_vocabulary::ValueId,
        cast: fn(
            semantic_vocabulary::IntegerType,
            semantic_vocabulary::IntegerType,
            semantic_vocabulary::IntegerValue,
        ) -> Option<semantic_vocabulary::IntegerValue>,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let target_type = integer_scalar_type(operation)?;
        let TerminalScalarValue::Integer {
            scalar_type: source_type,
            value,
        } = self.scalar_value(operand)?
        else {
            return Err(malformed());
        };
        let value = cast(source_type, target_type, value).ok_or(malformed())?;
        self.store_scalar(
            operation,
            TerminalScalarValue::Integer {
                scalar_type: target_type,
                value,
            },
        )
    }

    pub(crate) fn execute_integer_widen(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::IntegerWiden { operand } = operation.kind else {
            unreachable!("dispatched execute_integer_widen")
        };
        self.execute_integer_unary_cast(
            operation,
            operand,
            semantic_vocabulary::IntegerType::widen_value_to,
        )
    }

    pub(crate) fn execute_integer_exact_cast(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::IntegerExactCast { operand, .. } = operation.kind else {
            unreachable!("dispatched execute_integer_exact_cast")
        };
        self.execute_integer_unary_cast(
            operation,
            operand,
            semantic_vocabulary::IntegerType::exact_cast_value_to,
        )
    }

    pub(crate) fn execute_integer_bitwise_and(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let (OperationKind::IntegerBitwiseAnd { left, right }
        | OperationKind::IntegerBitwiseOr { left, right }
        | OperationKind::IntegerBitwiseXor { left, right }) = operation.kind
        else {
            unreachable!("dispatched execute_integer_bitwise_and")
        };
        let scalar_type = integer_scalar_type(operation)?;
        let (left_type, left_value) = self.integer_operand(left)?;
        let (right_type, right_value) = self.integer_operand(right)?;
        if left_type != scalar_type || right_type != scalar_type {
            return Err(malformed());
        }
        let value = match operation.kind {
            OperationKind::IntegerBitwiseAnd { .. } => {
                scalar_type.bitwise_and(left_value, right_value)
            }
            OperationKind::IntegerBitwiseOr { .. } => {
                scalar_type.bitwise_or(left_value, right_value)
            }
            OperationKind::IntegerBitwiseXor { .. } => {
                scalar_type.bitwise_xor(left_value, right_value)
            }
            _ => unreachable!(),
        }
        .ok_or(malformed())?;
        self.store_scalar(
            operation,
            TerminalScalarValue::Integer { scalar_type, value },
        )
    }

    pub(crate) fn execute_wrapping_integer_shift_left(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let (OperationKind::WrappingIntegerShiftLeft { value, count }
        | OperationKind::WrappingIntegerShiftRight { value, count }
        | OperationKind::ExactIntegerShiftLeft { value, count, .. }
        | OperationKind::ExactIntegerShiftRight { value, count, .. }) = operation.kind
        else {
            unreachable!("dispatched execute_wrapping_integer_shift_left")
        };
        let value_type = integer_scalar_type(operation)?;
        let TerminalScalarValue::Integer {
            scalar_type: actual_value_type,
            value,
        } = self.scalar_value(value)?
        else {
            return Err(malformed());
        };
        let (count_type, count) = self.integer_operand(count)?;
        if actual_value_type != value_type {
            return Err(malformed());
        }
        let value = match operation.kind {
            OperationKind::WrappingIntegerShiftLeft { .. } => {
                value_type.wrapping_shift_left(value, count_type, count)
            }
            OperationKind::WrappingIntegerShiftRight { .. } => {
                value_type.wrapping_shift_right(value, count_type, count)
            }
            OperationKind::ExactIntegerShiftLeft { .. } => {
                value_type.exact_shift_left(value, count_type, count)
            }
            OperationKind::ExactIntegerShiftRight { .. } => {
                value_type.exact_shift_right(value, count_type, count)
            }
            _ => unreachable!(),
        }
        .ok_or(malformed())?;
        self.store_scalar(
            operation,
            TerminalScalarValue::Integer {
                scalar_type: value_type,
                value,
            },
        )
    }

    pub(crate) fn execute_exact_integer_add(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let (OperationKind::ExactIntegerAdd { left, right, .. }
        | OperationKind::WrappingIntegerAdd { left, right }
        | OperationKind::ExactIntegerSubtract { left, right, .. }
        | OperationKind::WrappingIntegerSubtract { left, right }
        | OperationKind::ExactIntegerMultiply { left, right, .. }
        | OperationKind::ExactIntegerDivide { left, right, .. }
        | OperationKind::ExactIntegerRemainder { left, right, .. }
        | OperationKind::WrappingIntegerDivide { left, right, .. }
        | OperationKind::WrappingIntegerRemainder { left, right, .. }
        | OperationKind::SaturatingIntegerDivide { left, right, .. }
        | OperationKind::SaturatingIntegerRemainder { left, right, .. }
        | OperationKind::WrappingIntegerMultiply { left, right }) = operation.kind
        else {
            unreachable!("dispatched execute_exact_integer_add")
        };
        let (scalar_type, left, right, left_type, right_type) =
            self.integer_operands(operation, left, right)?;
        let operands_match = if matches!(operation.kind, OperationKind::ExactIntegerAdd { .. }) {
            scalar_type.admits_exact_add_operands(left_type, right_type)
        } else if matches!(operation.kind, OperationKind::ExactIntegerSubtract { .. }) {
            scalar_type.admits_exact_subtract_operands(left_type, right_type)
        } else {
            left_type == scalar_type && right_type == scalar_type
        };
        if !operands_match {
            return Err(malformed());
        }
        let value = match operation.kind {
            OperationKind::ExactIntegerAdd { .. } => {
                scalar_type.exact_add_values(left_type, left, right_type, right)
            }
            OperationKind::WrappingIntegerAdd { .. } => scalar_type.wrapping_add(left, right),
            OperationKind::ExactIntegerSubtract { .. } => {
                scalar_type.exact_subtract_values(left_type, left, right_type, right)
            }
            OperationKind::WrappingIntegerSubtract { .. } => scalar_type.wrapping_sub(left, right),
            OperationKind::ExactIntegerMultiply { .. } => scalar_type.exact_mul(left, right),
            OperationKind::ExactIntegerDivide { .. } => scalar_type.exact_div(left, right),
            OperationKind::ExactIntegerRemainder { .. } => scalar_type.exact_rem(left, right),
            OperationKind::WrappingIntegerDivide { .. } => scalar_type.wrapping_div(left, right),
            OperationKind::WrappingIntegerRemainder { .. } => scalar_type.wrapping_rem(left, right),
            OperationKind::SaturatingIntegerDivide { .. } => {
                scalar_type.saturating_div(left, right)
            }
            OperationKind::SaturatingIntegerRemainder { .. } => {
                scalar_type.saturating_rem(left, right)
            }
            OperationKind::WrappingIntegerMultiply { .. } => scalar_type.wrapping_mul(left, right),
            _ => unreachable!(),
        }
        .ok_or(malformed())?;
        self.store_scalar(
            operation,
            TerminalScalarValue::Integer { scalar_type, value },
        )
    }

    saturating_integer_op! {
        execute_saturating_integer_add : SaturatingIntegerAdd => saturating_add ;
        execute_saturating_integer_subtract : SaturatingIntegerSubtract => saturating_sub ;
        execute_saturating_integer_multiply : SaturatingIntegerMultiply => saturating_mul ;
    }

    /// Store a scalar result and advance: every scalar execution epilogue.
    pub(crate) fn store_scalar(
        &mut self,
        operation: &terminal_psi::Operation,
        value: TerminalScalarValue,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        self.values
            .insert(operation.result.expect_scalar().id, value)?;
        Ok(OperationFlow::Advance)
    }
}
