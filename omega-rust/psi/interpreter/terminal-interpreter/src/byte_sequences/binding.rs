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
        /// A compiler-owned fixed descriptor has its own identity, separate
        /// from the borrowed array. Ordinary borrowed formals use the referent.
        descriptor_home: Option<TerminalStructuralValue>,
    },
    MutableField {
        field: StructuralByteSequenceRuntimeField,
        capacity: u64,
        parent_type: StructuralTypeId,
        referent: TerminalStructuralValue,
        descriptor_home: Option<TerminalStructuralValue>,
    },
    /// An exclusive loan of a declared bounded owner. Its bytes stay at the
    /// original frame or typed subtree home throughout the loan.
    MutableOwner {
        home: ByteOwnerHome,
        capacity: u64,
        referent: TerminalStructuralValue,
        descriptor_home: Option<TerminalStructuralValue>,
    },
}

/// Frame bindings retain their activation; typed subtree cells retain their
/// execution-wide storage coordinate. Neither home grants loan authority.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum ByteOwnerHome {
    Frame {
        depth: usize,
        machine: semantic_vocabulary::MachineId,
        place: semantic_vocabulary::PlaceId,
    },
    Storage(crate::values::StructuralRuntimePlace),
}

impl ByteSequenceBinding {
    /// Access belongs to the captured backing, independently of the descriptor
    /// home's ownership or later placement in another structural value.
    pub(crate) fn access(&self) -> StructuralAccess {
        match self {
            Self::Immutable(_) => StructuralAccess::SharedBorrow,
            Self::MutableArray { .. } | Self::MutableField { .. } | Self::MutableOwner { .. } => {
                StructuralAccess::MutableBorrow
            }
        }
    }

    pub(crate) fn descriptor_home(&self) -> Option<&TerminalStructuralValue> {
        match self {
            Self::Immutable(_) => None,
            Self::MutableArray {
                descriptor_home,
                referent,
                ..
            }
            | Self::MutableField {
                descriptor_home,
                referent,
                ..
            }
            | Self::MutableOwner {
                descriptor_home,
                referent,
                ..
            } => Some(descriptor_home.as_ref().unwrap_or(referent)),
        }
    }
    pub(crate) fn immutable_mut(
        &mut self,
    ) -> Result<&mut ByteSequenceView, TerminalInterpretError> {
        match self {
            Self::Immutable(view) => Ok(view),
            _ => Err(malformed()),
        }
    }

    /// Only an authenticated frame or declaration-keyed field lookup calls
    /// this. It relocates descriptor metadata, not referent storage. A captured
    /// descriptor may outlive replacement of its former slot on the RHS.
    pub(crate) fn with_descriptor_home(mut self, home: TerminalStructuralValue) -> Self {
        match &mut self {
            Self::Immutable(view) => *view = view.clone().with_descriptor_home(home),
            Self::MutableArray {
                descriptor_home, ..
            }
            | Self::MutableField {
                descriptor_home, ..
            }
            | Self::MutableOwner {
                descriptor_home, ..
            } => *descriptor_home = Some(home),
        }
        self
    }
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

    /// Re-authenticate a retained binding against the identity it presents:
    /// its descriptor home once a frame or field lookup relocated it, else
    /// its own window. Comparing a relocated binding with its raw window would
    /// reject the very loan this binding records.
    pub(crate) fn validate_presented_referent(
        &self,
        execution: &TerminalExecution,
    ) -> Result<(), TerminalInterpretError> {
        self.validate_mutable_referent(execution, self.descriptor_home().ok_or_else(malformed)?)
    }

