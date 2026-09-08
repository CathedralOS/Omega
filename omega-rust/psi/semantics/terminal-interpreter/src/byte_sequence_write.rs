//! Fixed-extent byte mutation through an existing exact mutable field loan.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, PlaceId, ValueId};
use terminal_psi::{
    ByteSequenceCarrier, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralMultiplicity, StructuralTypeShape,
};

use super::{
    ByteSequenceBinding, StructuralByteSequenceRuntimeField, TerminalExecution,
    TerminalInterpretError, TerminalScalarValue,
};

impl TerminalExecution {
    pub(super) fn mutable_byte_sequence_field(
        &self,
        place: PlaceId,
    ) -> Result<(StructuralByteSequenceRuntimeField, u64), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let machine = self
            .machines
            .get(&self.current_machine)
            .ok_or_else(invalid)?;
        let parameter = machine
            .structural_parameters
            .iter()
            .chain(
                self.blocks
                    .values()
                    .flat_map(|block| &block.structural_parameters),
            )
            .find(|parameter| parameter.place == place)
            .ok_or_else(invalid)?;
        if parameter.access != StructuralAccess::MutableBorrow
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(place))
        {
            return Err(invalid());
        }
        let value = self.structural_values.get(&place).ok_or_else(invalid)?;
        if value.structural_type != parameter.structural_type
            || !matches!(self.structural_types.get(&parameter.structural_type), Some(declaration)
                if declaration.shape == StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView))
        {
            return Err(invalid());
        }
        let binding = self.byte_sequence_values.get(&place).ok_or_else(invalid)?;
        binding.validate_mutable_referent(&self.structural_types, value)?;
        let ByteSequenceBinding::MutableField {
            field, capacity, ..
        } = binding
        else {
            return Err(invalid());
        };
        let bytes = self
            .structural_byte_sequence_fields
            .get(field)
            .ok_or_else(invalid)?;
        let length = u64::try_from(bytes.len()).map_err(|_| invalid())?;
        if length > *capacity {
            return Err(invalid());
        }
        Ok((field.clone(), length))
    }

    pub(super) fn byte_sequence_length(
        &self,
        source: PlaceId,
    ) -> Result<u64, TerminalInterpretError> {
        match self.byte_sequence_values.get(&source).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(source),
        )? {
            ByteSequenceBinding::Immutable(view) => u64::try_from(view.len())
                .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed),
            ByteSequenceBinding::MutableField { .. } => self
                .mutable_byte_sequence_field(source)
                .map(|(_, length)| length),
        }
    }

    /// Dispatch charges fuel before execution. Stage and check all operands and
    /// the original field's live extent before detaching or mutating backing.
    pub(super) fn execute_byte_sequence_write(
        &mut self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let OperationKind::ByteSequenceWrite {
            destination,
            index,
            value,
            length,
            ..
        } = operation.kind
        else {
            return Err(invalid());
        };
        if operation.result != OperationResult::Unit {
            return Err(invalid());
        }
        let (field, current_length) = self.mutable_byte_sequence_field(destination)?;
        let byte_index = self.byte_sequence_unsigned_operand(index, 64)?;
        let claimed_length = self.byte_sequence_unsigned_operand(length, 64)?;
        let byte = self.byte_sequence_unsigned_operand(value, 8)?;
        if claimed_length != current_length || byte_index >= current_length {
            return Err(invalid());
        }
        let byte_index = usize::try_from(byte_index).map_err(|_| invalid())?;
        let byte = u8::try_from(byte).map_err(|_| invalid())?;
        self.structural_byte_sequence_fields
            .get_mut(&field)
            .ok_or_else(invalid)?
            .replace_byte(byte_index, byte)
            .ok_or_else(invalid)
    }

    fn byte_sequence_unsigned_operand(
        &self,
        operand: ValueId,
        bits: u16,
    ) -> Result<u64, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
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
        if *scalar_type != IntegerType::new(IntegerSign::Unsigned, bits).map_err(|_| invalid())? {
            return Err(invalid());
        }
        u64::try_from(*value).map_err(|_| invalid())
    }
}
