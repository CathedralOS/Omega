//! Content equality resolves both operands through the shared operand
//! resolver — the same live extents the verifier bound — then compares the
//! byte windows the operands name. Equal live lengths and equal bytes at
//! every live position answer `true`.

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{OperationFlow, TerminalExecution};
use crate::values::TerminalScalarValue;
use terminal_psi::{Operation, OperationKind};

impl TerminalExecution<'_> {
    pub(crate) fn execute_byte_sequence_equal(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::ByteSequenceEqual { left, right } = &operation.kind else {
            return Err(malformed());
        };
        self.require_boolean_result(operation)?;
        let left = self.byte_sequence_operand_view(left)?;
        let right = self.byte_sequence_operand_view(right)?;
        let verdict = left.bytes() == right.bytes();
        self.store_scalar(operation, TerminalScalarValue::Boolean(verdict))
    }
}
