//! Concatenation snapshots immutable input values before publishing fresh owned
//! backing. Reexecuting the operation creates a new activation-local value;
//! previous shared observations retain their original bytes.
use crate::byte_sequences::{ByteSequenceView, binding::ByteSequenceBinding};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{OperationFlow, TerminalExecution, operation_kind};
use crate::values::TerminalStructuralValue;
use terminal_psi::{ByteSequenceCarrier, Operation, OperationKind, StructuralTypeShape};

impl TerminalExecution<'_> {
    pub(crate) fn execute_byte_sequence_concatenation(
        &mut self,
        operation: &Operation,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        operation_kind!(operation => EstablishByteSequenceConcatenation { left, right, .. });
        let result = operation.result.structural().ok_or_else(malformed)?;
        let StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) =
            self.program
                .structural_types()
                .get(&result.structural_type)
                .ok_or_else(malformed)?
                .shape
        else {
            return Err(malformed());
        };
        let left = self.byte_sequence_operand_view(left)?;
        let right = self.byte_sequence_operand_view(right)?;
        let length = left.len().checked_add(right.len()).ok_or_else(malformed)?;
        if u64::try_from(length).map_err(|_| malformed())? > capacity {
            return Err(malformed());
        }
        let mut bytes = super::storage::ByteSequenceStorage::zeroed(length, self.account.as_ref())?;
        bytes.bytes_mut()[..left.len()].copy_from_slice(left.bytes());
        bytes.bytes_mut()[left.len()..].copy_from_slice(right.bytes());
        let value = TerminalStructuralValue::minted(
            self.local_structural_identities.allocate()?,
            result.structural_type,
            result.qualifications.clone(),
        );
        self.byte_sequence_values.insert(
            result.place,
            ByteSequenceBinding::Immutable(ByteSequenceView::new(bytes)),
        )?;
        self.structural_values.insert(result.place, value)?;
        Ok(OperationFlow::Advance)
    }
}
