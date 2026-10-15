//! Whole replacement writes the original caller-owned field or frame-local
//! owner, after source snapshot.
use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::TerminalScalarValue;
use semantic_vocabulary::IntegerValue;
use terminal_psi::{Operation, OperationKind, OperationResult};

impl TerminalExecution<'_> {
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
        let binding = self
            .byte_sequence_values
            .get(destination)
            .ok_or_else(malformed)?;
        let (owner, capacity) = match binding {
            ByteSequenceBinding::MutableField {
                field, capacity, ..
            } => (ReplacedOwner::Field(field.clone()), *capacity),
            ByteSequenceBinding::MutableOwner { home, capacity, .. } => {
                (ReplacedOwner::Declared(home.clone()), *capacity)
            }
            ByteSequenceBinding::Immutable(_) | ByteSequenceBinding::MutableArray { .. } => {
                return Err(malformed());
            }
        };
        binding.validate_mutable_referent(
            self,
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
        // Immutable backing snapshots the whole input before changing the owner.
        match owner {
            ReplacedOwner::Field(field) => {
                self.structural_byte_sequence_fields
                    .insert(field, ByteSequenceBinding::Immutable(bytes));
            }
            ReplacedOwner::Declared(home) => {
                *self.byte_owner_bytes_mut(&home)? = bytes.detached_descriptor();
            }
        }
        Ok(())
    }
}

/// The caller-owned storage a whole replacement writes: a bounded byte field
/// or a pinned frame-local owner.
enum ReplacedOwner {
    Field(crate::values::StructuralByteSequenceRuntimeField),
    Declared(crate::byte_sequences::binding::ByteOwnerHome),
}
