//! Shared operand resolution for byte-sequence operations: one
//! `ByteSequenceOperand` names either a whole view or one structural field's
//! view, and every consumer reads the same live extent bound.

use crate::byte_sequences::ByteSequenceView;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::TerminalScalarValue;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_psi::control_flow::ByteSequenceOperand;

impl TerminalExecution<'_> {
    pub(crate) fn byte_sequence_operand_view(
        &self,
        operand: &ByteSequenceOperand,
    ) -> Result<ByteSequenceView, TerminalInterpretError> {
        let bytes = match operand {
            ByteSequenceOperand::Whole { source, .. } => self
                .byte_sequence_values
                .get(source)
                .ok_or_else(malformed)?
                .immutable()?
                .clone(),
            ByteSequenceOperand::Field { source, field, .. } => {
                let (key, capacity) =
                    self.readable_byte_field(source.place, &source.path, *field)?;
                let bytes = self
                    .structural_byte_sequence_fields
                    .get(&key)
                    .ok_or_else(malformed)?;
                if capacity.is_some_and(|capacity| bytes.len() as u128 > u128::from(capacity)) {
                    return Err(malformed());
                }
                bytes.clone()
            }
        };
        let expected = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
        if !matches!(self.values.get(&operand.length()), Some(TerminalScalarValue::Integer {
            scalar_type, value: IntegerValue::Unsigned(length),
        }) if *scalar_type == expected && *length == bytes.len() as u128)
        {
            return Err(malformed());
        }
        Ok(bytes)
    }
}