    pub(crate) fn validate_mutable_referent(
        &self,
        execution: &TerminalExecution,
        value: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        let structural_types = execution.program.structural_types();
        if let Self::MutableArray {
            array,
            array_type,
            offset,
            length,
            referent,
            descriptor_home,
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
                        && byte_arrays::byte_array_length(&structural_types, *array_type)
                            == Some(*length),
                ),
            };
            return if value == descriptor_home.as_ref().unwrap_or(referent)
                && descriptor_home.as_ref().is_none_or(|home| {
                    terminal_semantics::descriptor_reborrow_matches(
                        structural_types.values(),
                        referent.structural_type,
                        StructuralAccess::MutableBorrow,
                        Some(StructuralAccess::MutableBorrow),
                        home.structural_type,
                        StructuralAccess::MutableBorrow,
                    ) && (home.qualifications.is_empty()
                        || home.qualifications == referent.qualifications)
                        && home.projected_qualifications.is_empty()
                })
                && array.referent.opaque_identity == referent.opaque_identity
                && array.referent.path == backing_path
                && exact_window
                && referent.projected_qualifications.is_empty()
                && terminal_semantics::byte_qualification_predicates_in(
                    execution.program.catalogs.structural_domains.iter(),
                    referent.structural_type,
                    &referent.qualifications,
                )
                .is_some()
                && byte_arrays::byte_array_length(&structural_types, *array_type).is_some_and(
                    |extent| offset.checked_add(*length).is_some_and(|end| end <= extent),
                ) {
                Ok(())
            } else {
                Err(malformed())
            };
        }
        if let Self::MutableOwner {
            home,
            capacity,
            referent,
            descriptor_home,
        } = self
        {
            // A formal's type cannot authenticate an unrelated subtree cell.
            // Rejoin the stored coordinate to its live containing owner too.
            let exact_home = match home {
                ByteOwnerHome::Frame { .. } => referent.path.is_empty(),
                ByteOwnerHome::Storage(home) => {
                    *home == crate::values::StructuralRuntimePlace::from(referent)
                        && execution.actual_storage_type_from_owner(home)?
                            == referent.structural_type
                }
            };
            return if value == descriptor_home.as_ref().unwrap_or(referent)
                && exact_home
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
            descriptor_home,
        } = self
        else {
            return Err(malformed());
        };
        let Some((StructuralPathSegment::Field(identity), prefix)) = referent.path.split_last()
        else {
            return Err(malformed());
        };
        if value != descriptor_home.as_ref().unwrap_or(referent)
            || field.parent.opaque_identity != referent.opaque_identity
            || field.parent.path != prefix
            || (!referent.qualifications.is_empty()
                && !structural_types
                    .get(&referent.structural_type)
                    .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence()))
        {
            return Err(malformed());
        }
        let case = match field.namespace {
            crate::values::StructuralFieldNamespace::Member => None,
            crate::values::StructuralFieldNamespace::CasePayload => Some(
                execution
                    .structural_cases
                    .get(&field.parent)
                    .ok_or_else(malformed)?
                    .case,
            ),
        };
        let (fields, _) = execution.active_field_declarations(*parent_type, &field.parent, case)?;
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
    /// Field lookup has already authenticated the parent and declaration. Read
    /// metadata from the retained binding without observing payload contents.
    pub(crate) fn byte_field_length(
        &self,
        field: &StructuralByteSequenceRuntimeField,
    ) -> Result<u64, TerminalInterpretError> {
        let binding = self
            .structural_byte_sequence_fields
            .get(field)
            .ok_or_else(malformed)?;
        match binding {
            ByteSequenceBinding::Immutable(bytes) => {
                u64::try_from(bytes.len()).map_err(|_| malformed())
            }
            _ => self
                .byte_descriptor_length(binding, binding.descriptor_home().ok_or_else(malformed)?),
        }
    }

    /// Payload consumers first authenticate the selected field's read ceiling.
    /// Shared observation snapshots the exact retained window, never its slot
    /// or the containing record's storage.
    pub(crate) fn byte_field_snapshot(
        &self,
        field: &StructuralByteSequenceRuntimeField,
    ) -> Result<ByteSequenceView, TerminalInterpretError> {
        let binding = self
            .structural_byte_sequence_fields
            .get(field)
            .ok_or_else(malformed)?;
        if let ByteSequenceBinding::Immutable(bytes) = binding {
            return Ok(bytes.clone());
        }
        self.reborrow_byte_sequence_binding(
            binding,
            binding.descriptor_home().ok_or_else(malformed)?,
            StructuralAccess::SharedBorrow,
        )?
        .immutable()
        .cloned()
    }

    /// Select a declared byte owner at its live home. Inline fields have no
    /// standalone declaration and retain their declaration-keyed storage;
    /// every other projection keeps the selected nominal type and memberships.
    pub(crate) fn declared_byte_owner(
        &self,
        argument: &StructuralArgument,
    ) -> Result<Option<TerminalStructuralValue>, TerminalInterpretError> {
        let root = self
            .structural_values
            .get(&argument.place)
            .ok_or_else(malformed)?;
        let types = self.program.structural_types();
        if crate::custody::inline_structural_field(&types, root.structural_type, &argument.path)
            .is_some_and(|field| field.field_type.byte_sequence_carrier().is_some())
        {
            return Ok(None);
        }
        let selected =
            self.resolve_live_projection(argument.place, &argument.path, argument.access)?;
        Ok(types
            .get(&selected.structural_type)
            .is_some_and(|row| row.shape.is_bounded_owned_byte_sequence())
            .then_some(selected))
    }

    /// Rejoin an already resolved descriptor to its exact frame binding or
    /// retained aggregate slot. Cloning a binding retains its backing/window;
    /// it neither consumes the source nor copies mutable storage. A whole-home
    /// descriptor with no explicit home is authoritative only at its keyed
    /// structural binding, never because some other whole path is empty.
    pub(crate) fn byte_descriptor_binding(
        &self,
        argument: &StructuralArgument,
        selected: &TerminalStructuralValue,
    ) -> Result<ByteSequenceBinding, TerminalInterpretError> {
        if argument.access == StructuralAccess::WriteOnlyBorrow
            || !self
                .program
                .structural_types()
                .get(&selected.structural_type)
                .is_some_and(|row| {
                    // This selects frame custody, not descriptor words. A
                    // bounded loan keeps its statically known extent.
                    row.shape.is_borrowed_byte_carrier()
                        || row.shape.is_bounded_owned_byte_sequence()
                })
        {
            return Err(malformed());
        }
        let capture = |binding: &ByteSequenceBinding| match binding {
            ByteSequenceBinding::Immutable(bytes) => {
                ByteSequenceBinding::Immutable(bytes.clone().with_descriptor_home(selected.clone()))
            }
            binding => binding.clone(),
        };
        let matches = |binding: &ByteSequenceBinding, home: &TerminalStructuralValue| match binding
        {
            ByteSequenceBinding::Immutable(bytes) => {
                bytes.matches_bound_descriptor_home(home, selected)
            }
            _ => {
                home.opaque_identity == selected.opaque_identity
                    && home.path == selected.path
                    && home.structural_type == selected.structural_type
                    && binding.validate_mutable_referent(self, selected).is_ok()
            }
        };
        if argument.path.is_empty()
            && let Some(binding) = self.byte_sequence_values.get(&argument.place)
        {
            return self
                .structural_values
                .get(&argument.place)
                .filter(|home| matches(binding, home))
                .map(|_| capture(binding))
                .ok_or_else(malformed);
        }
        if let Some(binding) = self.captured_byte_owner_binding(selected) {
            // A by-value owner observation snapshots its bytes; it does not
            // request ownership from the older loan that identifies this home.
            // Explicit borrowed observations retain their existing access.
            let observation_access = match argument.access {
                StructuralAccess::Owned => StructuralAccess::SharedBorrow,
                access => access,
            };
            return self.reborrow_byte_sequence_binding(binding, selected, observation_access);
        }
        // Inline byte fields have declaration-keyed cells rather than array
        // cells. Resolve the parent first so reference crossings and captured
        // selectors retain the same actual home as the descriptor observation.
        if let Some((StructuralPathSegment::Field(identity), prefix)) = argument.path.split_last() {
            let (parent_path, case) = match prefix.split_last() {
                Some((StructuralPathSegment::Case(case), parent)) => (parent, Some(*case)),
                _ => (prefix, None),
            };
            let parent = self.resolve_live_projection(
                argument.place,
                parent_path,
                StructuralAccess::SharedBorrow,
            )?;
            let (fields, _) = self.active_field_declarations(
                parent.structural_type,
                &crate::values::StructuralRuntimePlace::from(&parent),
                case,
            )?;
            let field =
                terminal_psi::live_field_by_identity(fields, *identity).ok_or_else(malformed)?;
            if field.field_type.byte_sequence_carrier().is_some() {
                let (cell, _) = self.readable_byte_field(argument.place, prefix, field.id)?;
                let bytes = self
                    .structural_byte_sequence_fields
                    .get(&cell)
                    .ok_or_else(malformed)?;
                return Ok(bytes.clone().with_descriptor_home(selected.clone()));
            }
        }
        if let Some(bytes) = self
            .structural_byte_storage
            .get(&crate::values::StructuralRuntimePlace::from(selected))
        {
            return Ok(ByteSequenceBinding::Immutable(
                bytes.clone().with_descriptor_home(selected.clone()),
            ));
        }
        self.byte_sequence_values
            .iter()
            .find_map(|(place, binding)| {
                self.structural_values
                    .get(place)
                    .filter(|home| matches(binding, home))
                    .map(|_| capture(binding))
            })
            .or_else(|| {
                self.call_stack.iter().find_map(|frame| {
                    frame
                        .byte_sequence_values
                        .iter()
                        .find_map(|(place, binding)| {
                            frame
                                .structural_values
                                .get(place)
                                .filter(|home| matches(binding, home))
                                .map(|_| capture(binding))
                        })
                })
            })
            .ok_or_else(malformed)
    }

    /// Observe the live extent of the exact selected byte descriptor. Mutable
    /// backing is read through its existing array/field/activation owner.
    pub(crate) fn byte_descriptor_length(
        &self,
        binding: &ByteSequenceBinding,
        selected: &TerminalStructuralValue,
    ) -> Result<u64, TerminalInterpretError> {
        let length = match binding {
            ByteSequenceBinding::Immutable(bytes) => {
                if !bytes.matches_descriptor_home(selected) {
                    return Err(malformed());
                }
                return u64::try_from(bytes.len()).map_err(|_| malformed());
            }
            ByteSequenceBinding::MutableArray {
                array,
                offset,
                length,
                ..
            } => {
                binding.validate_mutable_referent(self, selected)?;
                let backing_length = self.stored_byte_array_length(array)?;
                if offset
                    .checked_add(*length)
                    .is_none_or(|end| end as u128 > backing_length as u128)
                {
                    return Err(malformed());
                }
                return Ok(*length);
            }
            _ => {
                binding.validate_mutable_referent(self, selected)?;
                self.exclusive_owner_bytes(binding)?.0.len()
            }
        };
        u64::try_from(length).map_err(|_| malformed())
    }

    /// Observe a shared descriptor at its exact current home, whether frame-local
    /// or relocated into an aggregate. The map supplies contents only; exact
    /// carrier type, immutable access and home correspondence remain mandatory.
    pub(crate) fn shared_byte_descriptor(
        &self,
        argument: &StructuralArgument,
        value: &TerminalStructuralValue,
    ) -> Result<ByteSequenceView, TerminalInterpretError> {
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
        self.validate_descriptor_borrow(argument, StructuralAccess::SharedBorrow)?;
        let selected = self.resolve_live_projection(
            argument.place,
            &argument.path,
            StructuralAccess::SharedBorrow,
        )?;
        if selected != *value {
            return Err(malformed());
        }
        let binding = self.byte_descriptor_binding(argument, &selected)?;
        let bytes = binding.immutable()?;
        bytes
            .matches_descriptor_home(&selected)
            .then(|| bytes.clone())
            .ok_or_else(malformed)
    }

    /// Transfer a view's descriptor with its exact producer custody.
    /// Ownership applies to the descriptor, never the caller's bytes: retain
    /// the binding's existing home/window and exclusive/shared permission.
    /// A mutable descriptor is affine; only a shared descriptor is copyable.
    pub(crate) fn prepare_owned_byte_descriptor_argument(
        &self,
        parameter: &terminal_psi::StructuralParameterDeclaration,
        argument: &StructuralArgument,
        resolved: &TerminalStructuralValue,
    ) -> Result<Option<(TerminalStructuralValue, ByteSequenceBinding)>, TerminalInterpretError>
    {
        let Some(StructuralTypeShape::ByteSequence(carrier)) = self
            .program
            .structural_types()
            .get(&parameter.structural_type)
            .map(|declaration| &declaration.shape)
        else {
            return Ok(None);
        };
        if parameter.access != StructuralAccess::Owned || !carrier.is_borrowed() {
            return Ok(None);
        }
        let (access, capacity) = match *carrier {
            terminal_psi::ByteSequenceCarrier::BorrowedView { access } => (access, None),
            terminal_psi::ByteSequenceCarrier::BoundedView { access, capacity } => {
                (access, Some(capacity))
            }
            _ => return Ok(None),
        };
        let owned_source = self
            .owned_block_source(argument.place)
            .is_some_and(|source| {
                source.access == StructuralAccess::Owned
                    && source.structural_type == parameter.structural_type
                    && source.multiplicity == parameter.multiplicity
            })
            || self.owned_result_matches(argument.place, parameter);
        if argument.access != StructuralAccess::Owned
            || !argument.path.is_empty()
            || parameter.structural_type != resolved.structural_type
            || !parameter.projected_qualifications.is_empty()
            || !resolved.projected_qualifications.is_empty()
        {
            return Err(malformed());
        }
        let binding = self.byte_descriptor_binding(argument, resolved)?;
        let permission_matches = match &binding {
            ByteSequenceBinding::Immutable(bytes) => {
                // Copying an unrestricted shared descriptor neither owns nor
                // copies its referent. Authenticate readable carrier custody
                // and the formal's actual byte domains, just as a shared loan
                // does; qualifications do not make descriptor words affine.
                self.validate_descriptor_borrow(argument, StructuralAccess::SharedBorrow)?;
                parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    && matches!(access, None | Some(StructuralAccess::SharedBorrow))
                    && terminal_semantics::byte_literal_qualifications_hold_in(
                        self.program.catalogs.structural_domains.iter(),
                        parameter.structural_type,
                        &parameter.qualifications,
                        bytes.bytes(),
                    )
            }
            ByteSequenceBinding::MutableArray { .. }
            | ByteSequenceBinding::MutableField { .. }
            | ByteSequenceBinding::MutableOwner { .. } => {
                parameter.multiplicity == StructuralMultiplicity::Affine
                    && matches!(access, None | Some(StructuralAccess::MutableBorrow))
                    && owned_source
                    && parameter.qualifications.is_empty()
                    && resolved.qualifications.is_empty()
            }
        };
        let length = self.byte_descriptor_length(&binding, resolved)?;
        if !permission_matches || capacity.is_some_and(|capacity| length != capacity) {
            return Err(malformed());
        }
        Ok(Some((resolved.clone(), binding)))
    }

    pub(crate) fn prepare_byte_descriptor_argument(
        &self,
        parameter: &terminal_psi::StructuralParameterDeclaration,
        argument: &StructuralArgument,
    ) -> Result<Option<(TerminalStructuralValue, ByteSequenceBinding)>, TerminalInterpretError>
    {
        if parameter.access == StructuralAccess::Owned {
            // Ownership transfer is admitted from the actual producer, not a
            // borrowed descriptor's geometry or the access of its backing.
            return Ok(None);
        }
        if !self.structural_values.contains_key(&argument.place) {
            return Ok(None);
        }
        // A stored reference lends the descriptor at its live referent. The
        // pure inline projection cannot cross that retained pointer identity.
        let Ok(value) =
            self.resolve_live_projection(argument.place, &argument.path, argument.access)
        else {
            return Ok(None);
        };
        if !self
            .program
            .structural_types()
            .get(&value.structural_type)
            .is_some_and(|row| row.shape.is_borrowed_byte_carrier())
        {
            return Ok(None);
        }
        // An affine mutable window is a descriptor loan, not an unrestricted
        // copy. The staged array arrival lends the owner's bytes; rejecting
        // the multiplicity here drops a match result the checker already
        // admitted.
        if parameter.multiplicity == StructuralMultiplicity::Affine
            && parameter.access == StructuralAccess::MutableBorrow
        {
            return Ok(None);
        }
        // A loan captures the actual descriptor, whether frame-local or in a
        // field. Declared geometry grants no read/write permission: authenticate
        // every source crossing before retaining or attenuating its binding.
        if !matches!(
            parameter.access,
            terminal_psi::reading_borrow_pair_pattern!()
        ) || argument.access != parameter.access
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.projected_qualifications.is_empty()
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(argument.place))
        {
            return Err(malformed());
        }
        self.validate_descriptor_borrow(argument, parameter.access)?;
        let binding = self.byte_descriptor_binding(argument, &value)?;
        if !terminal_semantics::descriptor_reborrow_matches(
            self.program.structural_types().values(),
            value.structural_type,
            parameter.access,
            Some(binding.access()),
            parameter.structural_type,
            parameter.access,
        ) {
            return Err(malformed());
        }
        let binding = self.reborrow_byte_sequence_binding(&binding, &value, parameter.access)?;
        // A lent descriptor must satisfy every domain the formal names
        // (structural access, shared view fields). The checked call proved
        // that implication; execution fails closed on the actual bytes
        // rather than trusting whichever domains the source value spells,
        // and the formal then holds the descriptor under those domains.
        if parameter.access == StructuralAccess::MutableBorrow {
            if !parameter.qualifications.is_empty() || !value.qualifications.is_empty() {
                return Err(malformed());
            }
        } else if !terminal_semantics::byte_literal_qualifications_hold_in(
            self.program.catalogs.structural_domains.iter(),
            parameter.structural_type,
            &parameter.qualifications,
            binding.immutable()?.bytes(),
        ) {
            return Err(malformed());
        }
        let mut value = value;
        value.structural_type = parameter.structural_type;
        value.absorb_qualifications(&parameter.qualifications);
        let binding = binding.with_descriptor_home(value.clone());
        Ok(Some((value, binding)))
    }

    /// Reborrow a live byte descriptor, retaining its home for exclusive use
    /// and exposing only an immutable snapshot when authority is attenuated.
    pub(crate) fn reborrow_byte_sequence_binding(
        &self,
        binding: &ByteSequenceBinding,
        value: &TerminalStructuralValue,
        access: StructuralAccess,
    ) -> Result<ByteSequenceBinding, TerminalInterpretError> {
        if !matches!(access, terminal_psi::reading_borrow_pair_pattern!()) {
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
        binding.validate_mutable_referent(self, value)?;
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
                .immutable()?
                .clone(),
            ByteSequenceBinding::MutableOwner { home, .. } => self.byte_owner_bytes(home)?.clone(),
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
                    .ok_or_else(malformed)?
                    .immutable()?,
                *capacity,
            ),
            ByteSequenceBinding::MutableOwner { home, capacity, .. } => {
                (self.byte_owner_bytes(home)?, *capacity)
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

    /// The live bytes of the captured owner. Storage cells must already exist;
    /// frame homes must still belong to the captured activation.
    pub(crate) fn byte_owner_bytes(
        &self,
        home: &ByteOwnerHome,
    ) -> Result<&ByteSequenceView, TerminalInterpretError> {
        let (depth, expected_machine, place) = match home {
            ByteOwnerHome::Frame {
                depth,
                machine,
                place,
            } => (depth, machine, place),
            ByteOwnerHome::Storage(home) => {
                return self.structural_byte_storage.get(home).ok_or_else(malformed);
            }
        };
        let (machine, values) = if *depth == self.call_stack.len() {
            (self.current_machine, &self.byte_sequence_values)
        } else {
            let frame = self.call_stack.get(*depth).ok_or_else(malformed)?;
            (frame.current_machine, &frame.byte_sequence_values)
        };
        if machine != *expected_machine {
            return Err(malformed());
        }
        values.get(place).ok_or_else(malformed)?.immutable()
    }

    /// The same pinned owner's bytes, for an exclusive write through its loan.
    pub(crate) fn byte_owner_bytes_mut(
        &mut self,
        home: &ByteOwnerHome,
    ) -> Result<&mut ByteSequenceView, TerminalInterpretError> {
        let (depth, expected_machine, place) = match home {
            ByteOwnerHome::Frame {
                depth,
                machine,
                place,
            } => (depth, machine, place),
            ByteOwnerHome::Storage(home) => {
                return self
                    .structural_byte_storage
                    .get_mut(home)
                    .ok_or_else(malformed);
            }
        };
        let (machine, values) = if *depth == self.call_stack.len() {
            (self.current_machine, &mut self.byte_sequence_values)
        } else {
            let frame = self.call_stack.get_mut(*depth).ok_or_else(malformed)?;
            (frame.current_machine, &mut frame.byte_sequence_values)
        };
        if machine != *expected_machine {
            return Err(malformed());
        }
        match values.get_mut(place) {
            Some(ByteSequenceBinding::Immutable(bytes)) => Ok(bytes),
            _ => Err(malformed()),
        }
    }
}
