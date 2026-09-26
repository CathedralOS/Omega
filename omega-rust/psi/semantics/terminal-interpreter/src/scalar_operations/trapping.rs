//! Trapping integer primitives: the exact result on a normal return, or a
//! `Trap` crash at the operation itself.
//!
//! The trap predicate is the one the settled integer-policy catalog names
//! for the Trapping policy (result outside the carrier, zero divisor, signed
//! `MIN / -1`, shift count outside `0..width`, unrepresentable conversion).
//! Each is exactly the case where the carrier's `exact_*` evaluation has no
//! value, so the interpreter evaluates that one function and never a wrapped
//! or saturated substitute. The site is the operation's own coordinate; like
//! a boundary-call crash, the reported frontier is the machine-local live
//! claim set at the operation.

use crate::errors::TerminalInterpretError;
use crate::execution::{OperationFlow, TerminalExecution};
use crate::results::{TerminalCrash, TerminalCrashSite, TerminalExecutionStatus};
use crate::values::TerminalScalarValue;
use semantic_vocabulary::{IntegerType, IntegerValue, ScalarType, ValueId};
use terminal_psi::{CrashCause, OperationKind, TrappingIntegerOperation};

impl TerminalExecution {
    fn trapping_integer_operand(
        &self,
        value: ValueId,
    ) -> Result<(IntegerType, IntegerValue), TerminalInterpretError> {
        match self
            .values
            .get(&value)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(value))?
        {
            TerminalScalarValue::Integer { scalar_type, value } => Ok((scalar_type, value)),
            _ => Err(TerminalInterpretError::VerifiedOperationMalformed),
        }
    }

    pub(crate) fn execute_trapping_integer(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::TrappingInteger {
            operation: trapping,
        } = operation.kind
        else {
            unreachable!("dispatched execute_trapping_integer")
        };
        let ScalarType::Integer(result_type) = operation.result.expect_scalar().scalar_type else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let binary = |execution: &Self, left: ValueId, right: ValueId| {
            let (left_type, left) = execution.trapping_integer_operand(left)?;
            let (right_type, right) = execution.trapping_integer_operand(right)?;
            if left_type != result_type || right_type != result_type {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            Ok((left, right))
        };
        let exact = match trapping {
            TrappingIntegerOperation::Add { left, right } => {
                let (left, right) = binary(self, left, right)?;
                result_type.exact_add(left, right)
            }
            TrappingIntegerOperation::Subtract { left, right } => {
                let (left, right) = binary(self, left, right)?;
                result_type.exact_sub(left, right)
            }
            TrappingIntegerOperation::Multiply { left, right } => {
                let (left, right) = binary(self, left, right)?;
                result_type.exact_mul(left, right)
            }
            TrappingIntegerOperation::Divide { left, right } => {
                let (left, right) = binary(self, left, right)?;
                result_type.exact_div(left, right)
            }
            TrappingIntegerOperation::Remainder { left, right } => {
                let (left, right) = binary(self, left, right)?;
                result_type.exact_rem(left, right)
            }
            TrappingIntegerOperation::ShiftLeft { value, count }
            | TrappingIntegerOperation::ShiftRight { value, count } => {
                let (value_type, value) = self.trapping_integer_operand(value)?;
                let (count_type, count) = self.trapping_integer_operand(count)?;
                if value_type != result_type {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                }
                if matches!(trapping, TrappingIntegerOperation::ShiftLeft { .. }) {
                    result_type.exact_shift_left(value, count_type, count)
                } else {
                    result_type.exact_shift_right(value, count_type, count)
                }
            }
            TrappingIntegerOperation::Convert { operand } => {
                let (source_type, value) = self.trapping_integer_operand(operand)?;
                source_type.exact_cast_value_to(result_type, value)
            }
        };
        let Some(value) = exact else {
            // No exact value exists: this is the primitive's trap, committed
            // before any result is published.
            let crash = TerminalCrash {
                site: TerminalCrashSite::Operation {
                    machine: self.current_machine,
                    block: self.current,
                    operation: operation.id,
                },
                cause: CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: self.live_claims.keys().copied().collect(),
            };
            self.crash = Some(crash.clone());
            return Ok(OperationFlow::Yield(TerminalExecutionStatus::Crashed(
                crash,
            )));
        };
        self.values.insert(
            operation.result.expect_scalar().id,
            TerminalScalarValue::Integer {
                scalar_type: result_type,
                value,
            },
        );
        Ok(OperationFlow::Advance)
    }
}
