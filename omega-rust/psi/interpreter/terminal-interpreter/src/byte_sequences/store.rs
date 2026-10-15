//! Whole replacement writes the original caller-owned field or frame-local
//! owner, after source snapshot.
use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::byte_sequences::write::MutableByteSequenceStorage;
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
        // Replacing a whole loaned owner requires no read of its prior live
        // extent. Write-only formals retain that distinction from indexed writes.
        let binding = self
            .byte_sequence_values
            .get(destination)
            .ok_or_else(malformed)?;
        let value = self
            .structural_values
            .get(destination)
            .ok_or_else(malformed)?;
        let (owner, capacity) = match binding {
            ByteSequenceBinding::MutableField {
                field, capacity, ..
            } => {
                binding.validate_mutable_referent(self, value)?;
                (MutableByteSequenceStorage::Field(field.clone()), *capacity)
            }
            ByteSequenceBinding::MutableOwner { home, capacity, .. } => {
                binding.validate_mutable_referent(self, value)?;
                (MutableByteSequenceStorage::Owned(home.clone()), *capacity)
            }
            ByteSequenceBinding::Immutable(_) => {
                // Established frame values share the indexed writer's exact
                // owned-home admission; they need not first borrow themselves.
                let (owner, _) = self.mutable_byte_sequence_storage(*destination)?;
                let terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                ) = self
                    .program
                    .structural_types()
                    .get(&value.structural_type)
                    .ok_or_else(malformed)?
                    .shape
                else {
                    return Err(malformed());
                };
                (owner, capacity)
            }
            ByteSequenceBinding::MutableArray { .. } => return Err(malformed()),
        };
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
            MutableByteSequenceStorage::Field(field) => {
                self.structural_byte_sequence_fields
                    .insert(field, ByteSequenceBinding::Immutable(bytes));
            }
            MutableByteSequenceStorage::Owned(home) => {
                *self.byte_owner_bytes_mut(&home)? = bytes.detached_descriptor();
            }
            MutableByteSequenceStorage::Array { .. } => return Err(malformed()),
        }
        Ok(())
    }
}
