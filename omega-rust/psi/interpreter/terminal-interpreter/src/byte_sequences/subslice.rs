//! Validate and publish one byte window without replacing its backing.

use crate::byte_sequences::binding::ByteSequenceBinding;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, StructuralPlaceKind, ValueId};
use terminal_psi::{Operation, OperationKind, StructuralAccess, StructuralMultiplicity};

use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::values::{TerminalScalarValue, TerminalStructuralValue};

#[cfg(test)]
mod tests;

impl TerminalExecution {
    /// Called only after the dispatch loop has charged this execution. Prepare
    /// the complete view before replacement, including when source and result
    /// storage alias. No failing validation may discard a previous descriptor.
    pub(crate) fn execute_byte_sequence_subslice(
        &mut self,
        operation: &Operation,
    ) -> Result<(), TerminalInterpretError> {
        let OperationKind::ByteSequenceSubslice {
            source,
            start,
            end,
            length,
            ..
        } = &operation.kind
        else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let result = operation
            .result
            .structural()
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let count_type = IntegerType::new(IntegerSign::Unsigned, 64)
            .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)?;
        let count = |operand: &ValueId| -> Result<usize, TerminalInterpretError> {
            let value = self
                .values
                .get(operand)
                .ok_or(TerminalInterpretError::VerifiedValueMissing(*operand))?;
            let TerminalScalarValue::Integer {
                scalar_type,
                value: IntegerValue::Unsigned(value),
            } = value
            else {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            };
            if *scalar_type != count_type {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            let value = u64::try_from(*value)
                .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)?;
            usize::try_from(value).map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)
        };
        let start = count(start)?;
        let end = count(end)?;
        let length = count(length)?;
        let source_value = self.structural_values.get(source).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(*source),
        )?;
        let binding = self.byte_sequence_values.get(source).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(*source),
        )?;
        let mutable = result.multiplicity == StructuralMultiplicity::Affine;
        let source_mutable = matches!(
            binding,
            crate::byte_sequences::binding::ByteSequenceBinding::MutableArray { .. }
                | crate::byte_sequences::binding::ByteSequenceBinding::MutableField { .. }
        );
        let source_length = self.byte_sequence_length(*source)?;
        let machine = self.program.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        // Canonical verification owns unique producer/result IDs. Retain the
        // exact operation declaration here so reuse cannot overwrite a parameter
        // or another producer's storage, even in malformed private runtime state.
        if !machine.structural_places.iter().any(|place| {
            place.id == result.place
                && place.kind
                    == StructuralPlaceKind::OperationResult {
                        producer: operation.id,
                        structural_type: result.structural_type,
                    }
        }) || !matches!(
            self.program.structural_types.get(&result.structural_type),
            Some(declaration)
                if declaration.shape.is_borrowed_byte_view()
        ) || u64::try_from(length).ok() != Some(source_length)
            || start > end
            || end > length
            || source_value.structural_type != result.structural_type
            || (!source_mutable && !source_value.path.is_empty())
            || !source_value.qualifications.is_empty()
            || !matches!(
                result.multiplicity,
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
            )
            || (mutable && !source_mutable)
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(*source) || claim.place == Some(result.place))
            || self.live_affine_frontier.iter().any(|entry| {
                (entry.place == *source && !source_mutable) || entry.place == result.place
            })
            || self.scalar_case_values.contains_key(source)
            || self.scalar_case_values.contains_key(&result.place)
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let destination = TerminalStructuralValue {
            opaque_identity: result.place.get(),
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        match (
            self.structural_values.get(&result.place),
            self.byte_sequence_values.get(&result.place),
        ) {
            (None, None) => {}
            (
                Some(previous),
                Some(crate::byte_sequences::binding::ByteSequenceBinding::Immutable(_)),
            ) if *previous == destination => {}
            _ => return Err(TerminalInterpretError::VerifiedOperationMalformed),
        }
        let (destination, view) = self.narrow_byte_sequence_binding(
            binding,
            source_value,
            start..end,
            if mutable {
                StructuralAccess::MutableBorrow
            } else {
                StructuralAccess::SharedBorrow
            },
            destination,
        )?;
        self.byte_sequence_values.insert(result.place, view);
        self.structural_values.insert(result.place, destination);
        if mutable {
            self.live_affine_frontier
                .insert(terminal_psi::StructuralAffineDiscard {
                    place: result.place,
                    path: Vec::new(),
                    structural_type: result.structural_type,
                });
        }
        Ok(())
    }
    /// Capture a window of an authenticated byte binding before publication.
    /// Region formation and subslicing use the same absolute array coordinates;
    /// descriptor copies never acquire a second backing or require copyback.
    pub(crate) fn narrow_byte_sequence_binding(
        &self,
        binding: &ByteSequenceBinding,
        source: &TerminalStructuralValue,
        window: std::ops::Range<usize>,
        access: StructuralAccess,
        mut destination: TerminalStructuralValue,
    ) -> Result<(TerminalStructuralValue, ByteSequenceBinding), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        if window.start > window.end
            || !matches!(
                access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            )
            || source.structural_type != destination.structural_type
            || !source.qualifications.is_empty()
            || !destination.qualifications.is_empty()
            || !self
                .program
                .structural_types
                .get(&destination.structural_type)
                .is_some_and(|row| row.shape.is_borrowed_byte_view())
        {
            return Err(invalid());
        }
        let narrowed = match binding {
            ByteSequenceBinding::Immutable(bytes) if access == StructuralAccess::SharedBorrow => {
                if !source.path.is_empty() {
                    return Err(invalid());
                }
                ByteSequenceBinding::Immutable(
                    bytes
                        .subslice(window.start, window.end)
                        .ok_or_else(invalid)?,
                )
            }
            ByteSequenceBinding::MutableArray {
                array,
                array_type,
                offset,
                length,
                ..
            } => {
                binding.validate_mutable_referent(&self.program.structural_types, source)?;
                let start = u64::try_from(window.start).map_err(|_| invalid())?;
                let end = u64::try_from(window.end).map_err(|_| invalid())?;
                if end > *length {
                    return Err(invalid());
                }
                let start = offset.checked_add(start).ok_or_else(invalid)?;
                let end = offset.checked_add(end).ok_or_else(invalid)?;
                if usize::try_from(end).map_err(|_| invalid())?
                    > self.stored_byte_array_length(array)?
                {
                    return Err(invalid());
                }
                if access == StructuralAccess::MutableBorrow {
                    destination = source.clone();
                    destination.path = array.referent.path.clone();
                    destination
                        .path
                        .push(terminal_psi::StructuralPathSegment::FixedByteRange { start, end });
                    let narrowed = ByteSequenceBinding::MutableArray {
                        array: array.clone(),
                        array_type: *array_type,
                        offset: start,
                        length: end - start,
                        referent: destination.clone(),
                    };
                    narrowed
                        .validate_mutable_referent(&self.program.structural_types, &destination)?;
                    narrowed
                } else {
                    ByteSequenceBinding::Immutable(self.stored_byte_array_snapshot(
                        array,
                        usize::try_from(start).map_err(|_| invalid())?,
                        usize::try_from(end).map_err(|_| invalid())?,
                    )?)
                }
            }
            ByteSequenceBinding::Immutable(_) | ByteSequenceBinding::MutableField { .. } => {
                return Err(invalid());
            }
        };
        Ok((destination, narrowed))
    }
}
