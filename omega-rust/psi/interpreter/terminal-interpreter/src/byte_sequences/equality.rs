//! Content equality resolves both operands through the shared operand
//! resolver — the same live extents the verifier bound — then compares the
//! byte windows the operands name. Equal live lengths and equal bytes at
//! every live position answer `true`.

use crate::errors::TerminalInterpretError;
use crate::execution::{OperationFlow, TerminalExecution, operation_kind};
use crate::values::TerminalScalarValue;
use terminal_psi::{Operation, OperationKind};

impl TerminalExecution<'_> {
    pub(crate) fn execute_byte_sequence_equal(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        operation_kind!(operation => ByteSequenceEqual { left, right });
        self.require_boolean_result(operation)?;
        let left = self.byte_sequence_operand_view(left)?;
        let right = self.byte_sequence_operand_view(right)?;
        let verdict = left.bytes() == right.bytes();
        self.store_scalar(operation, TerminalScalarValue::Boolean(verdict))
    }
}
