//! Boolean and IEEE-float operations of the interpreter loop.

use crate::errors::TerminalInterpretError;
use crate::execution::{OperationFlow, TerminalExecution};
use crate::scalar_operations::{ieee_float_compare, nearest_ieee_float_fused_multiply_add};
use crate::values::TerminalScalarValue;
use semantic_vocabulary::ScalarType;
use terminal_psi::OperationKind;

impl TerminalExecution {
    pub(crate) fn execute_ieee_float_constant(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::IeeeFloatConstant { value } = operation.kind else {
            unreachable!("dispatched execute_ieee_float_constant")
        };
        if operation.result.expect_scalar().scalar_type != ScalarType::IeeeFloat(value.format()) {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::IeeeFloat(value),
        );
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_ieee_float_compare(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::IeeeFloatCompare {
            comparison,
            left,
            right,
        } = operation.kind
        else {
            unreachable!("dispatched execute_ieee_float_compare")
        };
        let read_float = |operand| match self.values.get(&operand).copied() {
            Some(TerminalScalarValue::IeeeFloat(value)) => Ok(value),
            Some(_) => Err(TerminalInterpretError::VerifiedOperationMalformed),
            None => Err(TerminalInterpretError::VerifiedValueMissing(operand)),
        };
        let left = read_float(left)?;
        let right = read_float(right)?;
        if left.format() != right.format()
            || operation.result.expect_scalar().scalar_type != ScalarType::Boolean
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Boolean(ieee_float_compare(comparison, left, right)),
        );
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_nearest_ieee_float_fused_multiply_add(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::NearestIeeeFloatFusedMultiplyAdd {
            left,
            right,
            addend,
        } = operation.kind
        else {
            unreachable!("dispatched execute_nearest_ieee_float_fused_multiply_add")
        };
        let ScalarType::IeeeFloat(format) = operation.result.expect_scalar().scalar_type else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let TerminalScalarValue::IeeeFloat(left) = self
            .values
            .get(&left)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let TerminalScalarValue::IeeeFloat(right) = self
            .values
            .get(&right)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let TerminalScalarValue::IeeeFloat(addend) = self
            .values
            .get(&addend)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(addend))?
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        if left.format() != format || right.format() != format || addend.format() != format {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let result = nearest_ieee_float_fused_multiply_add(format, left, right, addend);
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::IeeeFloat(result),
        );
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_boolean_constant(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::BooleanConstant { value } = operation.kind else {
            unreachable!("dispatched execute_boolean_constant")
        };
        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Boolean(value),
        );
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_boolean_not(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::BooleanNot { operand } = operation.kind else {
            unreachable!("dispatched execute_boolean_not")
        };
        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let TerminalScalarValue::Boolean(value) = self
            .values
            .get(&operand)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Boolean(!value),
        );
        Ok(OperationFlow::Advance)
    }

    pub(crate) fn execute_boolean_equal(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::BooleanEqual { left, right } = operation.kind else {
            unreachable!("dispatched execute_boolean_equal")
        };
        if operation.result.expect_scalar().scalar_type != ScalarType::Boolean {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let TerminalScalarValue::Boolean(left) = self
            .values
            .get(&left)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(left))?
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let TerminalScalarValue::Boolean(right) = self
            .values
            .get(&right)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(right))?
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Boolean(left == right),
        );
        Ok(OperationFlow::Advance)
    }
}
