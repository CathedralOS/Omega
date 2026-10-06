//! Whole replacement writes the original caller-owned field, after source snapshot.
use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::TerminalScalarValue;
use semantic_vocabulary::IntegerValue;
use terminal_psi::{Operation, OperationKind, OperationResult};

impl TerminalExecution {
    pub(crate) fn execute_byte_sequence_store(
        &mut self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        let OperationKind::ByteSequenceStore {
            destination,
            source,
            length,
            ..
        } = &operation.kind
        else {
            return Err(malformed());
        };
        if operation.result != OperationResult::Unit || source == destination {
            return Err(malformed());
        }
        let ByteSequenceBinding::MutableField {
            field, capacity, ..
        } = self
            .byte_sequence_values
            .get(destination)
            .ok_or_else(malformed)?
        else {
            return Err(malformed());
        };
        let field = field.clone();
        let capacity = *capacity;
        let binding = self
            .byte_sequence_values
            .get(destination)
            .ok_or_else(malformed)?;
        binding.validate_mutable_referent(
            &self.program.structural_types,
            self.structural_values
                .get(destination)
                .ok_or_else(malformed)?,
        )?;
        let TerminalScalarValue::Integer {
            value: IntegerValue::Unsigned(length),
            ..
        } = self.values.get(length).ok_or_else(malformed)?
        else {
            return Err(malformed());
        };
        let bytes = self
            .byte_sequence_values
            .get(source)
            .ok_or_else(malformed)?
            .immutable()?
            .clone();
        if bytes.len() as u128 != *length || *length > u128::from(capacity) {
            return Err(malformed());
        }
        // Immutable backing snapshots the whole input before changing this field.
        self.structural_byte_sequence_fields.insert(field, bytes);
        Ok(())
    }
}
