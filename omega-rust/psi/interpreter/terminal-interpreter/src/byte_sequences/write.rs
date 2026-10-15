//! Fixed-extent byte mutation through an exact mutable loan or established owner.

use semantic_vocabulary::{IntegerSign, IntegerType, PlaceId, ValueId};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralMultiplicity,
};

use crate::byte_sequences::binding::{ByteOwnerHome, ByteSequenceBinding};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::StructuralByteSequenceRuntimeField;

pub(crate) enum MutableByteSequenceStorage {
    /// A frame-local bounded owner, written in its own activation.
    Owned(ByteOwnerHome),
    Field(StructuralByteSequenceRuntimeField),
    Array {
        backing: crate::structural_inputs::byte_arrays::storage::ArrayStorage,
        offset: u64,
    },
}

impl TerminalExecution<'_> {
    pub(crate) fn mutable_byte_sequence_storage(
        &self,
        place: PlaceId,
    ) -> Result<(MutableByteSequenceStorage, u64), TerminalInterpretError> {
        let machine = self
            .program
            .machines()
            .get(&self.current_machine)
            .ok_or_else(malformed)?;
        let result = machine
            .structural_place(place)
            .and_then(|declaration| declaration.kind.as_operation_result());
        let owned_type = machine
            .structural_parameter(place)
            .filter(|parameter| parameter.access == StructuralAccess::Owned)
            .map(|parameter| parameter.structural_type)
            .or_else(|| result.map(|(_, structural_type)| structural_type));
        if owned_type.is_some_and(|structural_type| {
            self.program
                .structural_types()
                .get(&structural_type)
                .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence())
        }) {
            return self.owned_byte_sequence_storage(place);
        }
        let (structural_type, qualifications, projected_qualifications) =
            if let Some((producer, structural_type)) = result {
                let operation = machine.operation(producer).ok_or_else(malformed)?;
                let result = operation.result.structural().ok_or_else(malformed)?;
                // A completed readable descriptor carries the same backing
                // permission whether established here or returned by a call.
                // Its exact live affine home and binding are still required.
                if terminal_semantics::descriptor_results::result_access(
                    self.program.structural_types().values(),
                    result,
                ) != Some(StructuralAccess::MutableBorrow)
                    || result.place != place
                    || result.structural_type != structural_type
                    || result.multiplicity != StructuralMultiplicity::Affine
                    || !result.qualification_establishments.is_empty()
                    || !result.claims.is_empty()
                    || !self
                        .live_affine_frontier
                        .contains(&terminal_psi::StructuralAffineDiscard {
                            place,
                            path: Vec::new(),
                            structural_type,
                        })
                {
                    return Err(malformed());
                }
                (
                    structural_type,
                    &result.qualifications,
                    &result.projected_qualifications,
                )
            } else {
                let parameter = machine.structural_parameter(place).ok_or_else(malformed)?;
                if parameter.access != StructuralAccess::MutableBorrow
                    || (parameter.multiplicity != StructuralMultiplicity::Unrestricted
                        && !terminal_semantics::view_descriptor_parameter(
                            parameter,
                            self.program.structural_types().values(),
                        ))
                {
                    return Err(malformed());
                }
                (
                    parameter.structural_type,
                    &parameter.qualifications,
                    &parameter.projected_qualifications,
                )
            };
        let bounded_owner = self
            .program
            .structural_types()
            .get(&structural_type)
            .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence());
        if (!bounded_owner && !qualifications.is_empty())
            || !projected_qualifications.is_empty()
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(place))
        {
            return Err(malformed());
        }
        let value = self.structural_values.get(&place).ok_or_else(malformed)?;
        if value.structural_type != structural_type
            || !matches!(self.program.structural_types().get(&structural_type), Some(declaration)
                            if declaration.shape.is_borrowed_byte_carrier() || bounded_owner)
        {
            return Err(malformed());
        }
        let binding = self
            .byte_sequence_values
            .get(&place)
            .ok_or_else(malformed)?;
        binding.validate_mutable_referent(self, value)?;
        if let ByteSequenceBinding::MutableArray {
            array,
            array_type,
            offset,
            length: window_length,
            ..
        } = binding
        {
            let length = crate::structural_inputs::byte_arrays::byte_array_length(
                &self.program.structural_types(),
                *array_type,
            )
            .ok_or_else(malformed)?;
            if self.stored_byte_array_length(array)? as u128 != u128::from(length) {
                return Err(malformed());
            }
            return Ok((
                MutableByteSequenceStorage::Array {
                    backing: array.clone(),
                    offset: *offset,
                },
                *window_length,
            ));
        }
        let storage = match binding {
            ByteSequenceBinding::MutableField { field, .. } => {
                MutableByteSequenceStorage::Field(field.clone())
            }
            ByteSequenceBinding::MutableOwner { home, .. } => {
                MutableByteSequenceStorage::Owned(home.clone())
            }
            ByteSequenceBinding::Immutable(_) | ByteSequenceBinding::MutableArray { .. } => {
                return Err(malformed());
            }
        };
        let (bytes, _) = self.exclusive_owner_bytes(binding)?;
        Ok((
            storage,
            u64::try_from(bytes.len()).map_err(|_| malformed())?,
        ))
    }

    fn owned_byte_sequence_storage(
        &self,
        place: PlaceId,
    ) -> Result<(MutableByteSequenceStorage, u64), TerminalInterpretError> {
        let machine = self
            .program
            .machines()
            .get(&self.current_machine)
            .ok_or_else(malformed)?;
        // Owned parameters and completed owned results both bind activation
        // storage. Mutation must detach that home's payload, never an immutable
        // literal descriptor or another activation's backing.
        let (structural_type, qualifications, projected_qualifications, multiplicity) =
            if let Some(parameter) = machine.structural_parameter(place) {
                if parameter.access != StructuralAccess::Owned {
                    return Err(malformed());
                }
                (
                    parameter.structural_type,
                    &parameter.qualifications,
                    &parameter.projected_qualifications,
                    parameter.multiplicity,
                )
            } else {
                let declaration = machine.structural_place(place).ok_or_else(malformed)?;
                let (producer, structural_type) = declaration
                    .kind
                    .as_operation_result()
                    .ok_or_else(malformed)?;
                let operation = machine.operation(producer).ok_or_else(malformed)?;
                let result = operation.result.structural().ok_or_else(malformed)?;
                if !matches!(
                    operation.kind,
                    OperationKind::EstablishByteSequenceConcatenation { .. }
                ) || result.place != place
                    || result.structural_type != structural_type
                    || !result.qualification_establishments.is_empty()
                    || !result.claims.is_empty()
                {
                    return Err(malformed());
                }
                (
                    structural_type,
                    &result.qualifications,
                    &result.projected_qualifications,
                    result.multiplicity,
                )
            };
        let value = self.structural_values.get(&place).ok_or_else(malformed)?;
        if multiplicity != StructuralMultiplicity::Unrestricted
            || !projected_qualifications.is_empty()
            || value.structural_type != structural_type
            || &value.qualifications != qualifications
            || !value.path.is_empty()
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(place))
            || self.live_affine_discard_at(place).is_some()
            || terminal_semantics::byte_qualification_predicates_in(
                self.program.catalogs.structural_domains.iter(),
                structural_type,
                qualifications,
            )
            .is_none()
        {
            return Err(malformed());
        }
        let terminal_psi::StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
        ) = self
            .program
            .structural_types()
            .get(&structural_type)
            .ok_or_else(malformed)?
            .shape
        else {
            return Err(malformed());
        };
        let bytes = self
            .byte_sequence_values
            .get(&place)
            .ok_or_else(malformed)?
            .immutable()?;
        let length = u64::try_from(bytes.len()).map_err(|_| malformed())?;
        if length > capacity {
            return Err(malformed());
        }
        Ok((
            MutableByteSequenceStorage::Owned(ByteOwnerHome::Frame {
                depth: self.call_stack.len(),
                machine: self.current_machine,
                place,
            }),
            length,
        ))
    }

    pub(crate) fn byte_sequence_length(
        &self,
        source: PlaceId,
    ) -> Result<u64, TerminalInterpretError> {
        match crate::custody::verified_structural_place(&self.byte_sequence_values, source)? {
            ByteSequenceBinding::Immutable(view) => {
                u64::try_from(view.len()).map_err(|_| malformed())
            }
            ByteSequenceBinding::MutableField { .. }
            | ByteSequenceBinding::MutableArray { .. }
            | ByteSequenceBinding::MutableOwner { .. } => self
                .mutable_byte_sequence_storage(source)
                .map(|(_, length)| length),
        }
    }

    pub(crate) fn byte_sequence_read(
        &self,
        source: PlaceId,
        index: usize,
    ) -> Result<u8, TerminalInterpretError> {
        match self
            .byte_sequence_values
            .get(&source)
            .ok_or_else(malformed)?
        {
            ByteSequenceBinding::Immutable(view) => view.get(index).copied().ok_or_else(malformed),
            ByteSequenceBinding::MutableArray { .. }
            | ByteSequenceBinding::MutableField { .. }
            | ByteSequenceBinding::MutableOwner { .. } => {
                let (storage, length) = self.mutable_byte_sequence_storage(source)?;
                if index as u128 >= u128::from(length) {
                    return Err(malformed());
                }
                match storage {
                    MutableByteSequenceStorage::Array { backing, offset } => {
                        let index = usize::try_from(offset)
                            .ok()
                            .and_then(|offset| offset.checked_add(index))
                            .ok_or_else(malformed)?;
                        self.stored_byte_array_read(&backing, index)
                    }
                    MutableByteSequenceStorage::Field(field) => self
                        .structural_byte_sequence_fields
                        .get(&field)
                        .ok_or_else(malformed)?
                        .immutable()?
                        .get(index)
                        .copied()
                        .ok_or_else(malformed),
                    MutableByteSequenceStorage::Owned(home) => self
                        .byte_owner_bytes(&home)?
                        .get(index)
                        .copied()
                        .ok_or_else(malformed),
                }
            }
        }
    }

    /// Dispatch charges fuel before execution. Stage and check all operands and
    /// the destination's live extent before detaching or mutating backing.
    pub(crate) fn execute_byte_sequence_write(
        &mut self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        let OperationKind::ByteSequenceWrite {
            destination,
            index,
            value,
            length,
            ..
        } = operation.kind
        else {
            return Err(malformed());
        };
        if operation.result != OperationResult::Unit {
            return Err(malformed());
        }
        let (storage, current_length) = self.mutable_byte_sequence_storage(destination)?;
        let byte_index = self.byte_sequence_unsigned_operand(index, 64)?;
        let claimed_length = self.byte_sequence_unsigned_operand(length, 64)?;
        let byte = self.byte_sequence_unsigned_operand(value, 8)?;
        if claimed_length != current_length || byte_index >= current_length {
            return Err(malformed());
        }
        let byte = u8::try_from(byte).map_err(|_| malformed())?;
        let (bytes, backing_index) = match storage {
            MutableByteSequenceStorage::Owned(home) => {
                // The payload may also back earlier field snapshots. Detach via
                // replace_byte so mutation never changes those immutable values.
                (Some(self.byte_owner_bytes_mut(&home)?), byte_index)
            }
            MutableByteSequenceStorage::Field(field) => (
                Some(
                    self.structural_byte_sequence_fields
                        .get_mut(&field)
                        .ok_or_else(malformed)?
                        .immutable_mut()?,
                ),
                byte_index,
            ),
            MutableByteSequenceStorage::Array { backing, offset } => {
                let index = usize::try_from(offset.checked_add(byte_index).ok_or_else(malformed)?)
                    .map_err(|_| malformed())?;
                return self.stored_byte_array_write(&backing, index, byte);
            }
        };
        bytes
            .ok_or_else(malformed)?
            .replace_byte(
                usize::try_from(backing_index).map_err(|_| malformed())?,
                byte,
            )
            .ok_or_else(malformed)
    }

    fn byte_sequence_unsigned_operand(
        &self,
        operand: ValueId,
        bits: u16,
    ) -> Result<u64, TerminalInterpretError> {
        self.unsigned_integer_value(
            operand,
            IntegerType::new(IntegerSign::Unsigned, bits).map_err(|_| malformed())?,
        )
    }
}
