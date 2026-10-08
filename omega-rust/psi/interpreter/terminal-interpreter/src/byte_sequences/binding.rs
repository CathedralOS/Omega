//! Frame-owned byte arguments distinguish immutable values from field loans.

use crate::byte_sequences::ByteSequenceView;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::structural_inputs::byte_arrays;
use crate::structural_inputs::byte_arrays::storage::ArrayStorage;
use crate::values::StructuralByteSequenceRuntimeField;
use crate::values::TerminalStructuralValue;
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::StructuralAccess;
use terminal_psi::StructuralArgument;
use terminal_psi::StructuralMultiplicity;
use terminal_psi::StructuralPathSegment;
use terminal_psi::StructuralTypeDeclaration;
use terminal_psi::StructuralTypeShape;
#[derive(Clone)]
pub(crate) enum ByteSequenceBinding {
    Immutable(ByteSequenceView),
    MutableArray {
        array: ArrayStorage,
        array_type: StructuralTypeId,
        offset: u64,
        length: u64,
        referent: TerminalStructuralValue,
    },
    MutableField {
        field: StructuralByteSequenceRuntimeField,
        capacity: u64,
        parent_type: StructuralTypeId,
        referent: TerminalStructuralValue,
    },
    /// An exclusive loan of a frame-local bounded owner. Its bytes stay in
    /// the owner's activation; the loan pins that home and writes it in place.
    MutableOwner {
        home: ByteOwnerHome,
        capacity: u64,
        referent: TerminalStructuralValue,
    },
}

/// The exact activation and place holding one frame-local bounded owner.
/// Stack depth is stable across nested calls and state edges; the machine
/// prevents a later activation at the same depth from answering for it.
/// This is the frame-home contract primitive and array loans already use.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct ByteOwnerHome {
    pub(crate) depth: usize,
    pub(crate) machine: semantic_vocabulary::MachineId,
    pub(crate) place: semantic_vocabulary::PlaceId,
}

impl ByteSequenceBinding {
    pub(crate) fn immutable(&self) -> Result<&ByteSequenceView, TerminalInterpretError> {
        match self {
            Self::Immutable(view) => Ok(view),
            Self::MutableField { .. } | Self::MutableArray { .. } | Self::MutableOwner { .. } => {
                Err(malformed())
            }
        }
    }

    /// The referent an exclusive owner loan presents to its formal.
    pub(crate) fn owner_loan_referent(&self) -> Option<&TerminalStructuralValue> {
        match self {
            Self::MutableField { referent, .. } | Self::MutableOwner { referent, .. } => {
                Some(referent)
            }
            Self::Immutable(_) | Self::MutableArray { .. } => None,
        }
    }

    pub(crate) fn validate_mutable_referent(
        &self,
        structural_types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
        value: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        if let Self::MutableArray {
            array,
            array_type,
            offset,
            length,
            referent,
        } = self
        {
            let (backing_path, exact_window) = match referent.path.split_last() {
                Some((StructuralPathSegment::FixedByteRange { start, end }, backing)) => (
                    backing,
                    *start == *offset && start.checked_add(*length) == Some(*end),
                ),
                _ => (
                    referent.path.as_slice(),
                    *offset == 0
                        && byte_arrays::byte_array_length(structural_types, *array_type)
                            == Some(*length),
                ),
            };
            return if value == referent
                && array.referent.opaque_identity == referent.opaque_identity
                && array.referent.path == backing_path
                && exact_window
                && referent.qualifications.is_empty()
                && byte_arrays::byte_array_length(structural_types, *array_type).is_some_and(
                    |extent| offset.checked_add(*length).is_some_and(|end| end <= extent),
                ) {
                Ok(())
            } else {
                Err(malformed())
            };
        }
        if let Self::MutableOwner {
            capacity, referent, ..
        } = self
        {
            // The owner's own value under the formal's exact domains: a whole,
            // unprojected bounded owner of the loan's capacity.
            return if value == referent
                && referent.path.is_empty()
                && structural_types
                    .get(&referent.structural_type)
                    .is_some_and(|declaration| {
                        declaration.shape
                            == StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedOwned {
                                    capacity: *capacity,
                                },
                            )
                    })
            {
                Ok(())
            } else {
                Err(malformed())
            };
        }
        let Self::MutableField {
            field,
            capacity,
            parent_type,
            referent,
        } = self
        else {
            return Err(malformed());
        };
        let Some((StructuralPathSegment::Field(identity), prefix)) = referent.path.split_last()
        else {
            return Err(malformed());
        };
        if value != referent
            || field.parent.opaque_identity != referent.opaque_identity
            || field.parent.path != prefix
            || (!referent.qualifications.is_empty()
                && !structural_types
                    .get(&referent.structural_type)
                    .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence()))
        {
            return Err(malformed());
        }
        let Some(StructuralTypeDeclaration {
            shape: StructuralTypeShape::Record { fields, .. },
            ..
        }) = structural_types.get(parent_type)
        else {
            return Err(malformed());
        };
        if !fields.iter().any(|declaration| {
            declaration.id == field.field
                && declaration.identity == *identity
                && !declaration.relevance.is_erased()
                && declaration.field_type.byte_sequence_carrier()
                    == Some(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                        capacity: *capacity,
                    })
        }) {
            return Err(malformed());
        }
        Ok(())
    }
}

