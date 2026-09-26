//! Integer operations of the interpreter loop: constants, comparisons,
//! bitwise and width conversions, and the exact, wrapping and saturating
//! arithmetic families.

use crate::errors::TerminalInterpretError;
use crate::execution::{OperationFlow, TerminalExecution};
use crate::values::TerminalScalarValue;
use semantic_vocabulary::ScalarType;
use terminal_psi::OperationKind;

impl TerminalExecution {
    pub(crate) fn execute_integer_constant(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::IntegerConstant { value } = operation.kind else {
            unreachable!("dispatched execute_integer_constant")
        };
        let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Integer { scalar_type, value },
        );
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_integer_equal(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::IntegerEqual { left, right } = operation.kind else {
            unreachable!("dispatched execute_integer_equal")
        };
        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let TerminalScalarValue::Integer {
            scalar_type: left_type,
            value: left,
        } = self
            .values
            .get(&left)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let TerminalScalarValue::Integer {
            scalar_type: right_type,
            value: right,
        } = self
            .values
            .get(&right)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        if left_type != right_type {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Boolean(left == right),
        );
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_integer_less_than(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let (OperationKind::IntegerLessThan { left, right }
        | OperationKind::IntegerLessOrEqual { left, right }) = operation.kind
        else {
            unreachable!("dispatched execute_integer_less_than")
        };
        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let TerminalScalarValue::Integer {
            scalar_type: left_type,
            value: left_value,
        } = self
            .values
            .get(&left)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let TerminalScalarValue::Integer {
            scalar_type: right_type,
            value: right_value,
        } = self
            .values
            .get(&right)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        if left_type != right_type {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let ordering = left_type
            .compare(left_value, right_value)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let result = match operation.kind {
            OperationKind::IntegerLessThan { .. } => ordering.is_lt(),
            OperationKind::IntegerLessOrEqual { .. } => !ordering.is_gt(),
            _ => unreachable!(),
        };
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Boolean(result),
        );
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_integer_bitwise_not(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::IntegerBitwiseNot { operand } = operation.kind else {
            unreachable!("dispatched execute_integer_bitwise_not")
        };
        let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let TerminalScalarValue::Integer {
            scalar_type: operand_type,
            value: operand,
        } = self
            .values
            .get(&operand)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        if operand_type != scalar_type {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let value = scalar_type
            .bitwise_not(operand)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Integer { scalar_type, value },
        );
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_integer_widen(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::IntegerWiden { operand } = operation.kind else {
            unreachable!("dispatched execute_integer_widen")
        };
        let ScalarType::Integer(target_type) = operation.result.expect_scalar().scalar_type else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let TerminalScalarValue::Integer {
            scalar_type: source_type,
            value,
        } = self
            .values
            .get(&operand)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let value = source_type
            .widen_value_to(target_type, value)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Integer {
                scalar_type: target_type,
                value,
            },
        );
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_integer_exact_cast(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::IntegerExactCast { operand, .. } = operation.kind else {
            unreachable!("dispatched execute_integer_exact_cast")
        };
        let ScalarType::Integer(target_type) = operation.result.expect_scalar().scalar_type else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let TerminalScalarValue::Integer {
            scalar_type: source_type,
            value,
        } = self
            .values
            .get(&operand)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let value = source_type
            .exact_cast_value_to(target_type, value)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Integer {
                scalar_type: target_type,
                value,
            },
        );
        Ok(OperationFlow::Advance)
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
        let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let TerminalScalarValue::Integer {
            scalar_type: left_type,
            value: left_value,
        } = self
            .values
            .get(&left)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let TerminalScalarValue::Integer {
            scalar_type: right_type,
            value: right_value,
        } = self
            .values
            .get(&right)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        if left_type != scalar_type || right_type != scalar_type {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
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
        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Integer { scalar_type, value },
        );
        Ok(OperationFlow::Advance)
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
        let ScalarType::Integer(value_type) = operation.result.expect_scalar().scalar_type else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let TerminalScalarValue::Integer {
            scalar_type: actual_value_type,
            value,
        } = self
            .values
            .get(&value)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(value))?
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let TerminalScalarValue::Integer {
            scalar_type: count_type,
            value: count,
        } = self
            .values
            .get(&count)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(count))?
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        if actual_value_type != value_type {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
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
        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Integer {
                scalar_type: value_type,
                value,
            },
        );
        Ok(OperationFlow::Advance)
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
        let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let left = self
            .values
            .get(&left)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
        let right = self
            .values
            .get(&right)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
        let (
            TerminalScalarValue::Integer {
                scalar_type: left_type,
                value: left,
            },
            TerminalScalarValue::Integer {
                scalar_type: right_type,
                value: right,
            },
        ) = (left, right)
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        if left_type != scalar_type || right_type != scalar_type {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let value = match operation.kind {
            OperationKind::ExactIntegerAdd { .. } => scalar_type.exact_add(left, right),
            OperationKind::WrappingIntegerAdd { .. } => scalar_type.wrapping_add(left, right),
            OperationKind::ExactIntegerSubtract { .. } => scalar_type.exact_sub(left, right),
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
        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Integer { scalar_type, value },
        );
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_saturating_integer_add(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::SaturatingIntegerAdd { left, right } = operation.kind else {
            unreachable!("dispatched execute_saturating_integer_add")
        };
        let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let left = self
            .values
            .get(&left)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
        let right = self
            .values
            .get(&right)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
        let (
            TerminalScalarValue::Integer {
                scalar_type: left_type,
                value: left,
            },
            TerminalScalarValue::Integer {
                scalar_type: right_type,
                value: right,
            },
        ) = (left, right)
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        if left_type != scalar_type || right_type != scalar_type {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let value = scalar_type
            .saturating_add(left, right)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Integer { scalar_type, value },
        );
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_saturating_integer_subtract(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::SaturatingIntegerSubtract { left, right } = operation.kind else {
            unreachable!("dispatched execute_saturating_integer_subtract")
        };
        let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let left = self
            .values
            .get(&left)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
        let right = self
            .values
            .get(&right)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
        let (
            TerminalScalarValue::Integer {
                scalar_type: left_type,
                value: left,
            },
            TerminalScalarValue::Integer {
                scalar_type: right_type,
                value: right,
            },
        ) = (left, right)
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        if left_type != scalar_type || right_type != scalar_type {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let value = scalar_type
            .saturating_sub(left, right)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Integer { scalar_type, value },
        );
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_saturating_integer_multiply(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::SaturatingIntegerMultiply { left, right } = operation.kind else {
            unreachable!("dispatched execute_saturating_integer_multiply")
        };
        let ScalarType::Integer(scalar_type) = operation.result.expect_scalar().scalar_type else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let left = self
            .values
            .get(&left)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?;
        let right = self
            .values
            .get(&right)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?;
        let (
            TerminalScalarValue::Integer {
                scalar_type: left_type,
                value: left,
            },
            TerminalScalarValue::Integer {
                scalar_type: right_type,
                value: right,
            },
        ) = (left, right)
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        if left_type != scalar_type || right_type != scalar_type {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let value = scalar_type
            .saturating_mul(left, right)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Integer { scalar_type, value },
        );
        Ok(OperationFlow::Advance)
    }
}
