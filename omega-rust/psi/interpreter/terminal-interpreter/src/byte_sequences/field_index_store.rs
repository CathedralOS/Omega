//! Field metadata and length-preserving byte mutation use original referent backing.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use terminal_psi::{Operation, OperationKind, OperationResult};

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::TerminalScalarValue;

impl TerminalExecution {
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
        let OperationResult::Scalar(result) = &operation.result else {
            return Err(malformed());
        };
        let count_type = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
        if result.scalar_type != ScalarType::Integer(count_type) {
            return Err(malformed());
        }
        let (source_field, capacity) = self.readable_byte_field(*source, path, *field)?;
        let bytes = self
            .structural_byte_sequence_fields
            .get(&source_field)
            .ok_or(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                *source,
            ))?;
        let length = u64::try_from(bytes.len()).map_err(|_| malformed())?;
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
        let count = |operand| {
            let TerminalScalarValue::Integer {
                scalar_type,
                value: IntegerValue::Unsigned(value),
            } = self
                .values
                .get(&operand)
                .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
            else {
                return Err(malformed());
            };
            if *scalar_type != count_type {
                return Err(malformed());
            }
            u64::try_from(*value).map_err(|_| malformed())
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
            return Err(malformed());
        };
        if *scalar_type != byte_type || live_length > capacity || byte_index >= live_length {
            return Err(malformed());
        }
        let byte = u8::try_from(*byte).map_err(|_| malformed())?;
        let byte_index = usize::try_from(byte_index).map_err(|_| malformed())?;
        let bytes = self
            .structural_byte_sequence_fields
            .get_mut(&destination_field)
            .ok_or(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                *destination,
            ))?;
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
        let OperationResult::Scalar(result) = &operation.result else {
            return Err(malformed());
        };
        let byte_type = IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?;
        if result.scalar_type != ScalarType::Integer(byte_type) {
            return Err(malformed());
        }
        let (source_field, capacity) = self.readable_byte_field(*source, path, *field)?;
        let count_type = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
        let count = |operand| {
            let TerminalScalarValue::Integer {
                scalar_type,
                value: IntegerValue::Unsigned(value),
            } = self
                .values
                .get(&operand)
                .ok_or(TerminalInterpretError::VerifiedValueMissing(operand))?
            else {
                return Err(malformed());
            };
            if *scalar_type != count_type {
                return Err(malformed());
            }
            u64::try_from(*value).map_err(|_| malformed())
        };
        let byte_index = count(*index)?;
        let live_length = count(*length)?;
        if capacity.is_some_and(|capacity| live_length > capacity) || byte_index >= live_length {
            return Err(malformed());
        }
        let bytes = self
            .structural_byte_sequence_fields
            .get(&source_field)
            .ok_or(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                *source,
            ))?;
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

    /// A readable byte field: a bounded owner with its capacity, or a shared
    /// view field whose descriptor bounds nothing beyond its own extent.
    fn readable_byte_field(
        &self,
        root: semantic_vocabulary::PlaceId,
        path: &[terminal_psi::StructuralPathSegment],
        field: semantic_vocabulary::StructuralFieldId,
    ) -> Result<
        (
            crate::values::StructuralByteSequenceRuntimeField,
            Option<u64>,
        ),
        TerminalInterpretError,
    > {
        let (cell, carrier) =
            self.resolve_structural_byte_field_carrier(root, path, field, false)?;
        match carrier {
            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity } => {
                Ok((cell, Some(capacity)))
            }
            terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            } => Ok((cell, None)),
            _ => Err(malformed()),
        }
    }
}
