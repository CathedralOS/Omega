//! Predicate evaluation scans the operand's resolved bytes once. The operand
//! resolver already binds the live extent, so `holds_for` sees exactly the
//! byte window the verifier validated.

use crate::boundary::errors::TerminalInterpretError;
use crate::boundary::values::TerminalScalarValue;
use crate::engine::execution::{OperationFlow, TerminalExecution, operation_kind};
use terminal_psi::{Operation, OperationKind};

impl TerminalExecution<'_> {
    pub(crate) fn execute_byte_sequence_predicate(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        operation_kind!(operation => ByteSequencePredicate { operand, predicate });
        self.require_boolean_result(operation)?;
        let bytes = self.byte_sequence_operand_view(operand)?;
        let verdict = predicate.holds_for(bytes.bytes());
        self.store_scalar(operation, TerminalScalarValue::Boolean(verdict))
    }
}