impl TerminalExecution<'_> {
    /// Observe a shared descriptor at its exact current home, whether frame-local
    /// or relocated into an aggregate. The map supplies contents only; exact
    /// carrier type, immutable access and home correspondence remain mandatory.
    pub(crate) fn shared_byte_descriptor(
        &self,
        argument: &StructuralArgument,
        value: &TerminalStructuralValue,
    ) -> Result<&ByteSequenceView, TerminalInterpretError> {
        if !matches!(
            argument.access,
            StructuralAccess::Owned | StructuralAccess::SharedBorrow
        ) || !self
            .program
            .structural_types()
            .get(&value.structural_type)
            .is_some_and(|row| row.shape.is_shared_readable_byte_view())
        {
            return Err(malformed());
        }
        let root = self
            .structural_values
            .get(&argument.place)
            .ok_or_else(malformed)?;
        let selected = crate::custody::project_structural_value(
            &self.program.structural_types(),
            root,
            &argument.path,
        )?;
        if selected != *value {
            return Err(malformed());
        }
        if argument.path.is_empty()
            && let Some(binding) = self.byte_sequence_values.get(&argument.place)
        {
            let view = binding.immutable()?;
            return view
                .matches_descriptor_home(value)
                .then_some(view)
                .ok_or_else(malformed);
        }
        self.structural_byte_storage
            .get(&crate::values::StructuralRuntimePlace::from(value))
            .ok_or_else(malformed)
    }

    pub(crate) fn prepare_shared_descriptor_argument(
        &self,
        parameter: &terminal_psi::StructuralParameterDeclaration,
        argument: &StructuralArgument,
    ) -> Result<Option<(TerminalStructuralValue, ByteSequenceBinding)>, TerminalInterpretError>
    {
        if argument.path.is_empty()
            && self
                .byte_sequence_values
                .get(&argument.place)
                .is_some_and(|binding| !matches!(binding, ByteSequenceBinding::Immutable(_)))
        {
            // Exclusive descriptors retain their existing reborrow/attenuation
            // owner; a shared shape alone cannot recategorize that authority.
            return Ok(None);
        }
        let Some(root) = self.structural_values.get(&argument.place) else {
            return Ok(None);
        };
        let Ok(value) = crate::custody::project_structural_value(
            &self.program.structural_types(),
            root,
            &argument.path,
        ) else {
            return Ok(None);
        };
        if !self
            .program
            .structural_types()
            .get(&value.structural_type)
            .is_some_and(|row| row.shape.is_shared_readable_byte_view())
        {
            return Ok(None);
        }
        if parameter.structural_type != value.structural_type
            || parameter.access != StructuralAccess::SharedBorrow
            || argument.access != parameter.access
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.projected_qualifications.is_empty()
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(argument.place))
        {
            return Err(malformed());
        }
        let bytes = self.shared_byte_descriptor(argument, &value)?.clone();
        // A lent descriptor must satisfy every domain the formal names
        // (structural access, shared view fields). The checked call proved
        // that implication; execution fails closed on the actual bytes
        // rather than trusting whichever domains the source value spells,
        // and the formal then holds the descriptor under those domains.
        if !terminal_semantics::byte_literal_qualifications_hold_in(
            self.program.catalogs.structural_domains.iter(),
            parameter.structural_type,
            &parameter.qualifications,
            bytes.bytes(),
        ) {
            return Err(malformed());
        }
        let mut value = value;
        value.absorb_qualifications(&parameter.qualifications);
        let bytes = bytes.with_descriptor_home(value.clone());
        Ok(Some((value, ByteSequenceBinding::Immutable(bytes))))
    }

    /// Reborrow a live byte descriptor, retaining its home for exclusive use
    /// and exposing only an immutable snapshot when authority is attenuated.
    pub(crate) fn reborrow_byte_sequence_binding(
        &self,
        binding: &ByteSequenceBinding,
        value: &TerminalStructuralValue,
        access: StructuralAccess,
    ) -> Result<ByteSequenceBinding, TerminalInterpretError> {
        if !matches!(
            access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) {
            return Err(malformed());
        }
        if let ByteSequenceBinding::Immutable(view) = binding {
            return if access == StructuralAccess::SharedBorrow
                && view.matches_descriptor_home(value)
            {
                Ok(binding.clone())
            } else {
                Err(malformed())
            };
        }
        binding.validate_mutable_referent(&self.program.structural_types(), value)?;
        if access == StructuralAccess::MutableBorrow {
            return Ok(binding.clone());
        }
        let snapshot = match binding {
            ByteSequenceBinding::MutableArray {
                array,
                offset,
                length,
                ..
            } => self.stored_byte_array_snapshot(
                array,
                usize::try_from(*offset).map_err(|_| malformed())?,
                usize::try_from(offset.checked_add(*length).ok_or_else(malformed)?)
                    .map_err(|_| malformed())?,
            )?,
            ByteSequenceBinding::MutableField { field, .. } => self
                .structural_byte_sequence_fields
                .get(field)
                .ok_or_else(malformed)?
                .clone(),
            ByteSequenceBinding::MutableOwner { home, .. } => self.byte_owner_bytes(*home)?.clone(),
            ByteSequenceBinding::Immutable(_) => unreachable!(),
        };
        Ok(ByteSequenceBinding::Immutable(
            snapshot.with_descriptor_home(value.clone()),
        ))
    }

    /// The live bytes and capacity an exclusive owner loan names: a bounded
    /// byte field or a pinned frame-local owner. Each stays within capacity.
    pub(crate) fn exclusive_owner_bytes(
        &self,
        binding: &ByteSequenceBinding,
    ) -> Result<(&ByteSequenceView, u64), TerminalInterpretError> {
        let (bytes, capacity) = match binding {
            ByteSequenceBinding::MutableField {
                field, capacity, ..
            } => (
                self.structural_byte_sequence_fields
                    .get(field)
                    .ok_or_else(malformed)?,
                *capacity,
            ),
            ByteSequenceBinding::MutableOwner { home, capacity, .. } => {
                (self.byte_owner_bytes(*home)?, *capacity)
            }
            ByteSequenceBinding::Immutable(_) | ByteSequenceBinding::MutableArray { .. } => {
                return Err(malformed());
            }
        };
        if bytes.len() as u128 > u128::from(capacity) {
            return Err(malformed());
        }
        Ok((bytes, capacity))
    }

    /// The live bytes of a pinned frame-local owner. Its activation must still
    /// be the one the loan captured, and the owner must still hold its value.
    pub(crate) fn byte_owner_bytes(
        &self,
        home: ByteOwnerHome,
    ) -> Result<&ByteSequenceView, TerminalInterpretError> {
        let (machine, values) = if home.depth == self.call_stack.len() {
            (self.current_machine, &self.byte_sequence_values)
        } else {
            let frame = self.call_stack.get(home.depth).ok_or_else(malformed)?;
            (frame.current_machine, &frame.byte_sequence_values)
        };
        if machine != home.machine {
            return Err(malformed());
        }
        values.get(&home.place).ok_or_else(malformed)?.immutable()
    }

    /// The same pinned owner's bytes, for an exclusive write through its loan.
    pub(crate) fn byte_owner_bytes_mut(
        &mut self,
        home: ByteOwnerHome,
    ) -> Result<&mut ByteSequenceView, TerminalInterpretError> {
        let (machine, values) = if home.depth == self.call_stack.len() {
            (self.current_machine, &mut self.byte_sequence_values)
        } else {
            let frame = self.call_stack.get_mut(home.depth).ok_or_else(malformed)?;
            (frame.current_machine, &mut frame.byte_sequence_values)
        };
        if machine != home.machine {
            return Err(malformed());
        }
        match values.get_mut(&home.place) {
            Some(ByteSequenceBinding::Immutable(bytes)) => Ok(bytes),
            _ => Err(malformed()),
        }
    }
}
