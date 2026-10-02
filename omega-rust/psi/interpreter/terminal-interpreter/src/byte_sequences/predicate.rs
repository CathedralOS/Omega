//! Predicate evaluation scans the operand's resolved bytes once. The operand
//! resolver already binds the live extent, so `holds_for` sees exactly the
//! byte window the verifier validated.

use crate::errors::TerminalInterpretError;
use crate::execution::{OperationFlow, TerminalExecution};
use crate::values::TerminalScalarValue;
use semantic_vocabulary::ScalarType;
use terminal_psi::{Operation, OperationKind};

impl TerminalExecution {
    pub(crate) fn execute_byte_sequence_predicate(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let OperationKind::ByteSequencePredicate { operand, predicate } = &operation.kind else {
            return Err(invalid());
        };
        let result = operation.result.expect_scalar();
        if result.scalar_type != ScalarType::Boolean {
            return Err(invalid());
        }
        let bytes = self.byte_sequence_operand_view(operand)?;
        let verdict = predicate.holds_for(bytes.bytes());
        self.values
            .insert(result.id, TerminalScalarValue::Boolean(verdict));
        Ok(OperationFlow::Advance)
    }
}
