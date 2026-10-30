//! Predicate evaluation scans the operand's resolved bytes once. The operand
//! resolver already binds the live extent, so `holds_for` sees exactly the
//! byte window the verifier validated.

use crate::errors::TerminalInterpretError;
use crate::execution::{OperationFlow, TerminalExecution, operation_kind};
use crate::values::TerminalScalarValue;
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
