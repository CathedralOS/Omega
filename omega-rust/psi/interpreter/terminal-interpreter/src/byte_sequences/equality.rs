//! Content equality resolves both operands through the shared operand
//! resolver — the same live extents the verifier bound — then compares the
//! byte windows the operands name. Equal live lengths and equal bytes at
//! every live position answer `true`.

use crate::errors::TerminalInterpretError;
use crate::execution::{OperationFlow, TerminalExecution};
use crate::values::TerminalScalarValue;
use semantic_vocabulary::ScalarType;
use terminal_psi::{Operation, OperationKind};

impl TerminalExecution {
    pub(crate) fn execute_byte_sequence_equal(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let OperationKind::ByteSequenceEqual { left, right } = &operation.kind else {
            return Err(invalid());
        };
        let result = operation.result.expect_scalar();
        if result.scalar_type != ScalarType::Boolean {
            return Err(invalid());
        }
        let left = self.byte_sequence_operand_view(left)?;
        let right = self.byte_sequence_operand_view(right)?;
        let verdict = left.bytes() == right.bytes();
        self.values
            .insert(result.id, TerminalScalarValue::Boolean(verdict));
        Ok(OperationFlow::Advance)
    }
}
