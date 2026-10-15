//! Field metadata and length-preserving byte mutation use original referent backing.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use terminal_psi::{Operation, OperationKind, OperationResult};

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::TerminalScalarValue;

impl TerminalExecution<'_> {
    pub(crate) fn execute_structural_byte_sequence_field_length(
        &mut self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        let OperationKind::StructuralByteSequenceFieldLength {
            source,
            path,
            field,
        } = &operation.kind
        else {
            return Err(malformed());
        };
        let Some(result) = operation.result.as_scalar() else {
            return Err(malformed());
        };
        let count_type = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
        if result.scalar_type != ScalarType::Integer(count_type) {
            return Err(malformed());
        }
        let (source_field, capacity) = self.readable_byte_field(*source, path, *field)?;
        let length = self.byte_field_length(&source_field)?;
        if capacity.is_some_and(|capacity| length > capacity) {
            return Err(malformed());
        }
        self.values.insert(
            result.id,
            TerminalScalarValue::Integer {
                scalar_type: count_type,
                value: IntegerValue::Unsigned(u128::from(length)),
            },
        );
        Ok(())
    }

    /// Fuel has already been charged. Validate every operand and the current
    /// live extent before detaching shared backing or changing one byte.
    pub(crate) fn execute_structural_byte_sequence_field_byte_store(
        &mut self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        let OperationKind::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            index,
            value,
            length,
            ..
        } = &operation.kind
        else {
            return Err(malformed());
        };
        if operation.result != OperationResult::Unit {
            return Err(malformed());
        }
        let (destination_field, capacity) =
            self.resolve_structural_byte_sequence_field(*destination, path, *field, true)?;
        let count_type = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
        let byte_type = IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?;
        let count = |operand| self.unsigned_integer_value(operand, count_type);
        let byte_index = count(*index)?;
        let live_length = count(*length)?;
        let TerminalScalarValue::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(byte),
        } = self.scalar_value(*value)?
        else {
            return Err(malformed());
        };
        if scalar_type != byte_type || live_length > capacity || byte_index >= live_length {
            return Err(malformed());
        }
        let byte = u8::try_from(byte).map_err(|_| malformed())?;
        let byte_index = usize::try_from(byte_index).map_err(|_| malformed())?;
        let bytes = self
            .structural_byte_sequence_fields
            .get_mut(&destination_field)
            .ok_or(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                *destination,
            ))?
            .immutable_mut()?;
        if u64::try_from(bytes.len()).ok() != Some(live_length) {
            return Err(malformed());
        }
        bytes.replace_byte(byte_index, byte).ok_or_else(malformed)
    }

    /// Read one byte of the field's live extent. Every operand and the
    /// observed length are checked before the indexed read. A shared view
    /// field reads the bytes its stored descriptor denotes.
    pub(crate) fn execute_structural_byte_sequence_field_read(
        &mut self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        let OperationKind::StructuralByteSequenceFieldRead {
            source,
            path,
            field,
            index,
            length,
            ..
        } = &operation.kind
        else {
            return Err(malformed());
        };
        let Some(result) = operation.result.as_scalar() else {
            return Err(malformed());
        };
        let byte_type = IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?;
        if result.scalar_type != ScalarType::Integer(byte_type) {
            return Err(malformed());
        }
        let (source_field, capacity) = self.readable_byte_field(*source, path, *field)?;
        let count_type = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
        let count = |operand| self.unsigned_integer_value(operand, count_type);
        let byte_index = count(*index)?;
        let live_length = count(*length)?;
        if capacity.is_some_and(|capacity| live_length > capacity) || byte_index >= live_length {
            return Err(malformed());
        }
        let bytes = self.byte_field_snapshot(&source_field)?;
        if u64::try_from(bytes.len()).ok() != Some(live_length) {
            return Err(malformed());
        }
        let byte_index = usize::try_from(byte_index).map_err(|_| malformed())?;
        let byte = *bytes.get(byte_index).ok_or_else(malformed)?;
        self.values.insert(
            result.id,
            TerminalScalarValue::Integer {
                scalar_type: byte_type,
                value: IntegerValue::Unsigned(u128::from(byte)),
            },
        );
        Ok(())
    }
}
