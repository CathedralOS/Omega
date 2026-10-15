//! Validate and publish one byte window without replacing its backing.

use crate::byte_sequences::binding::ByteSequenceBinding;
use semantic_vocabulary::{IntegerSign, IntegerType, ValueId};
use terminal_psi::{Operation, OperationKind, StructuralAccess, StructuralMultiplicity};

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::TerminalStructuralValue;

impl TerminalExecution<'_> {
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
            return Err(malformed());
        };
        let result = operation.result.structural().ok_or(malformed())?;
        let count_type = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
        let count = |operand: &ValueId| -> Result<usize, TerminalInterpretError> {
            usize::try_from(self.unsigned_integer_value(*operand, count_type)?)
                .map_err(|_| malformed())
        };
        let start = count(start)?;
        let end = count(end)?;
        let length = count(length)?;
        let mutable = result.multiplicity == StructuralMultiplicity::Affine;
        let access = if mutable {
            StructuralAccess::MutableBorrow
        } else {
            StructuralAccess::SharedBorrow
        };
        self.validate_descriptor_borrow(source, access)?;
        let source_value = self.resolve_live_projection(source.place, &source.path, access)?;
        let binding = self.byte_descriptor_binding(source, &source_value)?;
        let source_length = self.byte_descriptor_length(&binding, &source_value)?;
        let machine = self.program.verified_machine(self.current_machine)?;
        // Canonical verification owns unique producer/result IDs. Retain the
        // exact operation declaration here so reuse cannot overwrite a parameter
        // or another producer's storage, even in malformed private runtime state.
        if !machine.structural_places.iter().any(|place| {
            place.id == result.place
                && place.kind.as_operation_result()
                    == Some((operation.id, result.structural_type))
        }) || !matches!(
            self.program.structural_types().get(&result.structural_type),
            Some(declaration)
                if declaration.shape.is_borrowed_byte_view()
        ) || u64::try_from(length).ok() != Some(source_length)
            || start > end
            || end > length
            || source_value.structural_type != result.structural_type
            // A shared window only reads its source; it claims none of the
            // source's predicates (its result is unqualified), so a qualified
            // source lends it as freely as a plain one. A mutable window
            // could break a predicate the source still declares.
            || (mutable && !source_value.qualifications.is_empty())
            || !matches!(
                result.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            )
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
            || !result.claims.is_empty()
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(source.place) || claim.place == Some(result.place))
            || self.live_affine_discard_at(result.place).is_some()
        {
            return Err(malformed());
        }
        match (
            self.structural_values.get(&result.place),
            self.byte_sequence_values.get(&result.place),
        ) {
            (None, None) => {}
            (Some(previous), Some(ByteSequenceBinding::Immutable(bytes)))
                if *previous
                    == TerminalStructuralValue::minted(
                        previous.opaque_identity,
                        result.structural_type,
                        Vec::new(),
                    )
                    && bytes.matches_bound_descriptor_home(previous, previous) => {}
            _ => return Err(malformed()),
        }
        let mut identity_cursor = self.local_structural_identities.cursor();
        let destination = TerminalStructuralValue::minted(
            self.local_structural_identities
                .allocate_staged(&mut identity_cursor)?,
            result.structural_type,
            Vec::new(),
        );
        let (destination, view) = self.narrow_byte_sequence_binding(
            &binding,
            &source_value,
            start..end,
            if mutable {
                StructuralAccess::MutableBorrow
            } else {
                StructuralAccess::SharedBorrow
            },
            destination,
        )?;
        self.local_structural_identities
            .commit_cursor(identity_cursor);
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
        destination: TerminalStructuralValue,
    ) -> Result<(TerminalStructuralValue, ByteSequenceBinding), TerminalInterpretError> {
        if window.start > window.end
            || !matches!(
                access,terminal_psi::reading_borrow_pair_pattern!())
            || source.structural_type != destination.structural_type
            // As for an established window: a shared narrowing reads its
            // qualified source without claiming its predicates.
            || (access != StructuralAccess::SharedBorrow && !source.qualifications.is_empty())
            || !destination.qualifications.is_empty()
            || !self
                .program
                .structural_types()
                .get(&destination.structural_type)
                .is_some_and(|row| row.shape.is_borrowed_byte_view())
        {
            return Err(malformed());
        }
        let shared;
        let binding = if access == StructuralAccess::SharedBorrow {
            shared = self.reborrow_byte_sequence_binding(binding, source, access)?;
            &shared
        } else {
            binding
        };
        let narrowed = match binding {
            ByteSequenceBinding::Immutable(bytes) if access == StructuralAccess::SharedBorrow => {
                if !bytes.matches_descriptor_home(source) {
                    return Err(malformed());
                }
                ByteSequenceBinding::Immutable(
                    bytes
                        .subslice(window.start, window.end)
                        .ok_or_else(malformed)?,
                )
            }
            ByteSequenceBinding::MutableArray {
                array,
                array_type,
                offset,
                length,
                referent,
                ..
            } => {
                binding.validate_mutable_referent(self, source)?;
                let start = u64::try_from(window.start).map_err(|_| malformed())?;
                let end = u64::try_from(window.end).map_err(|_| malformed())?;
                if end > *length {
                    return Err(malformed());
                }
                let start = offset.checked_add(start).ok_or_else(malformed)?;
                let end = offset.checked_add(end).ok_or_else(malformed)?;
                if usize::try_from(end).map_err(|_| malformed())?
                    > self.stored_byte_array_length(array)?
                {
                    return Err(malformed());
                }
                if access == StructuralAccess::MutableBorrow {
                    let mut referent = referent.clone();
                    referent.path = array.referent.path.clone();
                    referent
                        .path
                        .push(terminal_psi::StructuralPathSegment::FixedByteRange { start, end });
                    let narrowed = ByteSequenceBinding::MutableArray {
                        array: array.clone(),
                        array_type: *array_type,
                        offset: start,
                        length: end - start,
                        referent,
                        descriptor_home: Some(destination.clone()),
                    };
                    narrowed.validate_mutable_referent(self, &destination)?;
                    narrowed
                } else {
                    ByteSequenceBinding::Immutable(self.stored_byte_array_snapshot(
                        array,
                        usize::try_from(start).map_err(|_| malformed())?,
                        usize::try_from(end).map_err(|_| malformed())?,
                    )?)
                }
            }
            ByteSequenceBinding::Immutable(_)
            | ByteSequenceBinding::MutableField { .. }
            | ByteSequenceBinding::MutableOwner { .. } => {
                return Err(malformed());
            }
        };
        let narrowed = match narrowed {
            ByteSequenceBinding::Immutable(bytes) => {
                ByteSequenceBinding::Immutable(bytes.with_descriptor_home(destination.clone()))
            }
            binding => binding,
        };
        Ok((destination, narrowed))
    }
}
