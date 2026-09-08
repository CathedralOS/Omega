//! Field metadata and length-preserving byte mutation use original referent backing.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use terminal_psi::{Operation, OperationKind, OperationResult};

use super::{TerminalExecution, TerminalInterpretError, TerminalScalarValue};

impl TerminalExecution {
    pub(super) fn execute_structural_byte_sequence_field_length(
        &mut self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let OperationKind::StructuralByteSequenceFieldLength {
            source,
            path,
            field,
        } = &operation.kind
        else {
            return Err(invalid());
        };
        let OperationResult::Scalar(result) = &operation.result else {
            return Err(invalid());
        };
        let count_type = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?;
        if result.scalar_type != ScalarType::Integer(count_type) {
            return Err(invalid());
        }
        let (source_field, capacity) =
            self.resolve_structural_byte_sequence_field(*source, path, *field, false)?;
        let bytes = self
            .structural_byte_sequence_fields
            .get(&source_field)
            .ok_or(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                *source,
            ))?;
        let length = u64::try_from(bytes.len()).map_err(|_| invalid())?;
        if length > capacity {
            return Err(invalid());
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
    pub(super) fn execute_structural_byte_sequence_field_byte_store(
        &mut self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
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
            return Err(invalid());
        };
        if operation.result != OperationResult::Unit {
            return Err(invalid());
        }
        let (destination_field, capacity) =
            self.resolve_structural_byte_sequence_field(*destination, path, *field, true)?;
        let count_type = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?;
        let byte_type = IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| invalid())?;
        let count = |operand| {
            let TerminalScalarValue::Integer {
                scalar_type,
                value: IntegerValue::Unsigned(value),
            } = self
                .values
                .get(&operand)
                .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
            else {
                return Err(invalid());
            };
            if *scalar_type != count_type {
                return Err(invalid());
            }
            u64::try_from(*value).map_err(|_| invalid())
        };
        let byte_index = count(*index)?;
        let live_length = count(*length)?;
        let TerminalScalarValue::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(byte),
        } = self
            .values
            .get(value)
            .ok_or(TerminalInterpretError::VerifiedValueMissing(*value))?
        else {
            return Err(invalid());
        };
        if *scalar_type != byte_type || live_length > capacity || byte_index >= live_length {
            return Err(invalid());
        }
        let byte = u8::try_from(*byte).map_err(|_| invalid())?;
        let byte_index = usize::try_from(byte_index).map_err(|_| invalid())?;
        let bytes = self
            .structural_byte_sequence_fields
            .get_mut(&destination_field)
            .ok_or(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                *destination,
            ))?;
        if u64::try_from(bytes.len()).ok() != Some(live_length) {
            return Err(invalid());
        }
        bytes.replace_byte(byte_index, byte).ok_or_else(invalid)
    }
}
