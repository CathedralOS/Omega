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

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{OperationFlow, TerminalExecution, dispatched_operation};
use crate::results::{TerminalCrash, TerminalCrashSite, TerminalExecutionStatus};
use crate::values::TerminalScalarValue;
use semantic_vocabulary::ValueId;
use terminal_psi::{CrashCause, OperationKind, TrappingIntegerOperation};

impl TerminalExecution<'_> {
    /// Commit a `Trap` crash at `operation` in the current block: the
    /// reported frontier is the machine-local live claim set now.
    pub(crate) fn operation_trap(&mut self, operation: &terminal_psi::Operation) -> TerminalCrash {
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
        crash
    }

    pub(crate) fn execute_trapping_integer(
        &mut self,
        operation: &terminal_psi::Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        dispatched_operation!(operation => TrappingInteger { operation: trapping });
        let result_type = crate::scalar_operations::integer_scalar_type(operation)?;
        let binary = |execution: &Self, left: ValueId, right: ValueId| {
            let (left_type, left) = execution.integer_operand(left)?;
            let (right_type, right) = execution.integer_operand(right)?;
            if left_type != result_type || right_type != result_type {
                return Err(malformed());
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
                let (value_type, value) = self.integer_operand(value)?;
                let (count_type, count) = self.integer_operand(count)?;
                if value_type != result_type {
                    return Err(malformed());
                }
                if matches!(trapping, TrappingIntegerOperation::ShiftLeft { .. }) {
                    result_type.exact_shift_left(value, count_type, count)
                } else {
                    result_type.exact_shift_right(value, count_type, count)
                }
            }
            TrappingIntegerOperation::Convert { operand } => {
                let (source_type, value) = self.integer_operand(operand)?;
                source_type.exact_cast_value_to(result_type, value)
            }
        };
        let Some(value) = exact else {
            // No exact value exists: this is the primitive's trap, committed
            // before any result is published.
            return Ok(OperationFlow::Yield(TerminalExecutionStatus::Crashed(
                self.operation_trap(operation),
            )));
        };
        self.store_scalar(
            operation,
            TerminalScalarValue::Integer {
                scalar_type: result_type,
                value,
            },
        )
    }
}
