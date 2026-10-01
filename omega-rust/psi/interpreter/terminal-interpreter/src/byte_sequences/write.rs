//! Fixed-extent byte mutation through an exact mutable loan or fresh owned result.

use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, PlaceId, StructuralPlaceKind, ValueId,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralMultiplicity,
};

use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::values::StructuralByteSequenceRuntimeField;
use crate::values::TerminalScalarValue;

pub(crate) enum MutableByteSequenceStorage {
    Owned(PlaceId),
    Field(StructuralByteSequenceRuntimeField),
    Array {
        backing: crate::structural_inputs::byte_arrays::storage::ArrayStorage,
        offset: u64,
    },
}

impl TerminalExecution {
    pub(crate) fn mutable_byte_sequence_storage(
        &self,
        place: PlaceId,
    ) -> Result<(MutableByteSequenceStorage, u64), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let machine = self
            .program
            .machines
            .get(&self.current_machine)
            .ok_or_else(invalid)?;
        if machine.structural_places.iter().any(|declaration| {
            declaration.id == place
                && matches!(
                    declaration.kind,
                    StructuralPlaceKind::OperationResult { .. }
                )
        }) {
            return self.owned_byte_sequence_storage(place);
        }
        let parameter = machine
            .structural_parameters
            .iter()
            .chain(
                self.program
                    .machines
                    .get(&self.current_machine)
                    .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(
                        self.current_machine,
                    ))?
                    .blocks
                    .values()
                    .flat_map(|block| &block.structural_parameters),
            )
            .find(|parameter| parameter.place == place)
            .ok_or_else(invalid)?;
        let bounded_owner = self
            .program
            .structural_types
            .get(&parameter.structural_type)
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
            });
        if parameter.access != StructuralAccess::MutableBorrow
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || (!bounded_owner && !parameter.qualifications.is_empty())
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
            || !matches!(self.program.structural_types.get(&parameter.structural_type), Some(declaration)
                            if declaration.shape.is_borrowed_byte_carrier() || bounded_owner)
        {
            return Err(invalid());
        }
        let binding = self.byte_sequence_values.get(&place).ok_or_else(invalid)?;
        binding.validate_mutable_referent(&self.program.structural_types, value)?;
        if let ByteSequenceBinding::MutableArray {
            array,
            array_type,
            offset,
            length: window_length,
            ..
        } = binding
        {
            let length = crate::structural_inputs::byte_arrays::byte_array_length(
                &self.program.structural_types,
                *array_type,
            )
            .ok_or_else(invalid)?;
            if self.stored_byte_array_length(array)? as u128 != u128::from(length) {
                return Err(invalid());
            }
            return Ok((
                MutableByteSequenceStorage::Array {
                    backing: array.clone(),
                    offset: *offset,
                },
                *window_length,
            ));
        }
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
        Ok((MutableByteSequenceStorage::Field(field.clone()), length))
    }

    fn owned_byte_sequence_storage(
        &self,
        place: PlaceId,
    ) -> Result<(MutableByteSequenceStorage, u64), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let machine = self
            .program
            .machines
            .get(&self.current_machine)
            .ok_or_else(invalid)?;
        let declaration = machine
            .structural_places
            .iter()
            .find(|declaration| declaration.id == place)
            .ok_or_else(invalid)?;
        let StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } = declaration.kind
        else {
            return Err(invalid());
        };
        let operation = machine
            .blocks
            .values()
            .flat_map(|block| &block.operations)
            .find(|operation| operation.id == producer)
            .ok_or_else(invalid)?;
        let result = operation.result.structural().ok_or_else(invalid)?;
        let value = self.structural_values.get(&place).ok_or_else(invalid)?;
        if !matches!(
            operation.kind,
            OperationKind::EstablishByteSequenceConcatenation { .. }
        ) || result.place != place
            || result.structural_type != structural_type
            || result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
            || !result.claims.is_empty()
            || value.structural_type != structural_type
            || value.qualifications != result.qualifications
            || !value.path.is_empty()
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(place))
            || self
                .live_affine_frontier
                .iter()
                .any(|entry| entry.place == place)
            || terminal_semantics::byte_qualification_predicates_in(
                self.program.catalogs.structural_domains.iter(),
                structural_type,
                &result.qualifications,
            )
            .is_none()
        {
            return Err(invalid());
        }
        let terminal_psi::StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
        ) = self
            .program
            .structural_types
            .get(&structural_type)
            .ok_or_else(invalid)?
            .shape
        else {
            return Err(invalid());
        };
        let bytes = self
            .byte_sequence_values
            .get(&place)
            .ok_or_else(invalid)?
            .immutable()?;
        let length = u64::try_from(bytes.len()).map_err(|_| invalid())?;
        if length > capacity {
            return Err(invalid());
        }
        Ok((MutableByteSequenceStorage::Owned(place), length))
    }

    pub(crate) fn byte_sequence_length(
        &self,
        source: PlaceId,
    ) -> Result<u64, TerminalInterpretError> {
        match self.byte_sequence_values.get(&source).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(source),
        )? {
            ByteSequenceBinding::Immutable(view) => u64::try_from(view.len())
                .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed),
            ByteSequenceBinding::MutableField { .. } | ByteSequenceBinding::MutableArray { .. } => {
                self.mutable_byte_sequence_storage(source)
                    .map(|(_, length)| length)
            }
        }
    }

    /// Dispatch charges fuel before execution. Stage and check all operands and
    /// the destination's live extent before detaching or mutating backing.
    pub(crate) fn execute_byte_sequence_write(
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
        let (storage, current_length) = self.mutable_byte_sequence_storage(destination)?;
        let byte_index = self.byte_sequence_unsigned_operand(index, 64)?;
        let claimed_length = self.byte_sequence_unsigned_operand(length, 64)?;
        let byte = self.byte_sequence_unsigned_operand(value, 8)?;
        if claimed_length != current_length || byte_index >= current_length {
            return Err(invalid());
        }
        let byte = u8::try_from(byte).map_err(|_| invalid())?;
        let (bytes, backing_index) = match storage {
            MutableByteSequenceStorage::Owned(place) => {
                // The payload may also back earlier field snapshots. Detach via
                // replace_byte so mutation never changes those immutable values.
                let Some(ByteSequenceBinding::Immutable(bytes)) =
                    self.byte_sequence_values.get_mut(&place)
                else {
                    return Err(invalid());
                };
                (Some(bytes), byte_index)
            }
            MutableByteSequenceStorage::Field(field) => (
                self.structural_byte_sequence_fields.get_mut(&field),
                byte_index,
            ),
            MutableByteSequenceStorage::Array { backing, offset } => {
                let index = usize::try_from(offset.checked_add(byte_index).ok_or_else(invalid)?)
                    .map_err(|_| invalid())?;
                return self.stored_byte_array_write(&backing, index, byte);
            }
        };
        bytes
            .ok_or_else(invalid)?
            .replace_byte(usize::try_from(backing_index).map_err(|_| invalid())?, byte)
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
