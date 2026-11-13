/*
    Accounted byte-sequence storage, bounded byte-field bindings, and typed or
    representation element views in the Terminal Psi interpreter.

    We track immutable, owned, and representation-backed byte sequences and
    element views, reserving live byte capacity before allocation or copy-on-write
    detachment.
*/

pub(crate) mod byte_sequences {
    pub(crate) mod binding {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::structural_inputs::byte_arrays;
        use crate::boundary::structural_inputs::byte_arrays::storage::ArrayStorage;
        use crate::boundary::values::StructuralByteSequenceRuntimeField;
        use crate::boundary::values::TerminalStructuralValue;
        use crate::engine::execution::TerminalExecution;
        use crate::engine::execution::frame_field_mut;
        use crate::storage::byte_sequences::ByteSequenceView;
        use semantic_vocabulary::StructuralTypeId;
        use terminal_psi::{
            ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
            StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeShape,
            live_field_by_identity,
        };
        use terminal_semantics::{borrowed_storage_parameter, borrowed_storage_reference_ceiling};
        #[derive(Clone)]
        pub(crate) enum ByteSequenceBinding {
            Immutable(ByteSequenceView),
            Representation(std::sync::Arc<crate::storage::element_views::ElementView>),
            MutableArray {
                array: ArrayStorage,
                array_type: StructuralTypeId,
                offset: u64,
                length: u64,
                referent: TerminalStructuralValue,
                descriptor_home: Option<TerminalStructuralValue>,
            },
            MutableField {
                field: StructuralByteSequenceRuntimeField,
                capacity: u64,
                parent_type: StructuralTypeId,
                referent: TerminalStructuralValue,
                descriptor_home: Option<TerminalStructuralValue>,
            },
            MutableOwner {
                home: ByteOwnerHome,
                capacity: u64,
                referent: TerminalStructuralValue,
                descriptor_home: Option<TerminalStructuralValue>,
            },
        }

        enum ByteDescriptorSource {
            Stored(ByteSequenceBinding),
            CapturedReferent(ByteSequenceBinding),
        }

        #[derive(Clone, PartialEq, Eq)]
        pub(crate) enum ByteOwnerHome {
            Frame {
                depth: usize,
                machine: semantic_vocabulary::MachineId,
                place: semantic_vocabulary::PlaceId,
            },
            Storage(crate::boundary::values::StructuralRuntimePlace),
        }

        impl ByteSequenceBinding {
            pub(crate) fn access(&self) -> StructuralAccess {
                match self {
                    Self::Immutable(_) | Self::Representation(_) => StructuralAccess::SharedBorrow,
                    Self::MutableArray { .. }
                    | Self::MutableField { .. }
                    | Self::MutableOwner { .. } => StructuralAccess::MutableBorrow,
                }
            }

            pub(crate) fn descriptor_home(&self) -> Option<&TerminalStructuralValue> {
                match self {
                    Self::Immutable(_) => None,
                    Self::Representation(view) => view.descriptor_home(),
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

            pub(crate) fn with_descriptor_home(mut self, home: TerminalStructuralValue) -> Self {
                match &mut self {
                    Self::Immutable(view) => *view = view.clone().with_descriptor_home(home),
                    Self::Representation(view) => {
                        *view =
                            std::sync::Arc::new(view.as_ref().clone().with_descriptor_home(home))
                    }
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

            pub(crate) fn relocated_descriptor(
                &self,
                source: &crate::boundary::values::StructuralRuntimePlace,
                destination: &crate::boundary::values::StructuralRuntimePlace,
            ) -> Result<Self, TerminalInterpretError> {
                let home = match self {
                    Self::Immutable(bytes) => bytes.descriptor_home(),
                    binding => binding.descriptor_home(),
                }
                .ok_or_else(malformed)?;
                if crate::boundary::values::StructuralRuntimePlace::from(home) != *source {
                    return Err(malformed());
                }
                let mut relocated = home.clone();
                relocated.opaque_identity = destination.opaque_identity;
                relocated.path = destination.path.clone();
                Ok(self.clone().with_descriptor_home(relocated))
            }
            pub(crate) fn immutable(&self) -> Result<&ByteSequenceView, TerminalInterpretError> {
                match self {
                    Self::Immutable(view) => Ok(view),
                    Self::Representation(_)
                    | Self::MutableField { .. }
                    | Self::MutableArray { .. }
                    | Self::MutableOwner { .. } => Err(malformed()),
                }
            }

            pub(crate) fn snapshot(
                &self,
                execution: &TerminalExecution,
            ) -> Result<ByteSequenceView, TerminalInterpretError> {
                if let Self::Immutable(bytes) = self {
                    return Ok(bytes.clone());
                }
                let read = self.read_view(execution)?;
                Ok(ByteSequenceView::new(
                    super::storage::ByteSequenceStorage::from_values(
                        read.len(),
                        (0..read.len()).map(|index| read.read(index)),
                        execution.account.as_ref(),
                    )?,
                ))
            }

            pub(crate) fn read_view<'storage>(
                &'storage self,
                execution: &'storage TerminalExecution<'_>,
            ) -> Result<super::read::TerminalByteRead<'storage>, TerminalInterpretError>
            {
                match self {
                    Self::Immutable(bytes) => {
                        Ok(super::read::TerminalByteRead::contiguous(bytes.bytes()))
                    }
                    Self::Representation(view) => Ok(
                        super::read::TerminalByteRead::representation(execution, view),
                    ),
                    _ => Err(malformed()),
                }
            }

            pub(crate) fn matches_shared_home(&self, value: &TerminalStructuralValue) -> bool {
                match self {
                    Self::Immutable(view) => view.matches_descriptor_home(value),
                    Self::Representation(view) => view.matches_descriptor_home(value),
                    _ => false,
                }
            }

            pub(crate) fn owner_loan_referent(&self) -> Option<&TerminalStructuralValue> {
                match self {
                    Self::MutableField { referent, .. } | Self::MutableOwner { referent, .. } => {
                        Some(referent)
                    }
                    Self::Immutable(_) | Self::Representation(_) | Self::MutableArray { .. } => {
                        None
                    }
                }
            }

            pub(crate) fn validate_presented_referent(
                &self,
                execution: &TerminalExecution,
            ) -> Result<(), TerminalInterpretError> {
                self.validate_mutable_referent(
                    execution,
                    self.descriptor_home().ok_or_else(malformed)?,
                )
            }

            pub(crate) fn validate_mutable_referent(
                &self,
                execution: &TerminalExecution,
                value: &TerminalStructuralValue,
            ) -> Result<(), TerminalInterpretError> {
                let structural_types = execution.structural_types();
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
                        Some((StructuralPathSegment::FixedRange { start, end }, backing)) => (
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
                        && byte_arrays::byte_array_length(&structural_types, *array_type)
                            .is_some_and(|extent| {
                                offset.checked_add(*length).is_some_and(|end| end <= extent)
                            })
                    {
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
                    let exact_home = match home {
                        ByteOwnerHome::Frame { .. } => referent.path.is_empty(),
                        ByteOwnerHome::Storage(home) => {
                            *home == crate::boundary::values::StructuralRuntimePlace::from(referent)
                                && execution.actual_storage_type_from_owner(home)?
                                    == referent.structural_type
                        }
                    };
                    return if value == descriptor_home.as_ref().unwrap_or(referent)
                        && exact_home
                        && structural_types.get(&referent.structural_type).is_some_and(
                            |declaration| {
                                declaration.shape
                                    == StructuralTypeShape::ByteSequence(
                                        ByteSequenceCarrier::BoundedOwned {
                                            capacity: *capacity,
                                        },
                                    )
                            },
                        ) {
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
                let Some((StructuralPathSegment::Field(identity), prefix)) =
                    referent.path.split_last()
                else {
                    return Err(malformed());
                };
                if value != descriptor_home.as_ref().unwrap_or(referent)
                    || field.parent.opaque_identity != referent.opaque_identity
                    || field.parent.path != prefix
                    || (!referent.qualifications.is_empty()
                        && !structural_types.get(&referent.structural_type).is_some_and(
                            |declaration| declaration.shape.is_bounded_owned_byte_sequence(),
                        ))
                {
                    return Err(malformed());
                }
                let case = match field.namespace {
                    crate::boundary::values::StructuralFieldNamespace::Member => None,
                    crate::boundary::values::StructuralFieldNamespace::CasePayload => {
                        Some(execution.structural_cases.require(&field.parent)?.case)
                    }
                };
                let (fields, _) =
                    execution.active_field_declarations(*parent_type, &field.parent, case)?;
                if !fields.iter().any(|declaration| {
                    declaration.id == field.field
                        && declaration.identity == *identity
                        && !declaration.relevance.is_erased()
                        && declaration.field_type.byte_sequence_carrier()
                            == Some(ByteSequenceCarrier::BoundedOwned {
                                capacity: *capacity,
                            })
                }) {
                    return Err(malformed());
                }
                Ok(())
            }
        }

        impl TerminalExecution<'_> {
            pub(crate) fn byte_binding_qualifications_hold(
                &self,
                binding: &ByteSequenceBinding,
                structural_type: StructuralTypeId,
                qualifications: &[semantic_vocabulary::StructuralDomainId],
            ) -> Result<bool, TerminalInterpretError> {
                if qualifications.is_empty() {
                    return Ok(true);
                }
                let bytes = binding.snapshot(self)?;
                Ok(terminal_semantics::byte_literal_qualifications_hold_in(
                    self.program.catalogs.structural_domains.iter(),
                    structural_type,
                    qualifications,
                    bytes.bytes(),
                ))
            }

            pub(crate) fn byte_field_read(
                &self,
                field: &StructuralByteSequenceRuntimeField,
            ) -> Result<super::read::TerminalByteRead<'_>, TerminalInterpretError> {
                use super::read::TerminalByteRead;
                let binding = self.structural_byte_sequence_fields.require(field)?;
                if let ByteSequenceBinding::Immutable(bytes) = binding {
                    return Ok(TerminalByteRead::contiguous(bytes.bytes()));
                }
                if let ByteSequenceBinding::Representation(_) = binding {
                    return binding.read_view(self);
                }
                binding.validate_presented_referent(self)?;
                match binding {
                    ByteSequenceBinding::MutableArray {
                        array,
                        offset,
                        length,
                        ..
                    } => self.stored_byte_array_read_view(
                        array,
                        usize::try_from(*offset).map_err(|_| malformed())?,
                        usize::try_from(offset.checked_add(*length).ok_or_else(malformed)?)
                            .map_err(|_| malformed())?,
                    ),
                    ByteSequenceBinding::MutableField { field, .. } => {
                        let bytes = self
                            .structural_byte_sequence_fields
                            .require(field)?
                            .immutable()?;
                        Ok(TerminalByteRead::contiguous(bytes.bytes()))
                    }
                    ByteSequenceBinding::MutableOwner { home, .. } => Ok(
                        TerminalByteRead::contiguous(self.byte_owner_bytes(home)?.bytes()),
                    ),
                    ByteSequenceBinding::Immutable(_) | ByteSequenceBinding::Representation(_) => {
                        Err(malformed())
                    }
                }
            }

            pub(crate) fn byte_field_length(
                &self,
                field: &StructuralByteSequenceRuntimeField,
            ) -> Result<u64, TerminalInterpretError> {
                let binding = self.structural_byte_sequence_fields.require(field)?;
                match binding {
                    ByteSequenceBinding::Immutable(bytes) => {
                        u64::try_from(bytes.len()).map_err(|_| malformed())
                    }
                    _ => self.byte_descriptor_length(
                        binding,
                        binding.descriptor_home().ok_or_else(malformed)?,
                    ),
                }
            }

            pub(crate) fn byte_field_snapshot(
                &self,
                field: &StructuralByteSequenceRuntimeField,
            ) -> Result<ByteSequenceView, TerminalInterpretError> {
                let binding = self.structural_byte_sequence_fields.require(field)?;
                if let ByteSequenceBinding::Immutable(bytes) = binding {
                    return Ok(bytes.clone());
                }
                self.reborrow_byte_sequence_binding(
                    binding,
                    binding.descriptor_home().ok_or_else(malformed)?,
                    StructuralAccess::SharedBorrow,
                )?
                .snapshot(self)
            }

            pub(crate) fn declared_byte_owner(
                &self,
                argument: &StructuralArgument,
            ) -> Result<Option<TerminalStructuralValue>, TerminalInterpretError> {
                let root = self.structural_value(argument.place)?;
                let types = self.structural_types();
                if crate::engine::custody::inline_structural_field(
                    &types,
                    root.structural_type,
                    &argument.path,
                )
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

            pub(crate) fn byte_descriptor_binding(
                &self,
                argument: &StructuralArgument,
                selected: &TerminalStructuralValue,
            ) -> Result<ByteSequenceBinding, TerminalInterpretError> {
                if let Some(source) = self.byte_descriptor_source(argument, selected)? {
                    return match source {
                        ByteDescriptorSource::Stored(binding) => Ok(binding),
                        ByteDescriptorSource::CapturedReferent(binding) => {
                            let access = match argument.access {
                                StructuralAccess::Owned => StructuralAccess::SharedBorrow,
                                access => access,
                            };
                            self.reborrow_byte_sequence_binding(&binding, selected, access)
                        }
                    };
                }
                if let Some(bytes) = self.structural_byte_storage.get(
                    &crate::boundary::values::StructuralRuntimePlace::from(selected),
                ) {
                    return Ok(ByteSequenceBinding::Immutable(
                        bytes
                            .copy_snapshot()?
                            .with_descriptor_home(selected.clone()),
                    ));
                }
                let home = crate::boundary::values::StructuralRuntimePlace::from(selected);
                if self.initialized_empty_byte_array(&home)? {
                    let storage =
                crate::boundary::structural_inputs::byte_arrays::storage::ArrayStorage::from(home);
                    let bytes = self.stored_byte_array_snapshot(&storage, 0, 0)?;
                    return Ok(ByteSequenceBinding::Immutable(
                        bytes.with_descriptor_home(selected.clone()),
                    ));
                }
                Err(malformed())
            }

            pub(crate) fn retained_byte_descriptor_binding(
                &self,
                argument: &StructuralArgument,
                selected: &TerminalStructuralValue,
            ) -> Result<Option<ByteSequenceBinding>, TerminalInterpretError> {
                Ok(self
                    .byte_descriptor_source(argument, selected)?
                    .map(|source| match source {
                        ByteDescriptorSource::Stored(binding)
                        | ByteDescriptorSource::CapturedReferent(binding) => binding,
                    }))
            }

            fn byte_descriptor_source(
                &self,
                argument: &StructuralArgument,
                selected: &TerminalStructuralValue,
            ) -> Result<Option<ByteDescriptorSource>, TerminalInterpretError> {
                if argument.access == StructuralAccess::WriteOnlyBorrow
                    || !self
                        .structural_types()
                        .get(&selected.structural_type)
                        .is_some_and(|row| {
                            row.shape.is_borrowed_byte_carrier()
                                || row.shape.is_bounded_owned_byte_sequence()
                        })
                {
                    return Err(malformed());
                }
                let capture = |binding: &ByteSequenceBinding| match binding {
                    ByteSequenceBinding::Immutable(bytes) => ByteSequenceBinding::Immutable(
                        bytes.clone().with_descriptor_home(selected.clone()),
                    ),
                    ByteSequenceBinding::Representation(view) => {
                        ByteSequenceBinding::Representation(std::sync::Arc::new(
                            view.as_ref().clone().with_descriptor_home(selected.clone()),
                        ))
                    }
                    binding => binding.clone(),
                };
                let matches =
                    |binding: &ByteSequenceBinding, home: &TerminalStructuralValue| match binding {
                        ByteSequenceBinding::Immutable(bytes) => {
                            bytes.matches_bound_descriptor_home(home, selected)
                        }
                        ByteSequenceBinding::Representation(view) => {
                            view.matches_bound_descriptor_home(home, selected)
                        }
                        _ => {
                            home.opaque_identity == selected.opaque_identity
                                && home.path == selected.path
                                && home.structural_type == selected.structural_type
                                && binding.validate_mutable_referent(self, selected).is_ok()
                        }
                    };
                if let Some(binding) = self.activation_storage.claim_descriptors.get(
                    &crate::boundary::values::StructuralRuntimePlace::from(selected),
                ) {
                    if !self.is_live_activation_descriptor(binding, selected) {
                        return Err(malformed());
                    }
                    return Ok(Some(ByteDescriptorSource::Stored(binding.clone())));
                }
                if let Some(binding) = self.structural_byte_descriptors.get(
                    &crate::boundary::values::StructuralRuntimePlace::from(selected),
                ) {
                    let home = match binding {
                        ByteSequenceBinding::Immutable(bytes) => bytes.descriptor_home(),
                        binding => binding.descriptor_home(),
                    }
                    .ok_or_else(malformed)?;
                    return matches(binding, home)
                        .then(|| capture(binding))
                        .map(ByteDescriptorSource::Stored)
                        .map(Some)
                        .ok_or_else(malformed);
                }
                if argument.path.is_empty()
                    && let Some(binding) = self.byte_sequence_values.get(&argument.place)
                {
                    return self
                        .structural_values
                        .get(&argument.place)
                        .filter(|home| matches(binding, home))
                        .map(|_| capture(binding))
                        .map(ByteDescriptorSource::Stored)
                        .map(Some)
                        .ok_or_else(malformed);
                }
                if let Some(binding) = self.captured_byte_owner_binding(selected) {
                    if matches!(
                        binding,
                        ByteSequenceBinding::Immutable(_) | ByteSequenceBinding::Representation(_)
                    ) {
                        if !binding.matches_shared_home(selected) {
                            return Err(malformed());
                        }
                    } else {
                        binding.validate_mutable_referent(self, selected)?;
                    }
                    return Ok(Some(ByteDescriptorSource::CapturedReferent(
                        binding.clone(),
                    )));
                }
                if let Some((StructuralPathSegment::Field(identity), prefix)) =
                    argument.path.split_last()
                {
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
                        &crate::boundary::values::StructuralRuntimePlace::from(&parent),
                        case,
                    )?;
                    let field = live_field_by_identity(fields, *identity).ok_or_else(malformed)?;
                    if field.field_type.byte_sequence_carrier().is_some() {
                        let (cell, _) =
                            self.readable_byte_field(argument.place, prefix, field.id)?;
                        let bytes = self.structural_byte_sequence_fields.require(&cell)?;
                        return Ok(Some(ByteDescriptorSource::Stored(
                            bytes.clone().with_descriptor_home(selected.clone()),
                        )));
                    }
                }
                let home = crate::boundary::values::StructuralRuntimePlace::from(selected);
                if self.structural_byte_storage.contains_key(&home)
                    || self.initialized_empty_byte_array(&home)?
                {
                    return Ok(None);
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
                    .map(ByteDescriptorSource::Stored)
                    .map(Some)
                    .ok_or_else(malformed)
            }

            pub(crate) fn byte_descriptor_length(
                &self,
                binding: &ByteSequenceBinding,
                selected: &TerminalStructuralValue,
            ) -> Result<u64, TerminalInterpretError> {
                let length = match binding {
                    ByteSequenceBinding::Representation(view) => {
                        if !view.matches_descriptor_home(selected) {
                            return Err(malformed());
                        }
                        return u64::try_from(view.len()).map_err(|_| malformed());
                    }
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

            pub(crate) fn shared_byte_descriptor_binding(
                &self,
                argument: &StructuralArgument,
                value: &TerminalStructuralValue,
            ) -> Result<ByteSequenceBinding, TerminalInterpretError> {
                if !matches!(
                    argument.access,
                    StructuralAccess::Owned | StructuralAccess::SharedBorrow
                ) || !self
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
                if !binding.matches_shared_home(&selected) {
                    return Err(malformed());
                }
                self.reborrow_byte_sequence_binding(
                    &binding,
                    &selected,
                    StructuralAccess::SharedBorrow,
                )
            }

            pub(crate) fn prepare_owned_byte_descriptor_argument(
                &self,
                parameter: &StructuralParameterDeclaration,
                argument: &StructuralArgument,
                resolved: &TerminalStructuralValue,
            ) -> Result<
                Option<(TerminalStructuralValue, ByteSequenceBinding)>,
                TerminalInterpretError,
            > {
                let Some(StructuralTypeShape::ByteSequence(carrier)) = self
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
                    ByteSequenceCarrier::BorrowedView { access } => (access, None),
                    ByteSequenceCarrier::BoundedView { access, capacity } => {
                        (access, Some(capacity))
                    }
                    _ => return Ok(None),
                };
                let selected =
                    self.resolve_live_projection(argument.place, &argument.path, argument.access)?;
                if self.present_owned_descriptor(
                    selected.clone(),
                    parameter.structural_type,
                    parameter.multiplicity,
                )? != *resolved
                {
                    return Err(malformed());
                }
                let owned_source = self.owned_source_for(
                    argument.place,
                    selected.structural_type,
                    parameter.multiplicity,
                );
                let binding = self.byte_descriptor_binding(argument, &selected)?;
                let activation_claim = parameter.multiplicity == StructuralMultiplicity::Linear
                    && self.is_live_activation_descriptor(&binding, &selected);
                if activation_claim {
                    let claims = crate::engine::custody::projected_ownership_claims(
                        &self.live_claims,
                        argument,
                    )?;
                    if claims.len() != 1 || !claims[0].1.path.is_empty() {
                        return Err(malformed());
                    }
                }
                if argument.access != StructuralAccess::Owned
                    || (!argument.path.is_empty() && !activation_claim)
                    || parameter.structural_type != resolved.structural_type
                    || !parameter.projected_qualifications.is_empty()
                    || !resolved.projected_qualifications.is_empty()
                {
                    return Err(malformed());
                }
                let permission_matches = match &binding {
                    ByteSequenceBinding::Representation(_) => {
                        self.validate_descriptor_borrow(argument, StructuralAccess::SharedBorrow)?;
                        parameter.multiplicity == StructuralMultiplicity::Unrestricted
                            && matches!(access, None | Some(StructuralAccess::SharedBorrow))
                            && self.byte_binding_qualifications_hold(
                                &binding,
                                parameter.structural_type,
                                &parameter.qualifications,
                            )?
                    }
                    ByteSequenceBinding::Immutable(bytes) => {
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
                        (parameter.multiplicity == StructuralMultiplicity::Affine
                            || activation_claim)
                            && matches!(access, None | Some(StructuralAccess::MutableBorrow))
                            && (owned_source || activation_claim)
                            && parameter.qualifications.is_empty()
                            && resolved.qualifications.is_empty()
                    }
                };
                let length = self.byte_descriptor_length(&binding, &selected)?;
                if !permission_matches || capacity.is_some_and(|capacity| length != capacity) {
                    return Err(malformed());
                }
                Ok(Some((
                    resolved.clone(),
                    binding.with_descriptor_home(resolved.clone()),
                )))
            }

            pub(crate) fn prepare_byte_descriptor_argument(
                &self,
                parameter: &StructuralParameterDeclaration,
                argument: &StructuralArgument,
            ) -> Result<
                Option<(TerminalStructuralValue, ByteSequenceBinding)>,
                TerminalInterpretError,
            > {
                if borrowed_storage_parameter(parameter, self.structural_types().values())
                    && borrowed_storage_reference_ceiling(
                        parameter,
                        self.structural_types().values(),
                    )
                    .is_none()
                {
                    return Ok(None);
                }
                if parameter.access == StructuralAccess::Owned {
                    return Ok(None);
                }
                if !self.structural_values.contains_key(&argument.place) {
                    return Ok(None);
                }
                let Ok(value) =
                    self.resolve_live_projection(argument.place, &argument.path, argument.access)
                else {
                    return Ok(None);
                };
                if !self
                    .structural_types()
                    .get(&value.structural_type)
                    .is_some_and(|row| row.shape.is_borrowed_byte_carrier())
                {
                    return Ok(None);
                }
                let storage_ceiling =
                    borrowed_storage_reference_ceiling(parameter, self.structural_types().values());
                let claimed_elsewhere =
                    crate::engine::custody::place_is_claimed(&self.live_claims, argument.place)
                        && !self
                            .byte_descriptor_binding(argument, &value)
                            .is_ok_and(|binding| {
                                self.is_live_activation_descriptor(&binding, &value)
                            });
                if !(terminal_semantics::view_descriptor_parameter(
                    parameter,
                    self.structural_types().values(),
                ) || storage_ceiling.is_some())
                    || argument.access != parameter.access
                    || !parameter.projected_qualifications.is_empty()
                    || claimed_elsewhere
                {
                    return Err(malformed());
                }
                self.validate_descriptor_borrow(argument, parameter.access)
                    .map_err(|error| {
                        error.in_argument_preparation("byte descriptor source permission")
                    })?;
                let binding = self
                    .byte_descriptor_binding(argument, &value)
                    .map_err(|error| {
                        error.in_argument_preparation("byte descriptor backing relation")
                    })?;
                let requested_access = storage_ceiling.unwrap_or(parameter.access);
                if !terminal_semantics::descriptor_reborrow_matches(
                    self.structural_types().values(),
                    value.structural_type,
                    parameter.access,
                    Some(binding.access()),
                    parameter.structural_type,
                    requested_access,
                ) {
                    return Err(TerminalInterpretError::ReferenceFormation {
                        stage: "byte descriptor geometry",
                        detail: format!(
                            "source={:?} target={parameter:?} argument={argument:?} binding-access={:?}",
                            self.structural_types().get(&value.structural_type),
                            binding.access()
                        ),
                    });
                }
                let binding = if requested_access == StructuralAccess::WriteOnlyBorrow {
                    binding.clone()
                } else {
                    self.reborrow_byte_sequence_binding(&binding, &value, requested_access)
                        .map_err(|error| {
                            error.in_argument_preparation("byte descriptor reborrow")
                        })?
                };
                if matches!(
                    requested_access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                ) {
                    if !parameter.qualifications.is_empty() || !value.qualifications.is_empty() {
                        return Err(malformed());
                    }
                } else if !self.byte_binding_qualifications_hold(
                    &binding,
                    parameter.structural_type,
                    &parameter.qualifications,
                )? {
                    return Err(malformed());
                }
                let mut value = value;
                value.structural_type = parameter.structural_type;
                value.absorb_qualifications(&parameter.qualifications);
                let binding = binding.with_descriptor_home(value.clone());
                Ok(Some((value, binding)))
            }

            pub(crate) fn reborrow_byte_sequence_binding(
                &self,
                binding: &ByteSequenceBinding,
                value: &TerminalStructuralValue,
                access: StructuralAccess,
            ) -> Result<ByteSequenceBinding, TerminalInterpretError> {
                if !matches!(access, terminal_psi::reading_borrow_pair_pattern!()) {
                    return Err(malformed());
                }
                if matches!(
                    binding,
                    ByteSequenceBinding::Immutable(_) | ByteSequenceBinding::Representation(_)
                ) {
                    return if access == StructuralAccess::SharedBorrow
                        && binding.matches_shared_home(value)
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
                        array_type,
                        offset,
                        length,
                        ..
                    } => self
                        .stored_byte_array_snapshot(
                            array,
                            usize::try_from(*offset).map_err(|_| malformed())?,
                            usize::try_from(offset.checked_add(*length).ok_or_else(malformed)?)
                                .map_err(|_| malformed())?,
                        )?
                        .with_original_array(
                            crate::storage::byte_sequences::OriginalByteArrayWindow {
                                array: array.clone(),
                                array_type: *array_type,
                                offset: usize::try_from(*offset).map_err(|_| malformed())?,
                                length: usize::try_from(*length).map_err(|_| malformed())?,
                            },
                        ),
                    ByteSequenceBinding::MutableField { field, .. } => self
                        .structural_byte_sequence_fields
                        .require(field)?
                        .immutable()?
                        .clone(),
                    ByteSequenceBinding::MutableOwner { home, .. } => {
                        self.byte_owner_bytes(home)?.clone()
                    }
                    ByteSequenceBinding::Immutable(_) | ByteSequenceBinding::Representation(_) => {
                        unreachable!()
                    }
                };
                Ok(ByteSequenceBinding::Immutable(
                    snapshot.with_descriptor_home(value.clone()),
                ))
            }

            pub(crate) fn exclusive_owner_bytes(
                &self,
                binding: &ByteSequenceBinding,
            ) -> Result<(&ByteSequenceView, u64), TerminalInterpretError> {
                let (bytes, capacity) = match binding {
                    ByteSequenceBinding::MutableField {
                        field, capacity, ..
                    } => (
                        self.structural_byte_sequence_fields
                            .require(field)?
                            .immutable()?,
                        *capacity,
                    ),
                    ByteSequenceBinding::MutableOwner { home, capacity, .. } => {
                        (self.byte_owner_bytes(home)?, *capacity)
                    }
                    ByteSequenceBinding::Immutable(_)
                    | ByteSequenceBinding::Representation(_)
                    | ByteSequenceBinding::MutableArray { .. } => {
                        return Err(malformed());
                    }
                };
                if bytes.len() as u128 > u128::from(capacity) {
                    return Err(malformed());
                }
                Ok((bytes, capacity))
            }

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
                        return self
                    .structural_byte_storage
                    .get(home)
                    .and_then(|owner| owner.local_view())
                    .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                        "bounded byte owner requires its byte-sequence storage representation",
                    ));
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
                        if !self.structural_byte_storage.contains_key(home) {
                            return Err(malformed());
                        }
                        self.invalidate_stored_provenance(home);
                        return self.structural_byte_storage.get_mut(home).and_then(|owner| owner.local_view_mut())
                    .ok_or(TerminalInterpretError::UnsupportedSemanticVariant("bounded byte owner requires its mutable byte-sequence storage representation"));
                    }
                };
                let (machine, values, structural_values) = if *depth == self.call_stack.len() {
                    (
                        self.current_machine,
                        &self.byte_sequence_values,
                        &self.structural_values,
                    )
                } else {
                    let frame = self.call_stack.get(*depth).ok_or_else(malformed)?;
                    (
                        frame.current_machine,
                        &frame.byte_sequence_values,
                        &frame.structural_values,
                    )
                };
                if machine != *expected_machine
                    || !matches!(values.get(place), Some(ByteSequenceBinding::Immutable(_)))
                {
                    return Err(malformed());
                }
                let changed = structural_values
                    .get(place)
                    .map(crate::boundary::values::StructuralRuntimePlace::from);
                if let Some(changed) = changed {
                    self.invalidate_stored_provenance(&changed);
                }
                let values = frame_field_mut!(self, *depth, byte_sequence_values);
                match values.get_mut(place) {
                    Some(ByteSequenceBinding::Immutable(bytes)) => Ok(bytes),
                    _ => Err(malformed()),
                }
            }
        }
    }
    pub(crate) mod concatenation {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::TerminalStructuralValue;
        use crate::engine::execution::{OperationFlow, TerminalExecution, operation_kind};
        use crate::storage::byte_sequences::{ByteSequenceView, binding::ByteSequenceBinding};
        use terminal_psi::{ByteSequenceCarrier, Operation, OperationKind, StructuralTypeShape};

        impl TerminalExecution<'_> {
            pub(crate) fn execute_byte_sequence_concatenation(
                &mut self,
                operation: &Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                operation_kind!(operation => EstablishByteSequenceConcatenation { left, right, .. });
                let result = operation.result.structural().ok_or_else(malformed)?;
                let StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                    capacity,
                }) = self.structural_declaration(result.structural_type)?.shape
                else {
                    return Err(malformed());
                };
                let left = self.byte_sequence_operand_view(left)?;
                let right = self.byte_sequence_operand_view(right)?;
                let length = left.len().checked_add(right.len()).ok_or_else(malformed)?;
                if u64::try_from(length).map_err(|_| malformed())? > capacity {
                    return Err(malformed());
                }
                let mut bytes =
                    super::storage::ByteSequenceStorage::zeroed(length, self.account.as_ref())?;
                bytes.bytes_mut()[..left.len()].copy_from_slice(left.bytes());
                bytes.bytes_mut()[left.len()..].copy_from_slice(right.bytes());
                let value = TerminalStructuralValue::minted(
                    self.local_structural_identities.allocate()?,
                    result.structural_type,
                    result.qualifications.clone(),
                );
                self.byte_sequence_values.insert(
                    result.place,
                    ByteSequenceBinding::Immutable(ByteSequenceView::new(bytes)),
                )?;
                self.structural_values.insert(result.place, value)?;
                Ok(OperationFlow::Advance)
            }
        }
    }
    pub(crate) mod equality {
        use crate::boundary::errors::TerminalInterpretError;
        use crate::boundary::values::TerminalScalarValue;
        use crate::engine::execution::{OperationFlow, TerminalExecution, operation_kind};
        use terminal_psi::{Operation, OperationKind};

        impl TerminalExecution<'_> {
            pub(crate) fn execute_byte_sequence_equal(
                &mut self,
                operation: &Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                operation_kind!(operation => ByteSequenceEqual { left, right });
                self.require_boolean_result(operation)?;
                let left = self.byte_sequence_operand_view(left)?;
                let right = self.byte_sequence_operand_view(right)?;
                let verdict = left.bytes() == right.bytes();
                self.store_scalar(operation, TerminalScalarValue::Boolean(verdict))
            }
        }
    }
    pub(crate) mod field_index_store {
        use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
        use terminal_psi::{Operation, OperationKind, OperationResult};

        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::TerminalScalarValue;
        use crate::engine::execution::{TerminalExecution, operation_kind};

        impl TerminalExecution<'_> {
            pub(crate) fn execute_structural_byte_sequence_field_length(
                &mut self,
                operation: &Operation,
            ) -> Result<(), TerminalInterpretError> {
                operation_kind!(operation => StructuralByteSequenceFieldLength { source, path, field });
                let Some(result) = operation.result.as_scalar() else {
                    return Err(malformed());
                };
                let count_type =
                    IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
                if result.scalar_type != ScalarType::Integer(count_type) {
                    return Err(malformed());
                }
                let (source_field, capacity) =
                    self.byte_field_length_home(*source, path, *field)?;
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
                )?;
                Ok(())
            }

            pub(crate) fn execute_structural_byte_sequence_field_byte_store(
                &mut self,
                operation: &Operation,
            ) -> Result<(), TerminalInterpretError> {
                operation_kind!(operation => StructuralByteSequenceFieldByteStore {
                    destination, path, field, index, value, length, ..
                });
                if operation.result != OperationResult::Unit {
                    return Err(malformed());
                }
                let (destination_field, capacity) =
                    self.resolve_structural_byte_sequence_field(*destination, path, *field, true)?;
                let count_type =
                    IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
                let byte_type =
                    IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?;
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
                bytes.replace_byte(byte_index, byte)?;
                self.invalidate_stored_provenance(&destination_field.parent);
                Ok(())
            }

            pub(crate) fn execute_structural_byte_sequence_field_read(
                &mut self,
                operation: &Operation,
            ) -> Result<(), TerminalInterpretError> {
                operation_kind!(operation => StructuralByteSequenceFieldRead {
                    source, path, field, index, length, ..
                });
                let Some(result) = operation.result.as_scalar() else {
                    return Err(malformed());
                };
                let byte_type =
                    IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?;
                if result.scalar_type != ScalarType::Integer(byte_type) {
                    return Err(malformed());
                }
                let (source_field, capacity) = self.readable_byte_field(*source, path, *field)?;
                let count_type =
                    IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
                let count = |operand| self.unsigned_integer_value(operand, count_type);
                let byte_index = count(*index)?;
                let live_length = count(*length)?;
                if capacity.is_some_and(|capacity| live_length > capacity)
                    || byte_index >= live_length
                {
                    return Err(malformed());
                }
                let bytes = self.byte_field_read(&source_field)?;
                if u64::try_from(bytes.len()).ok() != Some(live_length) {
                    return Err(malformed());
                }
                let byte_index = usize::try_from(byte_index).map_err(|_| malformed())?;
                let byte = bytes.read(byte_index)?;
                self.values.insert(
                    result.id,
                    TerminalScalarValue::Integer {
                        scalar_type: byte_type,
                        value: IntegerValue::Unsigned(u128::from(byte)),
                    },
                )?;
                Ok(())
            }
        }
    }
    pub(crate) mod field_store {
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, PlaceId, StructuralFieldId,
        };
        use terminal_psi::{
            ByteSequenceCarrier, Operation, OperationKind, OperationResult, StructuralAccess,
            StructuralPathSegment, StructuralTypeShape, live_field_by_id,
        };

        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::{
            StructuralByteSequenceRuntimeField, StructuralRuntimePlace, TerminalScalarValue,
        };
        use crate::engine::custody::{literal_fixed_array, project_structural_value};
        use crate::engine::execution::{TerminalExecution, operation_kind};
        use crate::storage::byte_sequences::binding::ByteSequenceBinding;

        impl TerminalExecution<'_> {
            pub(crate) fn byte_field_length_home(
                &self,
                root: PlaceId,
                path: &[StructuralPathSegment],
                field: StructuralFieldId,
            ) -> Result<(StructuralByteSequenceRuntimeField, Option<u64>), TerminalInterpretError>
            {
                let (cell, carrier) =
                    self.resolve_byte_field_carrier(root, path, field, false, true)?;
                let capacity = match carrier {
                    ByteSequenceCarrier::BorrowedView { .. } => None,
                    ByteSequenceCarrier::BoundedView { capacity, .. }
                    | ByteSequenceCarrier::BoundedOwned { capacity } => Some(capacity),
                };
                Ok((cell, capacity))
            }

            pub(crate) fn execute_structural_byte_sequence_field_store(
                &mut self,
                operation: &Operation,
            ) -> Result<(), TerminalInterpretError> {
                operation_kind!(operation => StructuralByteSequenceFieldStore {
                    destination, path, field, source, length, ..
                });
                if operation.result != OperationResult::Unit || destination == source {
                    return Err(malformed());
                }
                let (destination_field, capacity) =
                    self.resolve_structural_byte_sequence_field(*destination, path, *field, true)?;
                if crate::engine::custody::place_is_claimed(&self.live_claims, *source) {
                    return Err(malformed());
                }
                let TerminalScalarValue::Integer {
                    scalar_type,
                    value: IntegerValue::Unsigned(length),
                } = self.values.get(length).ok_or_else(malformed)?
                else {
                    return Err(malformed());
                };
                if *scalar_type
                    != IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?
                    || *length > u128::from(capacity)
                {
                    return Err(malformed());
                }
                let source_value = self.structural_value(*source)?;
                if !source_value.projected_qualifications.is_empty()
                    || !matches!(self.structural_types().get(&source_value.structural_type), Some(declaration)
                            if declaration.shape.is_borrowed_byte_view()
                                || declaration.shape.is_bounded_owned_byte_sequence())
                {
                    return Err(malformed());
                }
                let binding = self.byte_sequence_values.require(source)?;
                let bytes = binding.snapshot(self)?;
                if !binding.matches_shared_home(source_value) || bytes.len() as u128 != *length {
                    return Err(malformed());
                }
                let bytes = bytes.clone();
                self.structural_byte_sequence_fields.insert(
                    destination_field.clone(),
                    ByteSequenceBinding::Immutable(bytes),
                )?;
                self.invalidate_stored_provenance(&destination_field.parent);
                Ok(())
            }

            pub(crate) fn resolve_structural_byte_sequence_field(
                &self,
                root: PlaceId,
                path: &[StructuralPathSegment],
                field: StructuralFieldId,
                requires_mutation: bool,
            ) -> Result<(StructuralByteSequenceRuntimeField, u64), TerminalInterpretError>
            {
                let (cell, carrier) = self.resolve_structural_byte_field_carrier(
                    root,
                    path,
                    field,
                    requires_mutation,
                )?;
                let ByteSequenceCarrier::BoundedOwned { capacity } = carrier else {
                    return Err(malformed());
                };
                Ok((cell, capacity))
            }

            pub(crate) fn readable_byte_field(
                &self,
                root: semantic_vocabulary::PlaceId,
                path: &[terminal_psi::StructuralPathSegment],
                field: semantic_vocabulary::StructuralFieldId,
            ) -> Result<(StructuralByteSequenceRuntimeField, Option<u64>), TerminalInterpretError>
            {
                let (cell, carrier) =
                    self.resolve_structural_byte_field_carrier(root, path, field, false)?;
                match carrier {
                    ByteSequenceCarrier::BoundedOwned { capacity } => Ok((cell, Some(capacity))),
                    ByteSequenceCarrier::BorrowedView {
                        access:
                            None
                            | Some(StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow),
                    } => Ok((cell, None)),
                    ByteSequenceCarrier::BoundedView { access, capacity }
                        if access != Some(StructuralAccess::WriteOnlyBorrow) =>
                    {
                        Ok((cell, Some(capacity)))
                    }
                    _ => Err(malformed()),
                }
            }

            pub(crate) fn execute_structural_byte_view_field_store(
                &mut self,
                operation: &Operation,
            ) -> Result<(), TerminalInterpretError> {
                operation_kind!(operation => StructuralByteViewFieldStore {
                    destination, path, field, source, ..
                });
                if operation.result != OperationResult::Unit || destination == source {
                    return Err(malformed());
                }
                let shared_view = ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::SharedBorrow),
                };
                let destination_field = match field {
                    Some(field) => {
                        let (destination_field, carrier) = self
                            .resolve_structural_byte_field_carrier(
                                *destination,
                                path,
                                *field,
                                true,
                            )?;
                        if carrier != shared_view {
                            return Err(malformed());
                        }
                        Some(destination_field)
                    }
                    None => {
                        let home = self.structural_value(*destination)?;
                        if !path.is_empty()
                            || !home.path.is_empty()
                            || !matches!(self.structural_types().get(&home.structural_type),
                    Some(declaration) if declaration.shape.is_shared_readable_byte_view())
                        {
                            return Err(malformed());
                        }
                        None
                    }
                };
                let source_value = self.structural_value(*source)?;
                if !source_value.path.is_empty()
                    || !matches!(self.structural_types().get(&source_value.structural_type),
                Some(declaration) if declaration.shape.is_borrowed_byte_view())
                {
                    return Err(malformed());
                }
                let view = self.byte_sequence_values.require(source)?.clone();
                if !view.matches_shared_home(source_value) {
                    return Err(malformed());
                }
                match destination_field {
                    Some(destination_field) => {
                        self.structural_byte_sequence_fields
                            .insert(destination_field, view)?;
                    }
                    None => {
                        let home = self.structural_value(*destination)?;
                        self.byte_sequence_values
                            .insert(*destination, view.with_descriptor_home(home.clone()))?;
                    }
                }
                Ok(())
            }

            pub(crate) fn resolve_structural_byte_field_carrier(
                &self,
                root: PlaceId,
                path: &[StructuralPathSegment],
                field: StructuralFieldId,
                requires_mutation: bool,
            ) -> Result<
                (StructuralByteSequenceRuntimeField, ByteSequenceCarrier),
                TerminalInterpretError,
            > {
                self.resolve_byte_field_carrier(root, path, field, requires_mutation, false)
            }

            fn resolve_byte_field_carrier(
                &self,
                root: PlaceId,
                path: &[StructuralPathSegment],
                field: StructuralFieldId,
                requires_mutation: bool,
                length_metadata: bool,
            ) -> Result<
                (StructuralByteSequenceRuntimeField, ByteSequenceCarrier),
                TerminalInterpretError,
            > {
                let machine = self.program.machine(self.current_machine)?;
                if crate::engine::custody::place_is_claimed(&self.live_claims, root) {
                    return Err(malformed());
                }
                let (access, parameter_root) =
                    if let Some(parameter) = machine.structural_parameter(root) {
                        (parameter.access, true)
                    } else {
                        let declaration = machine.structural_place(root).ok_or_else(malformed)?;
                        let semantic_vocabulary::StructuralPlaceKind::OperationResult {
                            structural_type,
                            ..
                        } = declaration.kind
                        else {
                            return Err(malformed());
                        };
                        if !self
                            .structural_values
                            .get(&root)
                            .is_some_and(|value| value.structural_type == structural_type)
                        {
                            return Err(malformed());
                        }
                        (StructuralAccess::Owned, false)
                    };
                if length_metadata
                    && access == StructuralAccess::WriteOnlyBorrow
                    && path.contains(&StructuralPathSegment::Referent)
                {
                    return Err(malformed());
                }
                let (carrier_path, selected_case) = match path.split_last() {
                    Some((StructuralPathSegment::Case(case), prefix)) => (prefix, Some(*case)),
                    _ => (path, None),
                };
                let parent = if let Some(view) = self.element_view_values.get(&root) {
                    if parameter_root
                        && requires_mutation
                        && access != StructuralAccess::MutableBorrow
                    {
                        return Err(malformed());
                    }
                    let descriptor_type = self.live_structural_type(root)?;
                    let StructuralTypeShape::ElementView { element, .. } =
                        self.structural_declaration(descriptor_type)?.shape
                    else {
                        return Err(malformed());
                    };
                    let (collection, selected_path) = view.structural_projection(
                        &self.structural_types(),
                        carrier_path,
                        requires_mutation,
                    )?;
                    let (backing_element, _) =
                        literal_fixed_array(&self.structural_types(), collection.structural_type)?;
                    if element != backing_element {
                        return Err(malformed());
                    }
                    project_structural_value(&self.structural_types(), &collection, &selected_path)?
                } else {
                    let requested = if requires_mutation
                        || (length_metadata && access == StructuralAccess::WriteOnlyBorrow)
                    {
                        StructuralAccess::WriteOnlyBorrow
                    } else {
                        StructuralAccess::SharedBorrow
                    };
                    if !crate::storage::reference::access_covers(access, requested) {
                        return Err(malformed());
                    }
                    self.resolve_live_projection(root, carrier_path, requested)?
                };
                let (fields, namespace) = self.active_field_declarations(
                    parent.structural_type,
                    &StructuralRuntimePlace::from(&parent),
                    selected_case,
                )?;
                let field_declaration = live_field_by_id(fields, field).ok_or_else(malformed)?;
                let carrier = field_declaration
                    .field_type
                    .byte_sequence_carrier()
                    .ok_or_else(malformed)?;
                Ok((
                    StructuralByteSequenceRuntimeField {
                        parent: StructuralRuntimePlace::from(&parent),
                        namespace,
                        field,
                    },
                    carrier,
                ))
            }
        }
    }
    pub(crate) mod operands {
        use super::storage::ByteSequenceStorage;
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::structural_inputs::byte_arrays::storage::scalar_byte;
        use crate::boundary::values::TerminalScalarValue;
        use crate::engine::execution::TerminalExecution;
        use crate::storage::byte_sequences::ByteSequenceView;
        use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
        use terminal_psi::control_flow::ByteSequenceOperand;
        use terminal_psi::{StructuralAccess, StructuralTypeShape};

        impl TerminalExecution<'_> {
            pub(crate) fn fixed_byte_array_observation(
                &self,
                source: &terminal_psi::StructuralArgument,
            ) -> Result<Option<ByteSequenceView>, TerminalInterpretError> {
                let root = self.live_structural_type(source.place)?;
                let types = self.structural_types();
                let selected = terminal_semantics::runtime_structural_path_tip(
                    types.values(),
                    root,
                    &source.path,
                )
                .ok_or_else(malformed)?;
                if !matches!(selected.shape, StructuralTypeShape::FixedArray { .. })
                    || terminal_semantics::fixed_byte_extent(types.values(), selected.id).is_none()
                {
                    return Ok(None);
                }
                self.validate_descriptor_borrow(source, StructuralAccess::SharedBorrow)?;
                let snapshot = self.scalar_array_snapshot(source, selected.id)?;
                let observed = snapshot.elements();
                let values = observed.iter().map(scalar_byte);
                let bytes = ByteSequenceStorage::from_values(
                    snapshot.len(),
                    values,
                    self.account.as_ref(),
                )?;
                Ok(Some(ByteSequenceView::new(bytes)))
            }

            pub(crate) fn byte_sequence_operand_view(
                &self,
                operand: &ByteSequenceOperand,
            ) -> Result<ByteSequenceView, TerminalInterpretError> {
                let bytes = match operand {
                    ByteSequenceOperand::Whole { source, length } => {
                        if source.access != StructuralAccess::SharedBorrow {
                            return Err(malformed());
                        }
                        if let Some(bytes) = self.fixed_byte_array_observation(source)? {
                            let expected = IntegerType::new(IntegerSign::Unsigned, 64)
                                .map_err(|_| malformed())?;
                            if !matches!(self.values.get(length), Some(TerminalScalarValue::Integer { scalar_type, value: IntegerValue::Unsigned(count) })
                        if *scalar_type == expected && *count == bytes.len() as u128)
                            {
                                return Err(malformed());
                            }
                            return Ok(bytes);
                        }
                        self.validate_descriptor_borrow(source, StructuralAccess::SharedBorrow)?;
                        let selected = self.resolve_live_projection(
                            source.place,
                            &source.path,
                            StructuralAccess::SharedBorrow,
                        )?;
                        let binding = self.byte_descriptor_binding(source, &selected)?;
                        self.byte_descriptor_length(&binding, &selected)?;
                        self.reborrow_byte_sequence_binding(
                            &binding,
                            &selected,
                            StructuralAccess::SharedBorrow,
                        )?
                        .snapshot(self)?
                    }
                    ByteSequenceOperand::Field { source, field, .. } => {
                        let (key, capacity) =
                            self.readable_byte_field(source.place, &source.path, *field)?;
                        let bytes = self.byte_field_snapshot(&key)?;
                        if capacity
                            .is_some_and(|capacity| bytes.len() as u128 > u128::from(capacity))
                        {
                            return Err(malformed());
                        }
                        bytes.clone()
                    }
                };
                let expected =
                    IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
                if !matches!(self.values.get(&operand.length()), Some(TerminalScalarValue::Integer {
            scalar_type, value: IntegerValue::Unsigned(length),
        }) if *scalar_type == expected && *length == bytes.len() as u128)
                {
                    return Err(malformed());
                }
                Ok(bytes)
            }
        }
    }
    pub(crate) mod predicate {
        use crate::boundary::errors::TerminalInterpretError;
        use crate::boundary::values::TerminalScalarValue;
        use crate::engine::execution::{OperationFlow, TerminalExecution, operation_kind};
        use terminal_psi::{Operation, OperationKind};

        impl TerminalExecution<'_> {
            pub(crate) fn execute_byte_sequence_predicate(
                &mut self,
                operation: &Operation,
            ) -> Result<OperationFlow, TerminalInterpretError> {
                operation_kind!(operation => ByteSequencePredicate { operand, predicate });
                self.require_boolean_result(operation)?;
                let bytes = self.byte_sequence_operand_view(operand)?;
                let verdict = predicate.holds_for(bytes.bytes());
                self.store_scalar(operation, TerminalScalarValue::Boolean(verdict))
            }
        }
    }
    pub(crate) mod read {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::storage::scalar_storage::ScalarCell;

        #[derive(Clone, Copy)]
        enum ByteStorage<'storage> {
            Contiguous(&'storage [u8]),
            Cells(&'storage [ScalarCell]),
            Representation {
                reader: &'storage dyn RepresentationByteReader,
                view: &'storage crate::storage::element_views::ElementView,
            },
        }

        pub(crate) trait RepresentationByteReader {
            fn read_representation_byte(
                &self,
                view: &crate::storage::element_views::ElementView,
                index: usize,
            ) -> Result<u8, TerminalInterpretError>;
        }

        impl RepresentationByteReader for crate::engine::execution::TerminalExecution<'_> {
            fn read_representation_byte(
                &self,
                view: &crate::storage::element_views::ElementView,
                index: usize,
            ) -> Result<u8, TerminalInterpretError> {
                let scalar = self.read_element_view_scalar(
                    view,
                    index,
                    &[],
                    semantic_vocabulary::ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            8,
                        )
                        .map_err(|_| malformed())?,
                    ),
                )?;
                crate::boundary::structural_inputs::byte_arrays::storage::scalar_byte(&scalar)
            }
        }

        #[derive(Clone, Copy)]
        pub struct TerminalByteRead<'storage> {
            storage: ByteStorage<'storage>,
        }

        impl std::fmt::Debug for TerminalByteRead<'_> {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter
                    .debug_struct("TerminalByteRead")
                    .field("length", &self.len())
                    .finish_non_exhaustive()
            }
        }

        impl PartialEq for TerminalByteRead<'_> {
            fn eq(&self, other: &Self) -> bool {
                if let (ByteStorage::Cells(left), ByteStorage::Cells(right)) =
                    (self.storage, other.storage)
                    && left == right
                {
                    return true;
                }
                self.len() == other.len()
                    && (0..self.len()).all(|byte_index| {
                        match (self.read(byte_index), other.read(byte_index)) {
                            (Ok(left), Ok(right)) => left == right,
                            _ => false,
                        }
                    })
            }
        }

        impl Eq for TerminalByteRead<'_> {}

        impl<'storage> TerminalByteRead<'storage> {
            pub(crate) fn representation(
                reader: &'storage dyn RepresentationByteReader,
                view: &'storage crate::storage::element_views::ElementView,
            ) -> Self {
                Self {
                    storage: ByteStorage::Representation { reader, view },
                }
            }

            pub(crate) fn contiguous(bytes: &'storage [u8]) -> Self {
                Self {
                    storage: ByteStorage::Contiguous(bytes),
                }
            }

            pub(crate) fn cells(
                values: &'storage [ScalarCell],
            ) -> Result<Self, TerminalInterpretError> {
                for cell in values {
                    crate::boundary::structural_inputs::byte_arrays::storage::scalar_byte(
                        &cell.read()?,
                    )?;
                }
                Ok(Self {
                    storage: ByteStorage::Cells(values),
                })
            }

            pub fn len(&self) -> usize {
                match self.storage {
                    ByteStorage::Contiguous(bytes) => bytes.len(),
                    ByteStorage::Cells(values) => values.len(),
                    ByteStorage::Representation { view, .. } => view.len(),
                }
            }

            pub fn is_empty(&self) -> bool {
                self.len() == 0
            }

            pub fn read(&self, byte_index: usize) -> Result<u8, TerminalInterpretError> {
                match self.storage {
                    ByteStorage::Contiguous(bytes) => {
                        bytes.get(byte_index).copied().ok_or_else(malformed)
                    }
                    ByteStorage::Representation { reader, view } => {
                        reader.read_representation_byte(view, byte_index)
                    }
                    ByteStorage::Cells(values) => {
                        let value = values.get(byte_index).ok_or_else(malformed)?.read()?;
                        crate::boundary::structural_inputs::byte_arrays::storage::scalar_byte(
                            &value,
                        )
                    }
                }
            }

            pub fn copy_to(&self, destination: &mut [u8]) -> Result<(), TerminalInterpretError> {
                if destination.len() != self.len() {
                    return Err(malformed());
                }
                match self.storage {
                    ByteStorage::Contiguous(bytes) => destination.copy_from_slice(bytes),
                    ByteStorage::Cells(_) | ByteStorage::Representation { .. } => {
                        for (byte_index, byte) in destination.iter_mut().enumerate() {
                            *byte = self.read(byte_index)?;
                        }
                    }
                }
                Ok(())
            }
        }

        #[cfg(test)]
        mod tests {
            use super::{ByteStorage, TerminalByteRead};
            use crate::boundary::values::TerminalScalarValue;
            use crate::storage::scalar_storage::ScalarCell;
            use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};

            fn scalar(byte: u128) -> TerminalScalarValue {
                TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                    value: IntegerValue::Unsigned(byte),
                }
            }

            #[test]
            fn byte_read_borrows_both_storage_representations_and_copies_only_on_request() {
                let backing = [4, 8, 15, 16];
                let scalar_backing =
                    backing.map(|byte| ScalarCell::Local(scalar(u128::from(byte))));
                let contiguous = TerminalByteRead::contiguous(&backing[1..3]);
                let scalars = TerminalByteRead::cells(&scalar_backing[1..3]).unwrap();
                assert_eq!(contiguous, scalars);
                let ByteStorage::Contiguous(bytes) = contiguous.storage else {
                    panic!("contiguous")
                };
                let ByteStorage::Cells(values) = scalars.storage else {
                    panic!("scalars")
                };
                assert_eq!(bytes.as_ptr(), backing[1..].as_ptr());
                assert_eq!(values.as_ptr(), scalar_backing[1..].as_ptr());
                for view in [contiguous, scalars] {
                    assert_eq!(view.len(), 2);
                    assert_eq!(view.read(0).unwrap(), 8);
                    assert_eq!(view.read(1).unwrap(), 15);
                    assert!(view.read(2).is_err());
                    let mut wrong_size = [99; 3];
                    assert!(view.copy_to(&mut wrong_size).is_err());
                    assert_eq!(wrong_size, [99; 3]);
                    let mut output = [0; 2];
                    view.copy_to(&mut output).unwrap();
                    assert_eq!(output, [8, 15]);
                }
            }

            #[test]
            fn byte_read_checks_the_whole_window_before_exposing_malformed_scalars() {
                assert!(
                    TerminalByteRead::cells(&[
                        ScalarCell::Local(scalar(8)),
                        ScalarCell::Local(scalar(256))
                    ])
                    .is_err()
                );
                assert!(
                    TerminalByteRead::cells(&[ScalarCell::Local(TerminalScalarValue::Boolean(
                        true
                    ))])
                    .is_err()
                );
                for empty in [
                    TerminalByteRead::contiguous(&[]),
                    TerminalByteRead::cells(&[]).unwrap(),
                ] {
                    assert!(empty.is_empty());
                    assert!(empty.read(0).is_err());
                    empty.copy_to(&mut []).unwrap();
                }
            }
        }
    }
    pub(crate) mod storage {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};

        #[derive(Debug)]
        pub(crate) struct ByteSequenceStorage {
            bytes: Vec<u8>,
            lease: Option<EvaluationStorageLease>,
            account: Option<EvaluationInvocationAccount>,
        }

        impl AsRef<[u8]> for ByteSequenceStorage {
            fn as_ref(&self) -> &[u8] {
                self.bytes()
            }
        }

        impl ByteSequenceStorage {
            pub(crate) fn zeroed(
                length: usize,
                account: Option<&EvaluationInvocationAccount>,
            ) -> Result<Self, TerminalInterpretError> {
                let payload_bytes = u64::try_from(length).map_err(|_| malformed())?;
                let lease = account
                    .map(|account| account.reserve_storage(0, payload_bytes))
                    .transpose()
                    .map_err(TerminalInterpretError::Storage)?;
                let mut bytes = Vec::new();
                bytes.try_reserve_exact(length).map_err(|reason| {
                    TerminalInterpretError::StorageAllocationFailed {
                        requested_bytes: length,
                        reason: reason.to_string(),
                    }
                })?;
                bytes.resize(length, 0);
                Ok(Self {
                    bytes,
                    lease,
                    account: account.cloned(),
                })
            }

            pub(crate) fn copy(
                bytes: &[u8],
                account: Option<&EvaluationInvocationAccount>,
            ) -> Result<Self, TerminalInterpretError> {
                let mut storage = Self::zeroed(bytes.len(), account)?;
                storage.bytes.copy_from_slice(bytes);
                Ok(storage)
            }

            pub(crate) fn bytes(&self) -> &[u8] {
                &self.bytes
            }
            pub(crate) fn bytes_mut(&mut self) -> &mut [u8] {
                &mut self.bytes
            }
            pub(crate) fn account(&self) -> Option<&EvaluationInvocationAccount> {
                self.account.as_ref()
            }

            pub(crate) fn replace(&mut self, bytes: &[u8]) -> Result<(), TerminalInterpretError> {
                let previous = self.bytes.len();
                if bytes.len() > previous {
                    let added = bytes.len() - previous;
                    let added_bytes = u64::try_from(added).map_err(|_| malformed())?;
                    if let Some(lease) = &mut self.lease {
                        lease
                            .grow(0, added_bytes)
                            .map_err(TerminalInterpretError::Storage)?;
                    }
                    if let Err(reason) = self.bytes.try_reserve_exact(added) {
                        if let Some(lease) = &mut self.lease {
                            lease
                                .shrink(0, added_bytes)
                                .map_err(TerminalInterpretError::Storage)?;
                        }
                        return Err(TerminalInterpretError::StorageAllocationFailed {
                            requested_bytes: bytes.len(),
                            reason: reason.to_string(),
                        });
                    }
                }
                self.bytes.clear();
                self.bytes.extend_from_slice(bytes);
                if bytes.len() < previous
                    && let Some(lease) = &mut self.lease
                {
                    lease
                        .shrink(
                            0,
                            u64::try_from(previous - bytes.len()).map_err(|_| malformed())?,
                        )
                        .map_err(TerminalInterpretError::Storage)?;
                }
                Ok(())
            }

            pub(crate) fn from_values(
                length: usize,
                values: impl IntoIterator<Item = Result<u8, TerminalInterpretError>>,
                account: Option<&EvaluationInvocationAccount>,
            ) -> Result<Self, TerminalInterpretError> {
                let mut storage = Self::zeroed(length, account)?;
                let mut values = values.into_iter();
                for byte in &mut storage.bytes {
                    *byte = values.next().ok_or_else(malformed)??;
                }
                if values.next().is_some() {
                    return Err(malformed());
                }
                Ok(storage)
            }
        }

        #[cfg(test)]
        mod tests {
            use super::ByteSequenceStorage;
            use crate::boundary::errors::malformed;
            use crate::storage::byte_sequences::ByteSequenceView;
            use evaluation_accounting::{
                BuildEvaluationSponsor, BuildEvaluationSponsorLimits,
                EvaluationMeasurementIdentity, TerminalStorageScheduleIdentity,
            };

            fn sponsor(bytes: u64) -> BuildEvaluationSponsor {
                let measurement = EvaluationMeasurementIdentity::TerminalPsi {
                    work_schedule: psi_work::WorkSchedule::CURRENT.identity(),
                    storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
                };
                BuildEvaluationSponsor::new_with_measurement(
                    BuildEvaluationSponsorLimits::new(100, 8, 2, 2, 100, bytes, 100, 100).unwrap(),
                    measurement,
                )
                .unwrap()
            }

            #[test]
            fn views_retain_the_full_backing_once_until_its_last_owner_drops() {
                let sponsor = sponsor(5);
                let account = sponsor
                    .begin_invocation(sponsor.measurement(), 100)
                    .unwrap();
                let original = ByteSequenceView::new(
                    ByteSequenceStorage::copy(b"abcd", Some(&account)).unwrap(),
                );
                let alias = original.subslice(1, 2).unwrap();
                drop(original);
                assert_eq!(alias.bytes(), b"b");
                assert_eq!(sponsor.live_payload_bytes(), 4);
                assert!(ByteSequenceStorage::zeroed(2, Some(&account)).is_err());
                assert_eq!(sponsor.live_payload_bytes(), 4);
                drop(alias);
                assert_eq!(sponsor.live_payload_bytes(), 0);
                assert_eq!(
                    account.snapshot().unwrap().record().peak_live_payload_bytes,
                    4
                );
            }

            #[test]
            fn copy_on_write_reserves_the_overlapping_window_before_mutation() {
                for limit in [5, 6] {
                    let sponsor = sponsor(limit);
                    let account = sponsor
                        .begin_invocation(sponsor.measurement(), 100)
                        .unwrap();
                    let original = ByteSequenceView::new(
                        ByteSequenceStorage::copy(b"abcd", Some(&account)).unwrap(),
                    );
                    let mut window = original.subslice(1, 3).unwrap();
                    let changed = window.replace_byte(0, b'X');
                    assert_eq!(original.bytes(), b"abcd");
                    if limit == 5 {
                        assert!(changed.is_err());
                        assert_eq!(window.bytes(), b"bc");
                        assert_eq!(sponsor.live_payload_bytes(), 4);
                    } else {
                        changed.unwrap();
                        assert_eq!(window.bytes(), b"Xc");
                        assert_eq!(sponsor.live_payload_bytes(), 6);
                        window.replace_byte(1, b'Y').unwrap();
                        assert_eq!(
                            sponsor.live_payload_bytes(),
                            6,
                            "unique replacement allocates no backing"
                        );
                    }
                    drop(original);
                    assert_eq!(sponsor.live_payload_bytes(), if limit == 5 { 4 } else { 2 });
                    drop(window);
                    assert_eq!(sponsor.live_payload_bytes(), 0);
                }
            }

            #[test]
            fn staged_growth_shares_the_budget_and_refuses_without_replacing_contents() {
                let sponsor = sponsor(8);
                let first = sponsor
                    .begin_invocation(sponsor.measurement(), 100)
                    .unwrap();
                let second = sponsor
                    .begin_invocation(sponsor.measurement(), 100)
                    .unwrap();
                let mut storage = ByteSequenceStorage::copy(b"abc", Some(&first)).unwrap();
                let sibling = ByteSequenceStorage::zeroed(4, Some(&second)).unwrap();
                assert!(storage.replace(b"12345").is_err());
                assert_eq!(storage.bytes(), b"abc");
                assert_eq!(sponsor.live_payload_bytes(), 7);
                drop(sibling);
                storage.replace(b"12345").unwrap();
                assert_eq!(sponsor.live_payload_bytes(), 5);
                storage.replace(b"x").unwrap();
                assert_eq!(sponsor.live_payload_bytes(), 1);
                drop(storage);
                assert_eq!(sponsor.live_payload_bytes(), 0);
                assert!(ByteSequenceStorage::zeroed(usize::MAX, Some(&first)).is_err());
                assert_eq!(sponsor.live_payload_bytes(), 0);
            }

            #[test]
            fn malformed_scalar_copy_retires_its_private_reservation() {
                let sponsor = sponsor(8);
                let account = sponsor
                    .begin_invocation(sponsor.measurement(), 100)
                    .unwrap();
                assert!(
                    ByteSequenceStorage::from_values(2, [Ok(1), Err(malformed())], Some(&account))
                        .is_err()
                );
                assert_eq!(sponsor.live_payload_bytes(), 0);
                assert!(ByteSequenceStorage::from_values(2, [Ok(1)], Some(&account)).is_err());
                assert_eq!(sponsor.live_payload_bytes(), 0);
            }
        }
    }
    pub(crate) mod store {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::TerminalScalarValue;
        use crate::engine::execution::{TerminalExecution, operation_kind};
        use crate::storage::byte_sequences::binding::ByteSequenceBinding;
        use crate::storage::byte_sequences::write::MutableByteSequenceStorage;
        use semantic_vocabulary::IntegerValue;
        use terminal_psi::{Operation, OperationKind, OperationResult};

        impl TerminalExecution<'_> {
            pub(crate) fn execute_byte_sequence_store(
                &mut self,
                operation: &Operation,
            ) -> Result<(), TerminalInterpretError> {
                operation_kind!(operation => ByteSequenceStore { destination, source, length, .. });
                if operation.result != OperationResult::Unit || source == destination {
                    return Err(malformed());
                }
                let binding = self.byte_sequence_values.require(destination)?;
                let value = self.structural_value(*destination)?;
                let (owner, capacity) = match binding {
                    ByteSequenceBinding::MutableField {
                        field, capacity, ..
                    } => {
                        binding.validate_mutable_referent(self, value)?;
                        (MutableByteSequenceStorage::Field(field.clone()), *capacity)
                    }
                    ByteSequenceBinding::MutableOwner { home, capacity, .. } => {
                        binding.validate_mutable_referent(self, value)?;
                        (MutableByteSequenceStorage::Owned(home.clone()), *capacity)
                    }
                    ByteSequenceBinding::Immutable(_) => {
                        let (owner, _) = self.mutable_byte_sequence_storage(*destination)?;
                        let terminal_psi::StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                        ) = self.structural_declaration(value.structural_type)?.shape
                        else {
                            return Err(malformed());
                        };
                        (owner, capacity)
                    }
                    ByteSequenceBinding::Representation(_)
                    | ByteSequenceBinding::MutableArray { .. } => {
                        return Err(malformed());
                    }
                };
                let TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(length),
                    ..
                } = self.values.get(length).ok_or_else(malformed)?
                else {
                    return Err(malformed());
                };
                let bytes = self.byte_sequence_values.require(source)?.snapshot(self)?;
                if bytes.len() as u128 != *length || *length > u128::from(capacity) {
                    return Err(malformed());
                }
                match owner {
                    MutableByteSequenceStorage::Field(field) => {
                        self.structural_byte_sequence_fields
                            .insert(field.clone(), ByteSequenceBinding::Immutable(bytes))?;
                        self.invalidate_stored_provenance(&field.parent);
                    }
                    MutableByteSequenceStorage::Owned(home) => {
                        *self.byte_owner_bytes_mut(&home)? = bytes.owned_snapshot();
                    }
                    MutableByteSequenceStorage::Array { .. } => return Err(malformed()),
                }
                Ok(())
            }
        }
    }
    pub(crate) mod subslice {
        use crate::storage::byte_sequences::binding::ByteSequenceBinding;
        use semantic_vocabulary::{IntegerSign, IntegerType, ValueId};
        use terminal_psi::{Operation, OperationKind, StructuralAccess};

        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::TerminalStructuralValue;
        use crate::engine::execution::{TerminalExecution, operation_kind};

        impl TerminalExecution<'_> {
            pub(crate) fn execute_byte_sequence_subslice(
                &mut self,
                operation: &Operation,
            ) -> Result<(), TerminalInterpretError> {
                operation_kind!(operation => ByteSequenceSubslice { source, start, end, length, .. });
                let result = operation.result.structural().ok_or(malformed())?;
                let count_type =
                    IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
                let count = |operand: &ValueId| -> Result<usize, TerminalInterpretError> {
                    usize::try_from(self.unsigned_integer_value(*operand, count_type)?)
                        .map_err(|_| malformed())
                };
                let start = count(start)?;
                let end = count(end)?;
                let length = count(length)?;
                let access = terminal_semantics::descriptor_results::descriptor_value_access(
                    self.structural_types().values(),
                    result.structural_type,
                    result.multiplicity,
                )
                .ok_or_else(malformed)?;
                let mutable = access == StructuralAccess::MutableBorrow;
                self.validate_descriptor_borrow(source, access)?;
                let source_value =
                    self.resolve_live_projection(source.place, &source.path, access)?;
                let binding = self.byte_descriptor_binding(source, &source_value)?;
                let source_length = self.byte_descriptor_length(&binding, &source_value)?;
                let machine = self.program.verified_machine(self.current_machine)?;
                if !machine.structural_places.iter().any(|place| {
                    place.id == result.place
                        && place.kind.as_operation_result()
                            == Some((operation.id, result.structural_type))
                }) || !matches!(
                    self.structural_types().get(&result.structural_type),
                    Some(declaration)
                        if declaration.shape.is_borrowed_byte_view()
                ) || u64::try_from(length).ok() != Some(source_length)
                    || start > end
                    || end > length
                    || !terminal_semantics::descriptor_reborrow_matches(
                        self.structural_types().values(),
                        source_value.structural_type,
                        source.access,
                        Some(binding.access()),
                        result.structural_type,
                        access,
                    )
                    || (mutable && !source_value.qualifications.is_empty())
                    || !matches!(
                        result.multiplicity,
                        terminal_psi::nonlinear_multiplicity_pair_pattern!()
                    )
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                    || !result.qualification_establishments.is_empty()
                    || !result.claims.is_empty()
                    || crate::engine::custody::place_is_claimed(&self.live_claims, source.place)
                    || crate::engine::custody::place_is_claimed(&self.live_claims, result.place)
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
                    (Some(previous), Some(binding @ ByteSequenceBinding::Representation(_)))
                        if *previous
                            == TerminalStructuralValue::minted(
                                previous.opaque_identity,
                                result.structural_type,
                                Vec::new(),
                            )
                            && binding.matches_shared_home(previous) => {}
                    _ => return Err(malformed()),
                }
                let mut identity_cursor = self.local_structural_identities.cursor();
                let destination = self.mint_staged(&mut identity_cursor, result.structural_type)?;
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
                self.byte_sequence_values.insert(result.place, view)?;
                self.structural_values.insert(result.place, destination)?;
                self.track_affine_result(result);
                Ok(())
            }
            pub(crate) fn narrow_byte_sequence_binding(
                &self,
                binding: &ByteSequenceBinding,
                source: &TerminalStructuralValue,
                window: std::ops::Range<usize>,
                access: StructuralAccess,
                destination: TerminalStructuralValue,
            ) -> Result<(TerminalStructuralValue, ByteSequenceBinding), TerminalInterpretError>
            {
                if window.start > window.end
                    || !matches!(access, terminal_psi::reading_borrow_pair_pattern!())
                    || !terminal_semantics::descriptor_reborrow_matches(
                        self.structural_types().values(),
                        source.structural_type,
                        access,
                        Some(binding.access()),
                        destination.structural_type,
                        access,
                    )
                    || (access != StructuralAccess::SharedBorrow
                        && !source.qualifications.is_empty())
                    || !destination.qualifications.is_empty()
                    || !self
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
                let narrowed =
                    match binding {
                        ByteSequenceBinding::Representation(view)
                            if access == StructuralAccess::SharedBorrow =>
                        {
                            if !view.matches_descriptor_home(source) {
                                return Err(malformed());
                            }
                            let narrowed = view
                                .subslice(window.start, window.end, access)
                                .ok_or_else(malformed)?;
                            ByteSequenceBinding::Representation(std::sync::Arc::new(
                                narrowed.with_descriptor_home(destination.clone()),
                            ))
                        }
                        ByteSequenceBinding::Immutable(bytes)
                            if access == StructuralAccess::SharedBorrow =>
                        {
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
                                referent.path.push(
                                    terminal_psi::StructuralPathSegment::FixedRange { start, end },
                                );
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
                        ByteSequenceBinding::MutableOwner { .. }
                            if window.start == 0
                                && u64::try_from(window.end).ok()
                                    == Some(self.byte_descriptor_length(binding, source)?) =>
                        {
                            binding.validate_mutable_referent(self, source)?;
                            binding.clone().with_descriptor_home(destination.clone())
                        }
                        ByteSequenceBinding::Immutable(_)
                        | ByteSequenceBinding::Representation(_)
                        | ByteSequenceBinding::MutableField { .. }
                        | ByteSequenceBinding::MutableOwner { .. } => {
                            return Err(malformed());
                        }
                    };
                let narrowed = match narrowed {
                    ByteSequenceBinding::Immutable(bytes) => ByteSequenceBinding::Immutable(
                        bytes.with_descriptor_home(destination.clone()),
                    ),
                    binding => binding,
                };
                Ok((destination, narrowed))
            }
        }
    }
    pub(crate) mod write {
        use crate::boundary::structural_inputs::byte_arrays::byte_array_length;
        use semantic_vocabulary::{IntegerSign, IntegerType, PlaceId, ValueId};
        use terminal_psi::{
            Operation, OperationKind, OperationResult, StructuralAccess, StructuralMultiplicity,
        };

        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::StructuralByteSequenceRuntimeField;
        use crate::engine::custody::whole_path_discard;
        use crate::engine::execution::TerminalExecution;
        use crate::storage::byte_sequences::binding::{ByteOwnerHome, ByteSequenceBinding};
        use terminal_semantics::descriptor_results::result_access;
        use terminal_semantics::view_descriptor_parameter;

        pub(crate) enum MutableByteSequenceStorage {
            Owned(ByteOwnerHome),
            Field(StructuralByteSequenceRuntimeField),
            Array {
                backing: crate::boundary::structural_inputs::byte_arrays::storage::ArrayStorage,
                offset: u64,
            },
        }

        impl TerminalExecution<'_> {
            pub(crate) fn mutable_byte_sequence_storage(
                &self,
                place: PlaceId,
            ) -> Result<(MutableByteSequenceStorage, u64), TerminalInterpretError> {
                let machine = self.program.machine(self.current_machine)?;
                let result = machine
                    .structural_place(place)
                    .and_then(|declaration| declaration.kind.as_operation_result());
                let owned_type = machine
                    .structural_parameter(place)
                    .filter(|parameter| parameter.access == StructuralAccess::Owned)
                    .map(|parameter| parameter.structural_type)
                    .or_else(|| result.map(|(_, structural_type)| structural_type));
                if owned_type.is_some_and(|structural_type| {
                    self.structural_types()
                        .get(&structural_type)
                        .is_some_and(|declaration| {
                            declaration.shape.is_bounded_owned_byte_sequence()
                        })
                }) {
                    return self.owned_byte_sequence_storage(place);
                }
                let (structural_type, qualifications, projected_qualifications) =
                    if let Some((producer, structural_type)) = result {
                        let operation = machine.operation(producer).ok_or_else(malformed)?;
                        let result = operation.result.structural().ok_or_else(malformed)?;
                        if result_access(self.structural_types().values(), result)
                            != Some(StructuralAccess::MutableBorrow)
                            || result.place != place
                            || result.structural_type != structural_type
                            || result.multiplicity != StructuralMultiplicity::Affine
                            || !result.qualification_establishments.is_empty()
                            || !result.claims.is_empty()
                            || !self
                                .live_affine_frontier
                                .contains(&whole_path_discard(place, structural_type))
                        {
                            return Err(malformed());
                        }
                        (
                            structural_type,
                            &result.qualifications,
                            &result.projected_qualifications,
                        )
                    } else {
                        let parameter =
                            machine.structural_parameter(place).ok_or_else(malformed)?;
                        if parameter.access != StructuralAccess::MutableBorrow
                            || (parameter.multiplicity != StructuralMultiplicity::Unrestricted
                                && !view_descriptor_parameter(
                                    parameter,
                                    self.structural_types().values(),
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
                    .structural_types()
                    .get(&structural_type)
                    .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence());
                if terminal_semantics::byte_qualification_predicates_in(
                    self.program.catalogs.structural_domains.iter(),
                    structural_type,
                    qualifications,
                )
                .is_none()
                    || !projected_qualifications.is_empty()
                    || crate::engine::custody::place_is_claimed(&self.live_claims, place)
                {
                    return Err(malformed());
                }
                let value = self.structural_value(place)?;
                if value.structural_type != structural_type
                    || !matches!(self.structural_types().get(&structural_type), Some(declaration)
                            if declaration.shape.is_borrowed_byte_carrier() || bounded_owner)
                {
                    return Err(malformed());
                }
                let binding = self.byte_sequence_values.require(&place)?;
                binding.validate_mutable_referent(self, value)?;
                if let ByteSequenceBinding::MutableArray {
                    array,
                    array_type,
                    offset,
                    length: window_length,
                    ..
                } = binding
                {
                    let length = byte_array_length(&self.structural_types(), *array_type)
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
                    ByteSequenceBinding::Representation(_) => return Err(malformed()),
                    ByteSequenceBinding::Immutable(_)
                    | ByteSequenceBinding::MutableArray { .. } => {
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
                let machine = self.program.machine(self.current_machine)?;
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
                let value = self.structural_value(place)?;
                if multiplicity != StructuralMultiplicity::Unrestricted
                    || !projected_qualifications.is_empty()
                    || value.structural_type != structural_type
                    || &value.qualifications != qualifications
                    || !value.path.is_empty()
                    || crate::engine::custody::place_is_claimed(&self.live_claims, place)
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
                ) = self.structural_declaration(structural_type)?.shape
                else {
                    return Err(malformed());
                };
                let bytes = self.byte_sequence_values.require(&place)?.immutable()?;
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
                match crate::engine::custody::verified_structural_place(
                    &self.byte_sequence_values,
                    source,
                )? {
                    ByteSequenceBinding::Immutable(view) => {
                        u64::try_from(view.len()).map_err(|_| malformed())
                    }
                    binding @ ByteSequenceBinding::Representation(_) => {
                        self.byte_descriptor_length(binding, self.structural_value(source)?)
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
                match self.byte_sequence_values.require(&source)? {
                    ByteSequenceBinding::Immutable(view) => {
                        view.get(index).copied().ok_or_else(malformed)
                    }
                    binding @ ByteSequenceBinding::Representation(_) => {
                        self.byte_descriptor_length(binding, self.structural_value(source)?)?;
                        binding.read_view(self)?.read(index)
                    }
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
                                .require(&field)?
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
                let changed_parent = match &storage {
                    MutableByteSequenceStorage::Field(field) => Some(field.parent.clone()),
                    _ => None,
                };
                let (bytes, backing_index) = match storage {
                    MutableByteSequenceStorage::Owned(home) => {
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
                        let index =
                            usize::try_from(offset.checked_add(byte_index).ok_or_else(malformed)?)
                                .map_err(|_| malformed())?;
                        return self.stored_byte_array_write(&backing, index, byte);
                    }
                };
                bytes.ok_or_else(malformed)?.replace_byte(
                    usize::try_from(backing_index).map_err(|_| malformed())?,
                    byte,
                )?;
                if let Some(parent) = changed_parent {
                    self.invalidate_stored_provenance(&parent);
                }
                Ok(())
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
    }

    use std::ops::Range;
    use std::sync::Arc;

    #[derive(Clone)]
    pub(crate) struct ByteSequenceView {
        backing: Arc<ByteSequenceBacking>,
        window: Range<usize>,
        descriptor_home: Option<crate::boundary::values::TerminalStructuralValue>,
        original_array: Option<OriginalByteArrayWindow>,
    }

    #[derive(Clone)]
    pub(crate) struct OriginalByteArrayWindow {
        pub(crate) array: crate::boundary::structural_inputs::byte_arrays::storage::ArrayStorage,
        pub(crate) array_type: semantic_vocabulary::StructuralTypeId,
        pub(crate) offset: usize,
        pub(crate) length: usize,
    }

    struct ByteSequenceBacking {
        storage: storage::ByteSequenceStorage,
        exposure_namespace: Option<u64>,
    }

    impl PartialEq for ByteSequenceView {
        fn eq(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.backing, &other.backing) && self.window == other.window
        }
    }

    impl Eq for ByteSequenceView {}

    impl ByteSequenceView {
        pub(crate) fn shares_backing(&self, other: &Self) -> bool {
            Arc::ptr_eq(&self.backing, &other.backing)
        }

        pub(crate) fn retained_payload_bytes(&self) -> usize {
            self.backing.storage.bytes().len()
        }

        pub(crate) fn new(storage: storage::ByteSequenceStorage) -> Self {
            let length = storage.bytes().len();
            Self {
                backing: Arc::new(ByteSequenceBacking {
                    storage,
                    exposure_namespace:
                        crate::storage::structural_identities::new_exposure_namespace(),
                }),
                window: 0..length,
                descriptor_home: None,
                original_array: None,
            }
        }

        pub(crate) fn with_descriptor_home(
            mut self,
            home: crate::boundary::values::TerminalStructuralValue,
        ) -> Self {
            self.descriptor_home = Some(home);
            self
        }

        pub(crate) fn with_original_array(mut self, window: OriginalByteArrayWindow) -> Self {
            self.original_array = Some(window);
            self
        }

        pub(crate) fn original_array(&self) -> Option<&OriginalByteArrayWindow> {
            self.original_array.as_ref()
        }

        pub(crate) fn owned_snapshot(&self) -> Self {
            let mut copy = self.detached_descriptor();
            copy.original_array = None;
            copy
        }

        pub(crate) fn original_byte_allocation(&self) -> Option<(u64, usize, usize)> {
            Some((
                self.backing.exposure_namespace?,
                self.backing.storage.bytes().len(),
                self.window.start,
            ))
        }

        pub(crate) fn descriptor_home(
            &self,
        ) -> Option<&crate::boundary::values::TerminalStructuralValue> {
            self.descriptor_home.as_ref()
        }

        pub(crate) fn detached_descriptor(&self) -> Self {
            let mut descriptor = self.clone();
            descriptor.descriptor_home = None;
            descriptor
        }

        pub(crate) fn matches_descriptor_home(
            &self,
            value: &crate::boundary::values::TerminalStructuralValue,
        ) -> bool {
            self.descriptor_home
                .as_ref()
                .map_or_else(|| value.path.is_empty(), |home| home == value)
        }

        pub(crate) fn matches_bound_descriptor_home(
            &self,
            binding: &crate::boundary::values::TerminalStructuralValue,
            selected: &crate::boundary::values::TerminalStructuralValue,
        ) -> bool {
            let same_home = |home: &crate::boundary::values::TerminalStructuralValue| {
                home.opaque_identity == selected.opaque_identity
                    && home.path == selected.path
                    && home.structural_type == selected.structural_type
            };
            same_home(binding) && self.descriptor_home.as_ref().is_none_or(same_home)
        }

        pub(crate) fn storage_address_coordinate(&self, index: usize) -> Option<(u64, u64)> {
            if index >= self.len() {
                return None;
            }
            Some((
                self.backing.exposure_namespace?,
                u64::try_from(self.window.start.checked_add(index)?).ok()?,
            ))
        }

        pub(crate) fn len(&self) -> usize {
            self.window.end - self.window.start
        }

        pub(crate) fn bytes(&self) -> &[u8] {
            &self.backing.storage.bytes()[self.window.clone()]
        }

        pub(crate) fn into_account(
            self,
            account: Option<&evaluation_accounting::EvaluationInvocationAccount>,
        ) -> Result<Self, crate::boundary::errors::TerminalInterpretError> {
            let same = match (self.backing.storage.account(), account) {
                (None, None) => true,
                (Some(previous), Some(current)) => previous.same_invocation(current),
                _ => false,
            };
            if same {
                return Ok(self);
            }
            Ok(Self::new(storage::ByteSequenceStorage::copy(
                self.bytes(),
                account,
            )?))
        }

        pub(crate) fn get(&self, index: usize) -> Option<&u8> {
            self.bytes().get(index)
        }

        pub(crate) fn replace_byte(
            &mut self,
            byte_index: usize,
            value: u8,
        ) -> Result<(), crate::boundary::errors::TerminalInterpretError> {
            if byte_index >= self.len() {
                return Err(crate::boundary::errors::malformed());
            }
            let storage_index = self
                .window
                .start
                .checked_add(byte_index)
                .ok_or_else(crate::boundary::errors::malformed)?;
            if let Some(bytes) = Arc::get_mut(&mut self.backing) {
                *bytes
                    .storage
                    .bytes_mut()
                    .get_mut(storage_index)
                    .ok_or_else(crate::boundary::errors::malformed)? = value;
            } else {
                let mut storage = storage::ByteSequenceStorage::copy(
                    self.bytes(),
                    self.backing.storage.account(),
                )?;
                storage.bytes_mut()[byte_index] = value;
                *self = Self::new(storage);
            }
            self.original_array = None;
            Ok(())
        }

        pub(crate) fn subslice(&self, start: usize, end: usize) -> Option<Self> {
            self.bytes().get(start..end)?;
            let original_array = match &self.original_array {
                Some(original) => Some(OriginalByteArrayWindow {
                    array: original.array.clone(),
                    array_type: original.array_type,
                    offset: original.offset.checked_add(start)?,
                    length: end - start,
                }),
                None => None,
            };
            Some(Self {
                backing: Arc::clone(&self.backing),
                window: self.window.start.checked_add(start)?
                    ..self.window.start.checked_add(end)?,
                descriptor_home: None,
                original_array,
            })
        }
    }
}

pub(crate) mod element_views {
    pub(crate) mod immutable_sequence {
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::TerminalStructuralValue;
        use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};

        pub(crate) struct ImmutableStructuralSequence {
            elements: Vec<TerminalStructuralValue>,
            _lease: Option<EvaluationStorageLease>,
        }

        impl ImmutableStructuralSequence {
            pub(crate) fn from_values(
                length: usize,
                values: impl IntoIterator<
                    Item = Result<TerminalStructuralValue, TerminalInterpretError>,
                >,
                account: Option<&EvaluationInvocationAccount>,
            ) -> Result<Self, TerminalInterpretError> {
                let bytes = length
                    .checked_mul(std::mem::size_of::<TerminalStructuralValue>())
                    .ok_or_else(malformed)?;
                let payload_bytes = u64::try_from(bytes).map_err(|_| malformed())?;
                let lease = account
                    .map(|account| account.reserve_storage(0, payload_bytes))
                    .transpose()
                    .map_err(TerminalInterpretError::Storage)?;
                let mut elements = Vec::new();
                elements.try_reserve_exact(length).map_err(|reason| {
                    TerminalInterpretError::StorageAllocationFailed {
                        requested_bytes: bytes,
                        reason: reason.to_string(),
                    }
                })?;
                let mut values = values.into_iter();
                for _ in 0..length {
                    let value = values.next().ok_or_else(malformed)??;
                    if !value.path.is_empty()
                        || !value.qualifications.is_empty()
                        || !value.projected_qualifications.is_empty()
                    {
                        return Err(malformed());
                    }
                    elements.push(value);
                }
                if values.next().is_some() {
                    return Err(malformed());
                }
                Ok(Self {
                    elements,
                    _lease: lease,
                })
            }

            pub(crate) fn len(&self) -> usize {
                self.elements.len()
            }

            pub(crate) fn element(&self, position: usize) -> Option<&TerminalStructuralValue> {
                self.elements.get(position)
            }
        }
    }

    use crate::boundary::errors::{TerminalInterpretError, malformed};
    use crate::boundary::values::StructuralScalarRuntimeField;
    use crate::boundary::values::TerminalScalarValue;
    use crate::boundary::values::TerminalStructuralValue;
    use crate::engine::custody::literal_fixed_array;
    use crate::engine::execution::{OperationFlow, TerminalExecution, operation_kind};
    use crate::engine::scalar_operations::terminal_scalar_belongs_to_type;
    use crate::storage::runtime_rows::RuntimeRows;
    use semantic_vocabulary::IntegerSign;
    use semantic_vocabulary::IntegerType;
    use semantic_vocabulary::IntegerValue;
    use semantic_vocabulary::ScalarType;
    use semantic_vocabulary::StructuralPlaceKind;
    use semantic_vocabulary::ValueId;
    use std::ops::Range;
    use std::sync::Arc;
    use terminal_psi::ElementViewRegion;
    use terminal_psi::Operation;
    use terminal_psi::OperationKind;
    use terminal_psi::OperationResult;
    use terminal_psi::StructuralAccess;
    use terminal_psi::StructuralMultiplicity;
    use terminal_psi::StructuralPathSegment;
    use terminal_psi::StructuralTypeShape;

    #[derive(Clone)]
    pub(crate) struct ElementViewHome {
        pub(crate) storage: crate::storage::primitive_storage::PrimitiveStorageHome,
        pub(crate) path: Vec<StructuralPathSegment>,
    }

    pub(crate) struct CollectionStorage {
        pub(crate) collection: Option<TerminalStructuralValue>,
        pub(crate) home: Option<ElementViewHome>,
        pub(crate) view: Option<crate::storage::reference::AggregateViewTip>,
    }

    impl ElementViewHome {
        fn selected_path(
            &self,
            position: usize,
            suffix: &[StructuralPathSegment],
        ) -> Result<Vec<StructuralPathSegment>, TerminalInterpretError> {
            let mut path = Vec::with_capacity(self.path.len() + 1 + suffix.len());
            path.extend_from_slice(&self.path);
            path.push(StructuralPathSegment::FixedIndex(
                u64::try_from(position).map_err(|_| malformed())?,
            ));
            path.extend_from_slice(suffix);
            Ok(path)
        }
    }

    #[derive(Clone)]
    pub(crate) struct ElementView {
        diagnostic_source: semantic_vocabulary::PlaceId,
        backing: ElementViewBacking,
        window: Range<usize>,
        home: Option<Arc<ElementViewHome>>,
        descriptor_home: Option<TerminalStructuralValue>,
        access: StructuralAccess,
    }

    #[derive(Clone)]
    enum ElementViewBacking {
        Structural { collection: TerminalStructuralValue },
        ImmutableSequence(Arc<immutable_sequence::ImmutableStructuralSequence>),
        Typed(Arc<typed::TypedElementView>),
        Representation(Arc<representation::RepresentationView>),
    }

    impl ElementView {
        pub(crate) fn retain_storage(
            &self,
            retained: &mut crate::storage::structural_storage::RetainedStorage,
        ) {
            self.retain_projection_storage(retained, &[]);
        }

        pub(crate) fn retain_projection_storage(
            &self,
            retained: &mut crate::storage::structural_storage::RetainedStorage,
            projection: &[terminal_psi::ReferenceOriginProjection],
        ) {
            if let Some(home) = &self.home {
                retained.primitive_home(home.storage);
            }
            match &self.backing {
                ElementViewBacking::Typed(view) => view.retain_storage(retained),
                ElementViewBacking::Representation(view) => view.retain_storage(retained),
                ElementViewBacking::ImmutableSequence(elements) => {
                    for position in self.window.clone() {
                        if let Some(element) = elements.element(position) {
                            retained.value(element);
                        }
                    }
                    return;
                }
                ElementViewBacking::Structural { .. } => {}
            }
            let ElementViewBacking::Structural { collection } = &self.backing else {
                return;
            };
            let (window, suffix) = match projection.split_first() {
                None => (self.window.clone(), &[][..]),
                Some((terminal_psi::ReferenceOriginProjection::Element, suffix)) => {
                    (self.window.clone(), suffix)
                }
                Some((
                    terminal_psi::ReferenceOriginProjection::Exact(
                        StructuralPathSegment::FixedIndex(index),
                    ),
                    suffix,
                )) => {
                    let Ok(index) = usize::try_from(*index) else {
                        return;
                    };
                    let Some(selected) = self
                        .window
                        .start
                        .checked_add(index)
                        .filter(|selected| *selected < self.window.end)
                    else {
                        return;
                    };
                    let Some(end) = selected.checked_add(1) else {
                        return;
                    };
                    (selected..end, suffix)
                }
                _ => return,
            };
            retained.elements(collection, window, suffix);
        }

        pub(crate) fn retained_bytes(
            &self,
        ) -> Option<&crate::storage::byte_sequences::ByteSequenceView> {
            match &self.backing {
                ElementViewBacking::Representation(view) => view.retained_bytes(),
                ElementViewBacking::Structural { .. }
                | ElementViewBacking::ImmutableSequence(_)
                | ElementViewBacking::Typed(_) => None,
            }
        }

        pub(crate) fn captured_storage_home(
            &self,
            types: &crate::embedding::program::ProgramDeclarations<
                '_,
                terminal_psi::StructuralTypeDeclaration,
            >,
            path: &[StructuralPathSegment],
        ) -> Result<Option<ElementViewHome>, TerminalInterpretError> {
            if path.is_empty() {
                self.structural_projection(
                    types,
                    path,
                    self.access != StructuralAccess::SharedBorrow,
                )?;
                return Ok(self.home.as_deref().cloned());
            }
            let Some((StructuralPathSegment::FixedIndex(element_index), suffix)) =
                path.split_first()
            else {
                return Err(malformed());
            };
            let element_index = usize::try_from(*element_index).map_err(|_| malformed())?;
            if element_index >= self.len() {
                return Err(malformed());
            }
            self.structural_projection(types, path, self.access != StructuralAccess::SharedBorrow)?;
            self.home
                .as_ref()
                .map(|home| {
                    Ok(ElementViewHome {
                        storage: home.storage,
                        path: home.selected_path(
                            self.window
                                .start
                                .checked_add(element_index)
                                .ok_or_else(malformed)?,
                            suffix,
                        )?,
                    })
                })
                .transpose()
        }

        pub(crate) fn structural(
            collection: TerminalStructuralValue,
            length: usize,
            access: StructuralAccess,
            home: Option<ElementViewHome>,
            diagnostic_source: semantic_vocabulary::PlaceId,
        ) -> Self {
            Self {
                diagnostic_source,
                backing: ElementViewBacking::Structural { collection },
                window: 0..length,
                home: home.map(Arc::new),
                descriptor_home: None,
                access,
            }
        }

        pub(crate) fn immutable_sequence(
            elements: immutable_sequence::ImmutableStructuralSequence,
            descriptor: TerminalStructuralValue,
            diagnostic_source: semantic_vocabulary::PlaceId,
        ) -> Self {
            let length = elements.len();
            Self {
                diagnostic_source,
                backing: ElementViewBacking::ImmutableSequence(Arc::new(elements)),
                window: 0..length,
                home: None,
                descriptor_home: Some(descriptor),
                access: StructuralAccess::SharedBorrow,
            }
        }

        pub(crate) fn structural_projection(
            &self,
            types: &crate::embedding::program::ProgramDeclarations<
                '_,
                terminal_psi::StructuralTypeDeclaration,
            >,
            path: &[StructuralPathSegment],
            writing: bool,
        ) -> Result<(TerminalStructuralValue, Vec<StructuralPathSegment>), TerminalInterpretError>
        {
            if let ElementViewBacking::ImmutableSequence(elements) = &self.backing {
                if writing || self.access != StructuralAccess::SharedBorrow {
                    return Err(malformed());
                }
                let Some((StructuralPathSegment::FixedIndex(position), suffix)) =
                    path.split_first()
                else {
                    return Err(malformed());
                };
                let position = usize::try_from(*position).map_err(|_| malformed())?;
                if position >= self.len() {
                    return Err(malformed());
                }
                let selected = self
                    .window
                    .start
                    .checked_add(position)
                    .ok_or_else(malformed)?;
                return Ok((
                    elements.element(selected).ok_or_else(malformed)?.clone(),
                    suffix.to_vec(),
                ));
            }
            let ElementViewBacking::Structural { collection } = &self.backing else {
                return Err(malformed());
            };
            let requested = if writing {
                StructuralAccess::WriteOnlyBorrow
            } else {
                StructuralAccess::SharedBorrow
            };
            if !requested.admits_referent(self.access) {
                return Err(malformed());
            }
            if path.is_empty() {
                let home = self.descriptor_home.as_ref().ok_or_else(malformed)?;
                let (_, length) = literal_fixed_array(types, collection.structural_type)?;
                if home.structural_type != collection.structural_type
                    || self.window.start != 0
                    || u64::try_from(self.window.end).ok() != Some(length)
                {
                    return Err(malformed());
                }
                return Ok((collection.clone(), Vec::new()));
            }
            let Some((StructuralPathSegment::FixedIndex(index), suffix)) = path.split_first()
            else {
                return Err(malformed());
            };
            let index = usize::try_from(*index).map_err(|_| malformed())?;
            if index >= self.len() {
                return Err(malformed());
            }
            let selected = self.window.start.checked_add(index).ok_or_else(malformed)?;
            let mut path = Vec::with_capacity(path.len());
            path.push(StructuralPathSegment::FixedIndex(
                u64::try_from(selected).map_err(|_| malformed())?,
            ));
            path.extend_from_slice(suffix);
            Ok((collection.clone(), path))
        }

        pub(crate) fn with_descriptor_home(mut self, home: TerminalStructuralValue) -> Self {
            self.descriptor_home = Some(home);
            self
        }

        pub(crate) fn project_array(
            &self,
            types: &crate::embedding::program::ProgramDeclarations<
                '_,
                terminal_psi::StructuralTypeDeclaration,
            >,
            path: &[StructuralPathSegment],
            selected: &TerminalStructuralValue,
        ) -> Result<Self, TerminalInterpretError> {
            if path.is_empty() {
                if !self.matches_descriptor_home(selected) {
                    return Err(malformed());
                }
                return Ok(self.clone());
            }
            let (collection, projection) = self.structural_projection(
                types,
                path,
                self.access != StructuralAccess::SharedBorrow,
            )?;
            let projected =
                crate::engine::custody::project_structural_value(types, &collection, &projection)?;
            let StructuralTypeShape::FixedArray {
                length: terminal_psi::ArrayExtent::Literal(length),
                ..
            } = types.declaration(&selected.structural_type)?.shape
            else {
                return Err(malformed());
            };
            if !Self::same_descriptor_home(&projected, selected) {
                return Err(malformed());
            }
            let home = self.home.as_ref().map(|home| {
                let mut home_path = home.path.clone();
                home_path.extend(projection);
                ElementViewHome {
                    storage: home.storage,
                    path: home_path,
                }
            });
            Ok(Self::structural(
                selected.clone(),
                usize::try_from(length).map_err(|_| malformed())?,
                self.access,
                home,
                self.diagnostic_source,
            )
            .with_descriptor_home(selected.clone()))
        }

        pub(crate) fn lends_same_array_storage(&self, returned: &Self) -> bool {
            let (
                ElementViewBacking::Structural {
                    collection: expected,
                },
                ElementViewBacking::Structural { collection: actual },
            ) = (&self.backing, &returned.backing)
            else {
                return false;
            };
            expected.opaque_identity == actual.opaque_identity
                && expected.path == actual.path
                && expected.structural_type == actual.structural_type
                && self.window == returned.window
                && match (&self.home, &returned.home) {
                    (Some(expected), Some(actual)) => {
                        expected.storage == actual.storage && expected.path == actual.path
                    }
                    (None, None) => true,
                    _ => false,
                }
                && returned.access.admits_referent(self.access)
        }

        pub(crate) fn detached_descriptor(&self) -> Self {
            let mut descriptor = self.clone();
            descriptor.descriptor_home = None;
            descriptor
        }

        pub(crate) fn descriptor_home(&self) -> Option<&TerminalStructuralValue> {
            self.descriptor_home.as_ref()
        }

        pub(crate) fn matches_descriptor_home(&self, value: &TerminalStructuralValue) -> bool {
            self.descriptor_home.as_ref().map_or_else(
                || value.path.is_empty(),
                |home| Self::same_descriptor_home(home, value),
            )
        }

        fn same_descriptor_home(
            left: &TerminalStructuralValue,
            right: &TerminalStructuralValue,
        ) -> bool {
            left.opaque_identity == right.opaque_identity
                && left.path == right.path
                && left.structural_type == right.structural_type
        }

        pub(crate) fn matches_bound_descriptor_home(
            &self,
            binding: &TerminalStructuralValue,
            selected: &TerminalStructuralValue,
        ) -> bool {
            let same_home =
                |home: &TerminalStructuralValue| Self::same_descriptor_home(home, selected);
            same_home(binding) && self.descriptor_home.as_ref().is_none_or(same_home)
        }

        pub(crate) fn len(&self) -> usize {
            self.window.end - self.window.start
        }

        pub(crate) fn complete_scalar_array_frame(
            &self,
            types: &crate::embedding::program::ProgramDeclarations<
                '_,
                terminal_psi::StructuralTypeDeclaration,
            >,
        ) -> Option<(
            usize,
            semantic_vocabulary::PlaceId,
            semantic_vocabulary::StructuralTypeId,
        )> {
            let home = self.home.as_ref()?;
            let ElementViewBacking::Structural { collection } = &self.backing else {
                return None;
            };
            let (depth, place, root) = home.storage.scalar_array_frame();
            let selected =
                terminal_semantics::runtime_structural_path_tip(types.values(), root, &home.path)?;
            let StructuralTypeShape::FixedArray {
                length: terminal_psi::ArrayExtent::Literal(length),
                ..
            } = selected.shape
            else {
                return None;
            };
            (selected.id == collection.structural_type
                && self.window.start == 0
                && u64::try_from(self.window.end).ok() == Some(length))
            .then_some((depth, place, root))
        }

        pub(crate) fn access(&self) -> StructuralAccess {
            self.access
        }

        pub(crate) fn owned_multiplicity(&self) -> Option<StructuralMultiplicity> {
            match self.access {
                StructuralAccess::SharedBorrow => Some(StructuralMultiplicity::Unrestricted),
                StructuralAccess::MutableBorrow => Some(StructuralMultiplicity::Affine),
                _ => None,
            }
        }

        pub(crate) fn lends_structural_storage(&self, selected: &TerminalStructuralValue) -> bool {
            let ElementViewBacking::Structural { collection } = &self.backing else {
                return false;
            };
            let Some(path) = selected.path.strip_prefix(collection.path.as_slice()) else {
                return false;
            };
            let Some(StructuralPathSegment::FixedIndex(element_index)) = path.first() else {
                return false;
            };
            selected.opaque_identity == collection.opaque_identity
                && self.access == StructuralAccess::MutableBorrow
                && usize::try_from(*element_index)
                    .is_ok_and(|element_index| self.window.contains(&element_index))
        }

        pub(crate) fn captured_structural_index(
            &self,
            selected: &TerminalStructuralValue,
        ) -> Option<u64> {
            if let ElementViewBacking::ImmutableSequence(elements) = &self.backing {
                return self.window.clone().find_map(|position| {
                    let element = elements.element(position)?;
                    (element.opaque_identity == selected.opaque_identity
                        && selected.path.starts_with(&element.path))
                    .then(|| u64::try_from(position - self.window.start).ok())
                    .flatten()
                });
            }
            let ElementViewBacking::Structural { collection } = &self.backing else {
                return None;
            };
            if selected.opaque_identity != collection.opaque_identity {
                return None;
            }
            let suffix = selected.path.strip_prefix(collection.path.as_slice())?;
            let StructuralPathSegment::FixedIndex(index) = suffix.first()? else {
                return None;
            };
            let index = usize::try_from(*index).ok()?;
            self.window
                .contains(&index)
                .then(|| u64::try_from(index - self.window.start).ok())
                .flatten()
        }

        pub(crate) fn initialized_empty_scalar_array(
            &self,
            execution: &TerminalExecution,
            path: &[StructuralPathSegment],
            expected: semantic_vocabulary::StructuralTypeId,
        ) -> Result<bool, TerminalInterpretError> {
            if !StructuralAccess::SharedBorrow.admits_referent(self.access) {
                return Err(malformed());
            }
            let types = execution.structural_types();
            let (collection, projection) = if path.is_empty() {
                let ElementViewBacking::Structural { collection } = &self.backing else {
                    return Err(malformed());
                };
                let (_, length) = literal_fixed_array(&types, collection.structural_type)?;
                if self.window.start != 0 || u64::try_from(self.window.end).ok() != Some(length) {
                    return Err(malformed());
                }
                (collection.clone(), Vec::new())
            } else {
                self.structural_projection(&types, path, false)?
            };
            if let Some(home) = &self.home {
                let mut selected = home.path.clone();
                selected.extend(projection);
                execution.initialized_empty_array_in_frame(home.storage, &selected, expected)
            } else {
                let selected = crate::engine::custody::project_structural_value(
                    &types,
                    &collection,
                    &projection,
                )?;
                if selected.structural_type != expected {
                    return Err(malformed());
                }
                execution.initialized_empty_scalar_array(
                    &crate::boundary::values::StructuralRuntimePlace::from(&selected),
                    expected,
                )
            }
        }

        pub(crate) fn scalar_storage(
            &self,
            execution: &TerminalExecution,
            path: &[StructuralPathSegment],
            writing: bool,
        ) -> Result<
            (
                crate::storage::primitive_storage::PrimitiveStorage,
                ScalarType,
                bool,
            ),
            TerminalInterpretError,
        > {
            let requested = if writing {
                StructuralAccess::WriteOnlyBorrow
            } else {
                StructuralAccess::SharedBorrow
            };
            if !requested.admits_referent(self.access) {
                return Err(malformed());
            }
            let Some((StructuralPathSegment::FixedIndex(element_index), suffix)) =
                path.split_first()
            else {
                return Err(malformed());
            };
            let element_index = usize::try_from(*element_index).map_err(|_| malformed())?;
            if element_index >= self.len() {
                return Err(malformed());
            }
            let position = self
                .window
                .start
                .checked_add(element_index)
                .ok_or_else(malformed)?;
            if let ElementViewBacking::ImmutableSequence(elements) = &self.backing {
                if writing || self.access != StructuralAccess::SharedBorrow {
                    return Err(malformed());
                }
                let element = elements.element(position).ok_or_else(malformed)?;
                let scalar = terminal_semantics::primitive_read_projection_type(
                    execution.structural_types().values(),
                    element.structural_type,
                    suffix,
                )
                .ok_or_else(malformed)?;
                let path = terminal_semantics::canonical_primitive_read_path(
                    execution.structural_types().values(),
                    element.structural_type,
                    suffix,
                )
                .ok_or_else(malformed)?;
                return execution.primitive_view_access(element, &path, scalar, true);
            }
            if let Some(home) = &self.home {
                let path = home.selected_path(position, suffix)?;
                execution.primitive_home_access(home.storage, writing, &path)
            } else {
                let ElementViewBacking::Structural { collection } = &self.backing else {
                    return Err(malformed());
                };
                let mut selected = vec![StructuralPathSegment::FixedIndex(
                    u64::try_from(position).map_err(|_| malformed())?,
                )];
                selected.extend_from_slice(suffix);
                let scalar = if writing {
                    terminal_semantics::primitive_projection_type
                } else {
                    terminal_semantics::primitive_read_projection_type
                }(
                    execution.structural_types().values(),
                    collection.structural_type,
                    &selected,
                )
                .ok_or_else(malformed)?;
                execution.resident_element_storage(collection, position, suffix, scalar, !writing)
            }
        }

        pub(crate) fn subslice(
            &self,
            start: usize,
            end: usize,
            access: StructuralAccess,
        ) -> Option<Self> {
            if start > end || end > self.len() || !access.admits_referent(self.access) {
                return None;
            }
            Some(Self {
                diagnostic_source: self.diagnostic_source,
                backing: self.backing.clone(),
                window: self.window.start.checked_add(start)?
                    ..self.window.start.checked_add(end)?,
                home: self.home.clone(),
                descriptor_home: None,
                access,
            })
        }
    }

    fn count(
        operand: &ValueId,
        values: &RuntimeRows<ValueId, TerminalScalarValue>,
    ) -> Result<usize, TerminalInterpretError> {
        let count_type = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
        let TerminalScalarValue::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(value),
        } = values
            .get(operand)
            .ok_or(TerminalInterpretError::VerifiedValueMissing(*operand))?
        else {
            return Err(malformed());
        };
        if *scalar_type != count_type {
            return Err(malformed());
        }
        let value = u64::try_from(*value).map_err(|_| malformed())?;
        usize::try_from(value).map_err(|_| malformed())
    }

    impl TerminalExecution<'_> {
        pub(crate) fn element_view_argument(
            &self,
            argument: &terminal_psi::StructuralArgument,
            resolved: &TerminalStructuralValue,
        ) -> Result<&ElementView, TerminalInterpretError> {
            if argument.path.is_empty()
                && let Some(view) = self.element_view_values.get(&argument.place)
            {
                return if self
                    .structural_values
                    .get(&argument.place)
                    .is_some_and(|home| view.matches_bound_descriptor_home(home, resolved))
                {
                    Ok(view)
                } else {
                    Err(malformed())
                };
            }
            self.element_view_at_resolved_home(resolved)
        }

        pub(crate) fn element_view_at_resolved_home(
            &self,
            resolved: &TerminalStructuralValue,
        ) -> Result<&ElementView, TerminalInterpretError> {
            self.structural_element_view_fields
                .get(&crate::boundary::values::StructuralRuntimePlace::from(
                    resolved,
                ))
                .or_else(|| {
                    self.element_view_values.iter().find_map(|(place, view)| {
                        self.structural_values
                            .get(place)
                            .filter(|home| view.matches_bound_descriptor_home(home, resolved))
                            .map(|_| view)
                    })
                })
                .or_else(|| {
                    self.call_stack.iter().find_map(|frame| {
                        frame.element_view_values.iter().find_map(|(place, view)| {
                            frame
                                .structural_values
                                .get(place)
                                .filter(|home| view.matches_bound_descriptor_home(home, resolved))
                                .map(|_| view)
                        })
                    })
                })
                .ok_or_else(malformed)
        }

        pub(crate) fn retire_unrestricted_views(&mut self) {
            let retired = self
                .byte_sequence_values
                .keys()
                .chain(self.element_view_values.keys())
                .copied()
                .filter(|place| {
                    !self.live_affine_discard_at(*place).is_some()
                        && !crate::engine::custody::place_is_claimed(&self.live_claims, *place)
                })
                .collect::<Vec<_>>();
            for place in retired {
                if let Some(retired) = self.structural_values.remove(&place) {
                    let origin = self
                        .reference_referents
                        .get(&crate::boundary::values::StructuralRuntimePlace::from(
                            &retired,
                        ))
                        .cloned();
                    self.retired_structural_homes.push((place, retired, origin));
                }
                self.byte_sequence_values.remove(&place);
                self.element_view_values.remove(&place);
            }
        }

        pub(crate) fn execute_establish_element_view(
            &mut self,
            operation: &Operation,
        ) -> Result<OperationFlow, TerminalInterpretError> {
            let (destination, source, element, region, access, multiplicity) = match &operation.kind
            {
                OperationKind::EstablishElementView {
                    destination,
                    source,
                    element,
                    region,
                } => (
                    *destination,
                    source,
                    *element,
                    *region,
                    StructuralAccess::SharedBorrow,
                    StructuralMultiplicity::Unrestricted,
                ),
                OperationKind::EstablishMutableElementView {
                    destination,
                    source,
                    element,
                    region,
                } => (
                    *destination,
                    source,
                    *element,
                    *region,
                    StructuralAccess::MutableBorrow,
                    StructuralMultiplicity::Affine,
                ),
                _ => return Err(malformed()),
            };
            let Some(result) = operation.result.as_structural() else {
                return Err(malformed().in_reference_formation("element view result carrier"));
            };
            if destination != result.place
                || source.access != access
                || result.multiplicity != multiplicity
                || !result.qualifications.is_empty()
                || result
                    .projected_qualifications
                    .iter()
                    .any(|row| !row.is_element_membership())
                || !result.claims.is_empty()
            {
                return Err(malformed().in_reference_formation("element view result signature"));
            }
            let machine = self.program.verified_machine(self.current_machine)?;
            if !machine.structural_places.iter().any(|place| {
                place.id == result.place
                    && place.kind.as_operation_result()
                        == Some((operation.id, result.structural_type))
            }) {
                return Err(malformed().in_reference_formation("element view producer"));
            }
            let element_scalar = match &self.program.structural_declaration(element)?.shape {
                StructuralTypeShape::PrimitiveScalar(scalar) => Some(*scalar),
                _ => None,
            };
            let descriptor = &self
                .program
                .structural_declaration(result.structural_type)?
                .shape;
            match descriptor {
                StructuralTypeShape::ElementView {
                    element: view_element,
                    ..
                } if *view_element == element => {}
                StructuralTypeShape::ByteSequence(_)
                    if descriptor.is_borrowed_byte_carrier()
                        && element_scalar
                            == Some(ScalarType::Integer(
                                IntegerType::new(IntegerSign::Unsigned, 8)
                                    .map_err(|_| malformed())?,
                            )) => {}
                _ => return Err(malformed().in_reference_formation("element view descriptor")),
            }
            if self.structural_values.contains_key(&result.place)
                || self.scalar_array_values.contains_key(&result.place)
                || self.byte_sequence_values.contains_key(&result.place)
                || self.element_view_values.contains_key(&result.place)
                || crate::engine::custody::place_is_claimed(&self.live_claims, result.place)
                || self.live_affine_discard_at(result.place).is_some()
            {
                return Err(
                    malformed().in_reference_formation("element view destination already live")
                );
            }
            let formation_path = &source.path;
            let (backing_path, selected_window) = match source.path.split_last() {
                Some((StructuralPathSegment::FixedRange { start, end }, backing)) => {
                    if region != ElementViewRegion::WholeCollection || start > end {
                        return Err(malformed());
                    }
                    (
                        backing,
                        Some(
                            usize::try_from(*start).map_err(|_| malformed())?
                                ..usize::try_from(*end).map_err(|_| malformed())?,
                        ),
                    )
                }
                _ => (source.path.as_slice(), None),
            };
            let backing_source = terminal_psi::StructuralArgument {
                place: source.place,
                path: backing_path.to_vec(),
                access: source.access,
            };
            let source = &backing_source;
            let root_type = self.live_structural_type(source.place)?;
            let declared_source = match &machine.operation(operation.id).ok_or_else(malformed)?.kind
            {
                OperationKind::EstablishElementView { source, .. }
                | OperationKind::EstablishMutableElementView { source, .. } => source,
                _ => return Err(malformed()),
            };
            if declared_source.place != source.place {
                return Err(malformed());
            }
            let declared_backing_path = match declared_source.path.split_last() {
                Some((StructuralPathSegment::FixedRange { .. }, prefix)) => prefix,
                _ => declared_source.path.as_slice(),
            };
            let collection_shape = crate::engine::custody::resolve_structural_path_shape(
                &self.structural_types(),
                root_type,
                declared_backing_path,
            )
            .map_err(|error| error.in_reference_formation("element view source projection"))?;
            if let ElementViewRegion::WholeRepresentation { source_length, .. } = region {
                let source_length = count(&source_length, &self.values)?;
                self.validate_descriptor_borrow(source, StructuralAccess::SharedBorrow)?;
                let terminal_semantics::ElementViewRegionGeometry::WholeRepresentation {
                    backing,
                    source: source_layout,
                    target: target_layout,
                } = terminal_semantics::element_view_shape_region_geometry(
                    self.structural_types().values(),
                    &collection_shape,
                    element,
                    region,
                    access,
                )
                .ok_or_else(malformed)?
                else {
                    return Err(malformed());
                };
                let captured = match backing.extent {
                    terminal_semantics::CollectionExtent::StoredLength => {
                        let selected = self.resolve_live_projection(
                            source.place,
                            &source.path,
                            source.access,
                        )?;
                        let view = self.element_view_argument(source, &selected)?;
                        view.subslice(0, view.len(), StructuralAccess::SharedBorrow)
                            .ok_or_else(malformed)?
                    }
                    terminal_semantics::CollectionExtent::Fixed(length) => {
                        let CollectionStorage {
                            collection, home, ..
                        } = self.element_collection_storage(source)?;
                        let collection = match collection {
                            Some(collection) => collection,
                            None => {
                                let mut cursor = self.local_structural_identities.cursor();
                                let (root, _) = self.array_storage_root_staged(
                                    source.place,
                                    &mut cursor,
                                    &mut RuntimeRows::new(self.account.as_ref()),
                                )?;
                                let selected = crate::engine::custody::project_structural_value(
                                    &self.structural_types(),
                                    &root,
                                    &source.path,
                                )?;
                                self.local_structural_identities.commit_cursor(cursor);
                                selected
                            }
                        };
                        ElementView::structural(
                            collection,
                            usize::try_from(length).map_err(|_| malformed())?,
                            access,
                            home,
                            source.place,
                        )
                    }
                    _ => return Err(malformed()),
                };
                if captured.len() != source_length {
                    return Err(malformed());
                }
                let representation =
                    self.whole_representation_view(captured, source_layout, target_layout)?;
                let length = representation.length;
                let value = TerminalStructuralValue {
                    opaque_identity: self.local_structural_identities.allocate()?,
                    structural_type: result.structural_type,
                    qualifications: Vec::new(),
                    projected_qualifications: result.projected_qualifications.clone(),
                    path: Vec::new(),
                };
                let view = ElementView {
                    diagnostic_source: source.place,
                    backing: ElementViewBacking::Representation(Arc::new(representation)),
                    window: 0..length,
                    home: None,
                    descriptor_home: Some(value.clone()),
                    access,
                };
                if descriptor.is_borrowed_byte_view() {
                    self.byte_sequence_values.insert(
                    result.place,
                    crate::storage::byte_sequences::binding::ByteSequenceBinding::Representation(
                        Arc::new(view),
                    ),
                )?;
                } else if matches!(descriptor, StructuralTypeShape::ElementView { .. }) {
                    self.element_view_values.insert(result.place, view)?;
                } else {
                    return Err(malformed());
                }
                self.structural_values.insert(result.place, value)?;
                return Ok(OperationFlow::Advance);
            }
            let collection = terminal_semantics::element_view_shape_region_geometry(
                self.structural_types().values(),
                &collection_shape,
                element,
                ElementViewRegion::WholeCollection,
                source.access,
            )
            .and_then(|geometry| match geometry {
                terminal_semantics::ElementViewRegionGeometry::WholeCollection(backing) => {
                    Some(backing)
                }
                terminal_semantics::ElementViewRegionGeometry::RepresentationBytes { .. }
                | terminal_semantics::ElementViewRegionGeometry::WholeRepresentation { .. } => None,
            });
            let extent = collection.map(|backing| backing.extent);
            terminal_semantics::collection_view_descriptor(
                descriptor,
                extent,
                region,
                formation_path,
                self.structural_types().get(&element),
            )
            .ok_or_else(|| {
                malformed().in_reference_formation("element view collection descriptor")
            })?;
            if let Some(window) = &selected_window {
                let StructuralTypeShape::FixedArray {
                    element: source_element,
                    length: terminal_psi::ArrayExtent::Literal(length),
                } = *collection_shape
                else {
                    return Err(malformed());
                };
                if source_element != element
                    || element_scalar
                        != Some(ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?,
                        ))
                    || u64::try_from(window.end).map_err(|_| malformed())? > length
                {
                    return Err(malformed());
                }
            }
            if let ElementViewRegion::RepresentationBytes { start, end, .. } = region {
                let start = count(&start, &self.values)?;
                let end = count(&end, &self.values)?;
                let mut descriptor_value = TerminalStructuralValue {
                    opaque_identity: self.local_structural_identities.allocate()?,
                    structural_type: result.structural_type,
                    qualifications: Vec::new(),
                    projected_qualifications: result.projected_qualifications.clone(),
                    path: Vec::new(),
                };
                if descriptor.is_borrowed_byte_view() {
                    let collection = collection.ok_or_else(malformed)?;
                    let bound = match collection.extent {
                        terminal_semantics::CollectionExtent::Fixed(length) => length,
                        terminal_semantics::CollectionExtent::LiveLength { capacity } => capacity,
                        terminal_semantics::CollectionExtent::StoredLength => {
                            return Err(malformed());
                        }
                    };
                    if start > end || u64::try_from(end).map_err(|_| malformed())? > bound {
                        return Err(malformed());
                    }
                    let mut cursor = self.local_structural_identities.cursor();
                    let (source_value, binding) = self.establish_byte_array_binding(
                        source,
                        &descriptor_value,
                        &collection,
                        &mut cursor,
                    )?;
                    let (value, binding) = self.narrow_byte_sequence_binding(
                        &binding,
                        &source_value,
                        start..end,
                        access,
                        descriptor_value,
                    )?;
                    descriptor_value = value;
                    self.local_structural_identities.commit_cursor(cursor);
                    self.byte_sequence_values.insert(result.place, binding)?;
                } else {
                    if !matches!(descriptor, StructuralTypeShape::ElementView { .. }) {
                        return Err(malformed());
                    }
                    let collection_type = crate::engine::custody::resolve_structural_path_type(
                        &self.structural_types(),
                        root_type,
                        &source.path,
                    )?;
                    let view =
                        self.representation_view(source, collection_type, element, start, end)?;
                    let extent = view.length;
                    self.element_view_values.insert(
                        result.place,
                        ElementView {
                            diagnostic_source: source.place,
                            backing: ElementViewBacking::Representation(Arc::new(view)),
                            window: 0..extent,
                            home: None,
                            descriptor_home: None,
                            access,
                        },
                    )?;
                }
                self.structural_values
                    .insert(result.place, descriptor_value)?;
                self.track_affine_result(result);
                return Ok(OperationFlow::Advance);
            }
            if let StructuralTypeShape::FixedArray {
                element: source_element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } = *collection_shape
                && source_element != element
            {
                if !matches!(descriptor, StructuralTypeShape::ElementView { .. }) {
                    return Err(malformed());
                }
                let typed = self.typed_element_view(source, source_element, element)?;
                let extent = usize::try_from(length).map_err(|_| malformed())?;
                self.element_view_values.insert(
                    result.place,
                    ElementView {
                        diagnostic_source: source.place,
                        backing: ElementViewBacking::Typed(Arc::new(typed)),
                        window: 0..extent,
                        home: None,
                        descriptor_home: None,
                        access,
                    },
                )?;
                self.structural_values.insert(
                    result.place,
                    TerminalStructuralValue {
                        opaque_identity: self.local_structural_identities.allocate()?,
                        structural_type: result.structural_type,
                        qualifications: Vec::new(),
                        projected_qualifications: result.projected_qualifications.clone(),
                        path: Vec::new(),
                    },
                )?;
                self.track_affine_result(result);
                return Ok(OperationFlow::Advance);
            }
            let collection = collection.ok_or_else(|| {
                malformed().in_reference_formation("element view collection geometry")
            })?;
            if collection.storage == terminal_semantics::CollectionStorage::Borrowed
                && !descriptor.is_borrowed_byte_carrier()
                && collection.extent != terminal_semantics::CollectionExtent::StoredLength
            {
                return Err(malformed());
            }
            let borrowed_view = if collection.extent
                == terminal_semantics::CollectionExtent::StoredLength
                && !matches!(descriptor, StructuralTypeShape::ByteSequence(_))
            {
                let resolved =
                    self.resolve_live_projection(source.place, &source.path, source.access)?;
                let view = self.element_view_argument(source, &resolved)?;
                Some(view.subslice(0, view.len(), access).ok_or_else(malformed)?)
            } else {
                None
            };
            let collection_length = match collection.extent {
                terminal_semantics::CollectionExtent::Fixed(length) => length,
                terminal_semantics::CollectionExtent::StoredLength => {
                    if matches!(descriptor, StructuralTypeShape::ByteSequence(_)) {
                        let selected = self.resolve_live_projection(
                            source.place,
                            &source.path,
                            source.access,
                        )?;
                        let binding = self.byte_descriptor_binding(source, &selected)?;
                        self.byte_descriptor_length(&binding, &selected)?
                    } else {
                        u64::try_from(borrowed_view.as_ref().ok_or_else(malformed)?.len())
                            .map_err(|_| malformed())?
                    }
                }
                terminal_semantics::CollectionExtent::LiveLength { capacity }
                    if matches!(descriptor, StructuralTypeShape::ByteSequence(_)) =>
                {
                    capacity
                }
                _ => return Err(malformed()),
            };
            let extent = usize::try_from(collection_length).map_err(|_| malformed())?;
            let descriptor_value = TerminalStructuralValue {
                opaque_identity: self.local_structural_identities.allocate()?,
                structural_type: result.structural_type,
                qualifications: Vec::new(),
                projected_qualifications: result.projected_qualifications.clone(),
                path: Vec::new(),
            };
            match descriptor {
                StructuralTypeShape::ByteSequence(_) => {
                    let mut cursor = self.local_structural_identities.cursor();
                    let (value, binding) = self
                        .establish_byte_array_binding(
                            source,
                            &descriptor_value,
                            &collection,
                            &mut cursor,
                        )
                        .map_err(|error| {
                            error.in_reference_formation("element view byte storage")
                        })?;
                    let (_value, binding) = if let Some(window) = &selected_window {
                        self.narrow_byte_sequence_binding(
                            &binding,
                            &value,
                            window.clone(),
                            access,
                            descriptor_value.clone(),
                        )?
                    } else {
                        (value, binding)
                    };
                    let binding = binding.with_descriptor_home(descriptor_value.clone());
                    self.local_structural_identities.commit_cursor(cursor);
                    self.byte_sequence_values.insert(result.place, binding)?;
                }
                _ => {
                    let mut cursor = self.local_structural_identities.cursor();
                    let view = if let Some(view) = borrowed_view {
                        view.with_descriptor_home(descriptor_value.clone())
                    } else {
                        let CollectionStorage {
                            collection, home, ..
                        } = self.element_collection_storage(source)?;
                        let collection = match collection {
                            Some(collection) => collection,
                            None => {
                                let (root, _) = self.array_storage_root_staged(
                                    source.place,
                                    &mut cursor,
                                    &mut RuntimeRows::new(self.account.as_ref()),
                                )?;
                                crate::engine::custody::project_structural_value(
                                    &self.structural_types(),
                                    &root,
                                    &source.path,
                                )?
                            }
                        };
                        ElementView::structural(collection, extent, access, home, source.place)
                    };
                    let view = if let Some(window) = &selected_window {
                        view.subslice(window.start, window.end, access)
                            .ok_or_else(malformed)?
                    } else {
                        view
                    };
                    self.local_structural_identities.commit_cursor(cursor);
                    self.element_view_values.insert(result.place, view)?;
                }
            }
            self.structural_values
                .insert(result.place, descriptor_value)?;
            self.track_affine_result(result);
            Ok(OperationFlow::Advance)
        }

        pub(crate) fn element_collection_storage(
            &self,
            source: &terminal_psi::StructuralArgument,
        ) -> Result<CollectionStorage, TerminalInterpretError> {
            let root_type = self.live_structural_type(source.place)?;
            let authority = self.reference_carrier_access(source.place, root_type, &source.path)?;
            if !crate::storage::reference::access_covers(authority, source.access) {
                return Err(malformed());
            }
            if self.scalar_array_values.contains_key(&source.place) {
                return Ok(CollectionStorage {
                    collection: None,
                    home: Some(ElementViewHome {
                        storage: self.capture_primitive_home(source.place)?,
                        path: source.path.clone(),
                    }),
                    view: None,
                });
            }
            let selected =
                self.resolve_live_storage_projection(source.place, &source.path, source.access)?;
            let home = match selected.collection_home {
                Some(home) => Some(home),
                None => selected
                    .array_loan
                    .as_ref()
                    .map(|loan| {
                        if !source.access.admits_referent(loan.access()) {
                            return Err(malformed());
                        }
                        loan.captured_storage_home(&self.structural_types(), &[])
                    })
                    .transpose()?
                    .flatten(),
            };
            Ok(CollectionStorage {
                collection: Some(selected.value),
                home,
                view: selected.view_tip,
            })
        }

        fn resident_element_storage(
            &self,
            collection: &TerminalStructuralValue,
            position: usize,
            suffix: &[StructuralPathSegment],
            scalar: ScalarType,
            shared: bool,
        ) -> Result<
            (
                crate::storage::primitive_storage::PrimitiveStorage,
                ScalarType,
                bool,
            ),
            TerminalInterpretError,
        > {
            let mut path = Vec::with_capacity(1 + suffix.len());
            path.push(StructuralPathSegment::FixedIndex(
                u64::try_from(position).map_err(|_| malformed())?,
            ));
            path.extend_from_slice(suffix);
            let selected = self
                .resolve_reference_storage_projection(
                    collection,
                    &path,
                    if shared {
                        StructuralAccess::SharedBorrow
                    } else {
                        StructuralAccess::MutableBorrow
                    },
                )?
                .ok_or_else(malformed)?;
            if let Some(storage) = selected.primitive_storage {
                let StructuralTypeShape::PrimitiveScalar(logical) = self
                    .structural_declaration(selected.value.structural_type)?
                    .shape
                else {
                    return Err(malformed());
                };
                return Ok((storage, logical, shared));
            }
            let canonical = if shared {
                terminal_semantics::canonical_primitive_read_path
            } else {
                terminal_semantics::canonical_primitive_path
            }(
                self.structural_types().values(),
                collection.structural_type,
                &path,
            )
            .ok_or_else(malformed)?;
            self.primitive_view_access(collection, canonical.as_slice(), scalar, shared)
        }

        fn element_scalar_field(
            &self,
            collection: Option<&TerminalStructuralValue>,
            position: usize,
            path: &[StructuralPathSegment],
            scalar_type: ScalarType,
            access: StructuralAccess,
        ) -> Result<Option<(StructuralScalarRuntimeField, ScalarType)>, TerminalInterpretError>
        {
            let selection = [StructuralPathSegment::FixedIndex(
                u64::try_from(position).map_err(|_| malformed())?,
            )];
            self.scalar_field_in_projection(collection, &selection, path, scalar_type, access)
        }

        fn scalar_field_in_projection(
            &self,
            collection: Option<&TerminalStructuralValue>,
            selection: &[StructuralPathSegment],
            path: &[StructuralPathSegment],
            scalar_type: ScalarType,
            access: StructuralAccess,
        ) -> Result<Option<(StructuralScalarRuntimeField, ScalarType)>, TerminalInterpretError>
        {
            let Some((prefix, field_path)) =
                crate::storage::primitive_storage::scalar_field_projection(path)
            else {
                return Ok(None);
            };
            let mut selected_path = Vec::with_capacity(selection.len() + prefix.len());
            selected_path.extend_from_slice(selection);
            selected_path.extend_from_slice(prefix);
            let (parent, view) = self
                .resolve_reference_view_path(
                    collection.ok_or_else(malformed)?,
                    &selected_path,
                    access,
                )?
                .ok_or_else(malformed)?;
            let Some(observed) =
                self.scalar_field_at_carrier(parent, view, field_path, scalar_type)?
            else {
                return Ok(None);
            };
            Ok(Some((observed.cell, observed.stored_type)))
        }

        fn read_structural_element(
            &self,
            view: &ElementView,
            index: usize,
            path: &[StructuralPathSegment],
            scalar_type: ScalarType,
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            let index_path = [StructuralPathSegment::FixedIndex(
                u64::try_from(index).map_err(|_| malformed())?,
            )];
            let (collection, selection) =
                view.structural_projection(&self.structural_types(), &index_path, false)?;
            if let Some((field, stored_type)) = self.scalar_field_in_projection(
                Some(&collection),
                &selection,
                path,
                scalar_type,
                StructuralAccess::SharedBorrow,
            )? {
                let value = self
                    .structural_scalar_fields
                    .get(&field)
                    .map(crate::storage::scalar_storage::ScalarCell::read)
                    .transpose()?
                    .ok_or_else(malformed)?;
                if value.scalar_type() != stored_type {
                    return Err(malformed());
                }
                if stored_type == scalar_type {
                    Ok(value)
                } else {
                    crate::storage::primitive_storage::scalar_reference_observation(
                        value,
                        scalar_type,
                    )
                }
            } else {
                let mut projection = Vec::with_capacity(1 + path.len());
                projection.extend_from_slice(&index_path);
                projection.extend_from_slice(path);
                let (storage, logical, shared) = view.scalar_storage(self, &projection, false)?;
                self.read_primitive_storage(
                    view.diagnostic_source,
                    storage,
                    logical,
                    scalar_type,
                    shared,
                )
            }
        }

        fn write_structural_element(
            &mut self,
            view: &ElementView,
            index: usize,
            path: &[StructuralPathSegment],
            scalar: TerminalScalarValue,
        ) -> Result<(), TerminalInterpretError> {
            let ElementViewBacking::Structural { collection } = &view.backing else {
                return Err(malformed());
            };
            let position = view.window.start.checked_add(index).ok_or_else(malformed)?;
            if let Some((field, stored_type)) = self.element_scalar_field(
                Some(collection),
                position,
                path,
                scalar.scalar_type(),
                StructuralAccess::MutableBorrow,
            )? {
                if stored_type != scalar.scalar_type() {
                    return Err(malformed());
                }
                let stored = self
                    .structural_scalar_fields
                    .get_mut(&field)
                    .ok_or_else(malformed)?;
                if stored.scalar_type() != scalar.scalar_type() {
                    return Err(malformed());
                }
                stored.write(scalar)?;
                Ok(())
            } else {
                if let Some(home) = &view.home {
                    self.store_primitive_home(
                        home.storage,
                        &home.selected_path(position, path)?,
                        scalar,
                    )
                } else {
                    let (storage, logical, _) = self.resident_element_storage(
                        collection,
                        position,
                        path,
                        scalar.scalar_type(),
                        false,
                    )?;
                    self.store_primitive_storage(view.diagnostic_source, storage, logical, scalar)
                }
            }
        }

        pub(crate) fn execute_element_view_write(
            &mut self,
            operation: &Operation,
        ) -> Result<OperationFlow, TerminalInterpretError> {
            operation_kind!(operation => ElementViewWrite {
                destination, index, value, length, path, ..
            });
            if operation.result != OperationResult::Unit {
                return Err(malformed());
            }
            let index = count(index, &self.values)?;
            let length = count(length, &self.values)?;
            let element = self.scalar_value(*value)?;
            if !terminal_scalar_belongs_to_type(element) {
                return Err(malformed());
            }
            self.validate_descriptor_borrow(destination, StructuralAccess::MutableBorrow)
                .map_err(|error| error.in_argument_preparation("authorize element view write"))?;
            let selected = self
                .resolve_live_projection(
                    destination.place,
                    &destination.path,
                    StructuralAccess::MutableBorrow,
                )
                .map_err(|error| {
                    error.in_argument_preparation("resolve element view write descriptor")
                })?;
            let view = self
                .element_view_argument(destination, &selected)
                .map_err(|error| {
                    error.in_argument_preparation("capture element view write descriptor")
                })?
                .clone();
            if view.access != StructuralAccess::MutableBorrow
                || view.len() != length
                || index >= length
            {
                return Err(malformed());
            }
            match &view.backing {
                ElementViewBacking::ImmutableSequence(_) => return Err(malformed()),
                ElementViewBacking::Typed(_) => self
                    .write_typed_element(&view, index, path, element)
                    .map_err(|error| {
                        error.in_argument_preparation("write typed element backing")
                    })?,
                ElementViewBacking::Structural { .. } => self
                    .write_structural_element(&view, index, path, element)
                    .map_err(|error| {
                        error.in_argument_preparation("write structural element backing")
                    })?,
                ElementViewBacking::Representation(_) => self
                    .write_representation_element(&view, index, path, element)
                    .map_err(|error| {
                        error.in_argument_preparation("write representation element backing")
                    })?,
            }
            Ok(OperationFlow::Advance)
        }

        pub(crate) fn execute_element_view_length(
            &mut self,
            operation: &Operation,
        ) -> Result<OperationFlow, TerminalInterpretError> {
            operation_kind!(operation => ElementViewLength { source });
            let result = operation.result.scalar().ok_or_else(malformed)?;
            let integer_type =
                IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| malformed())?;
            if result.scalar_type != ScalarType::Integer(integer_type) {
                return Err(malformed());
            }
            let metadata_access = if source.access == StructuralAccess::WriteOnlyBorrow {
                if source
                    .path
                    .iter()
                    .any(|segment| matches!(segment, StructuralPathSegment::Referent))
                {
                    return Err(malformed());
                }
                StructuralAccess::WriteOnlyBorrow
            } else {
                self.validate_descriptor_borrow(source, StructuralAccess::SharedBorrow)?;
                StructuralAccess::SharedBorrow
            };
            let selected =
                self.resolve_live_projection(source.place, &source.path, metadata_access)?;
            if !self
                .structural_types()
                .get(&selected.structural_type)
                .is_some_and(|declaration| {
                    matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
                })
            {
                return Err(malformed());
            }
            let length = self.element_view_argument(source, &selected)?.len();
            self.values.insert(
                result.id,
                TerminalScalarValue::Integer {
                    scalar_type: integer_type,
                    value: IntegerValue::Unsigned(u128::try_from(length).map_err(|_| malformed())?),
                },
            )?;
            Ok(OperationFlow::Advance)
        }

        pub(crate) fn read_element_view_scalar(
            &self,
            view: &ElementView,
            index: usize,
            path: &[StructuralPathSegment],
            expected: ScalarType,
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            if !crate::storage::reference::access_covers(
                view.access(),
                StructuralAccess::SharedBorrow,
            ) || index >= view.len()
            {
                return Err(malformed());
            }
            let element = match &view.backing {
                ElementViewBacking::Typed(_) => {
                    self.read_typed_element(view, index, path, expected)?
                }
                ElementViewBacking::Representation(_) => {
                    self.read_representation_element(view, index, path, expected)?
                }
                ElementViewBacking::Structural { .. }
                | ElementViewBacking::ImmutableSequence(_) => {
                    self.read_structural_element(view, index, path, expected)?
                }
            };
            if element.scalar_type() != expected || !terminal_scalar_belongs_to_type(element) {
                return Err(malformed());
            }
            Ok(element)
        }

        pub(crate) fn execute_element_view_read(
            &mut self,
            operation: &Operation,
        ) -> Result<OperationFlow, TerminalInterpretError> {
            operation_kind!(operation => ElementViewRead { source, index, length, path, .. });
            let result = operation.result.scalar().ok_or_else(malformed)?;
            let index = count(index, &self.values)?;
            let length = count(length, &self.values)?;
            let view = crate::engine::custody::verified_structural_place(
                &self.element_view_values,
                *source,
            )?;
            if view.len() != length || index >= length {
                return Err(malformed());
            }
            let element = self.read_element_view_scalar(view, index, path, result.scalar_type)?;
            self.values.insert(result.id, element)?;
            Ok(OperationFlow::Advance)
        }

        pub(crate) fn execute_element_view_subslice(
            &mut self,
            operation: &Operation,
        ) -> Result<(), TerminalInterpretError> {
            operation_kind!(operation => ElementViewSubslice { source, start, end, length, .. });
            let result = operation.result.structural().ok_or_else(malformed)?;
            let start = count(start, &self.values)?;
            let end = count(end, &self.values)?;
            let length = count(length, &self.values)?;
            let access = match result.multiplicity {
                StructuralMultiplicity::Unrestricted => StructuralAccess::SharedBorrow,
                StructuralMultiplicity::Affine => StructuralAccess::MutableBorrow,
                StructuralMultiplicity::Linear => return Err(malformed()),
            };
            self.validate_descriptor_borrow(source, access)?;
            let source_value = self.resolve_live_projection(source.place, &source.path, access)?;
            let view = self.element_view_argument(source, &source_value)?.clone();
            let machine = self.program.verified_machine(self.current_machine)?;
            let types = self.structural_types();
            let same_element = types
                .get(&source_value.structural_type)
                .zip(types.get(&result.structural_type))
                .is_some_and(|(source, result)| {
                    matches!((&source.shape, &result.shape),
                (StructuralTypeShape::ElementView { element: source, .. },
                 StructuralTypeShape::ElementView { element: result, access: ceiling })
                    if source == result && ceiling.is_none_or(|ceiling| ceiling == access))
                });
            if result.place == source.place
                || !machine.structural_places.iter().any(|place| {
                    place.id == result.place
                        && place.kind
                            == StructuralPlaceKind::OperationResult {
                                producer: operation.id,
                                structural_type: result.structural_type,
                            }
                })
                || view.len() != length
                || !same_element
                || !crate::storage::reference::access_covers(view.access, access)
                || !source_value.qualifications.is_empty()
                || (view.len() != 0
                    && result
                        .projected_qualifications
                        .iter()
                        .any(|row| !source_value.projected_qualifications.contains(row)))
                || !result.qualifications.is_empty()
                || result
                    .projected_qualifications
                    .iter()
                    .any(|row| !row.is_element_membership())
                || !result.qualification_establishments.is_empty()
                || !self
                    .structural_types()
                    .get(&result.structural_type)
                    .is_some_and(|row| matches!(row.shape, StructuralTypeShape::ElementView { .. }))
                || !result.claims.is_empty()
                || crate::engine::custody::place_is_claimed(&self.live_claims, source.place)
                || crate::engine::custody::place_is_claimed(&self.live_claims, result.place)
                || self.live_affine_discard_at(result.place).is_some()
            {
                return Err(malformed());
            }
            match (
                self.structural_values.get(&result.place),
                self.element_view_values.get(&result.place),
            ) {
                (None, None) => {}
                (Some(previous), Some(previous_view))
                    if previous.structural_type == result.structural_type
                        && previous.qualifications.is_empty()
                        && previous.projected_qualifications == result.projected_qualifications
                        && previous.path.is_empty()
                        && previous_view.matches_bound_descriptor_home(previous, previous) => {}
                _ => return Err(malformed()),
            }
            let mut identity_cursor = self.local_structural_identities.cursor();
            let mut destination = self.mint_staged(&mut identity_cursor, result.structural_type)?;
            destination.projected_qualifications = result.projected_qualifications.clone();
            let window = view
                .subslice(start, end, access)
                .ok_or_else(malformed)?
                .with_descriptor_home(destination.clone());
            self.local_structural_identities
                .commit_cursor(identity_cursor);
            self.element_view_values.insert(result.place, window)?;
            self.structural_values.insert(result.place, destination)?;
            self.track_affine_result(result);
            Ok(())
        }
    }

    pub(crate) mod representation {
        use super::{ElementView, ElementViewBacking};
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::structural_inputs::byte_arrays::storage::ArrayStorage;
        use crate::boundary::values::{StructuralRuntimePlace, TerminalScalarValue};
        use crate::engine::custody::resolve_single_structural_argument;
        use crate::engine::execution::TerminalExecution;
        use crate::storage::byte_sequences::{ByteSequenceView, binding::ByteSequenceBinding};
        use crate::storage::runtime_rows::RuntimeRows;
        use semantic_vocabulary::{
            IeeeFloatFormat, IntegerSign, IntegerType, IntegerValue, ScalarType, StructuralTypeId,
        };
        use terminal_psi::{
            StructuralAccess, StructuralArgument, StructuralPathSegment, StructuralTypeShape,
        };
        use terminal_semantics::{
            RepresentationElementLayout, RepresentationScalarLeaf, ScalarViewValue,
        };

        pub(crate) struct RepresentationView {
            storage: ByteStorage,
            start: usize,
            pub(crate) length: usize,
            layout: RepresentationElementLayout,
        }

        enum ByteStorage {
            Array {
                storage: ArrayStorage,
                offset: usize,
            },
            Immutable(ByteSequenceView),
            Typed {
                source: ElementView,
                layout: RepresentationElementLayout,
            },
        }

        impl RepresentationView {
            pub(crate) fn retain_storage(
                &self,
                retained: &mut crate::storage::structural_storage::RetainedStorage,
            ) {
                match &self.storage {
                    ByteStorage::Array { storage, .. } => retained.array_storage(storage),
                    ByteStorage::Typed { source, .. } => source.retain_storage(retained),
                    ByteStorage::Immutable(_) => {}
                }
            }

            pub(crate) fn retained_bytes(&self) -> Option<&ByteSequenceView> {
                match &self.storage {
                    ByteStorage::Immutable(view) => Some(view),
                    ByteStorage::Array { .. } => None,
                    ByteStorage::Typed { source, .. } => source.retained_bytes(),
                }
            }

            fn scalar_position<'a>(
                &'a self,
                view: &ElementView,
                index: usize,
                path: &[StructuralPathSegment],
                expected: ScalarType,
            ) -> Result<(usize, &'a RepresentationScalarLeaf), TerminalInterpretError> {
                if index >= view.len() {
                    return Err(malformed());
                }
                let leaf = self
                    .layout
                    .leaves
                    .iter()
                    .find(|leaf| leaf.path == path && leaf.scalar_type == expected)
                    .ok_or_else(malformed)?;
                let element = view
                    .window
                    .start
                    .checked_add(index)
                    .filter(|index| *index < self.length)
                    .ok_or_else(malformed)?;
                let stride = usize::try_from(self.layout.size).map_err(|_| malformed())?;
                let offset = usize::try_from(leaf.offset).map_err(|_| malformed())?;
                let width = scalar_width(expected)?;
                if offset.checked_add(width).is_none_or(|end| end > stride) {
                    return Err(malformed());
                }
                let position = element
                    .checked_mul(stride)
                    .and_then(|offset| self.start.checked_add(offset))
                    .and_then(|position| position.checked_add(offset))
                    .ok_or_else(malformed)?;
                Ok((position, leaf))
            }
        }

        impl TerminalExecution<'_> {
            pub(crate) fn representation_view(
                &mut self,
                source: &StructuralArgument,
                collection: StructuralTypeId,
                element: StructuralTypeId,
                start: usize,
                end: usize,
            ) -> Result<RepresentationView, TerminalInterpretError> {
                let extent = terminal_semantics::representation_byte_extent(
                    self.structural_types().values(),
                    collection,
                )
                .and_then(|extent| usize::try_from(extent).ok())
                .ok_or_else(malformed)?;
                let layout = terminal_semantics::representation_element_layout(
                    self.structural_types().values(),
                    element,
                )
                .ok_or_else(malformed)?;
                let stride = usize::try_from(layout.size).map_err(|_| malformed())?;
                if start > end
                    || end > extent
                    || stride == 0
                    || !(end - start).is_multiple_of(stride)
                {
                    return Err(malformed());
                }
                let storage = if matches!(
                    self.structural_types()
                        .get(&collection)
                        .map(|row| &row.shape),
                    Some(StructuralTypeShape::ByteSequence(_))
                ) {
                    if !source.path.is_empty() {
                        return Err(malformed());
                    }
                    let value = self.structural_value(source.place)?;
                    if !value.qualifications.is_empty()
                        || self.byte_sequence_length(source.place)? != extent as u64
                    {
                        return Err(malformed());
                    }
                    match self.byte_sequence_values.require(&source.place)? {
                        binding @ ByteSequenceBinding::MutableArray { array, offset, .. } => {
                            binding.validate_mutable_referent(self, value)?;
                            ByteStorage::Array {
                                storage: array.clone(),
                                offset: usize::try_from(*offset).map_err(|_| malformed())?,
                            }
                        }
                        ByteSequenceBinding::Immutable(bytes)
                            if source.access == StructuralAccess::SharedBorrow =>
                        {
                            ByteStorage::Immutable(bytes.clone())
                        }
                        _ => return Err(malformed()),
                    }
                } else {
                    let mut cursor = self.local_structural_identities.cursor();
                    let (root, home) = self.array_storage_root_staged(
                        source.place,
                        &mut cursor,
                        &mut RuntimeRows::new(self.account.as_ref()),
                    )?;
                    let values =
                        RuntimeRows::from_rows(self.account.as_ref(), [(source.place, root)])?;
                    let referent = resolve_single_structural_argument(
                        &self.structural_types(),
                        &values,
                        source,
                    )?;
                    if referent.structural_type != collection || !referent.qualifications.is_empty()
                    {
                        return Err(malformed());
                    }
                    let storage = ArrayStorage {
                        referent: StructuralRuntimePlace::from(&referent),
                        home,
                    };
                    if self.stored_byte_array_length(&storage)? != extent {
                        return Err(malformed());
                    }
                    self.local_structural_identities.commit_cursor(cursor);
                    ByteStorage::Array { storage, offset: 0 }
                };
                Ok(RepresentationView {
                    storage,
                    start,
                    length: (end - start) / stride,
                    layout,
                })
            }

            pub(crate) fn read_representation_element(
                &self,
                view: &ElementView,
                index: usize,
                path: &[StructuralPathSegment],
                expected: ScalarType,
            ) -> Result<TerminalScalarValue, TerminalInterpretError> {
                let ElementViewBacking::Representation(representation) = &view.backing else {
                    return Err(malformed());
                };
                let (position, _) = representation.scalar_position(view, index, path, expected)?;
                let mut bits = 0_u64;
                for byte_index in 0..scalar_width(expected)? {
                    let index = position.checked_add(byte_index).ok_or_else(malformed)?;
                    let byte = self.representation_storage_byte(&representation.storage, index)?;
                    bits |= u64::from(byte) << (byte_index * 8);
                }
                decode_scalar(expected, bits)
            }

            fn representation_storage_byte(
                &self,
                storage: &ByteStorage,
                index: usize,
            ) -> Result<u8, TerminalInterpretError> {
                match storage {
                    ByteStorage::Array { storage, offset } => self.stored_byte_array_read(
                        storage,
                        offset.checked_add(index).ok_or_else(malformed)?,
                    ),
                    ByteStorage::Immutable(bytes) => {
                        bytes.get(index).copied().ok_or_else(malformed)
                    }
                    ByteStorage::Typed { source, layout } => {
                        let stride = usize::try_from(layout.size).map_err(|_| malformed())?;
                        if stride == 0 {
                            return Err(malformed());
                        }
                        let element = index / stride;
                        let offset = index % stride;
                        let leaf = layout
                            .leaves
                            .iter()
                            .find(|leaf| {
                                usize::try_from(leaf.offset)
                                    .ok()
                                    .zip(scalar_width(leaf.scalar_type).ok())
                                    .is_some_and(|(start, width)| {
                                        offset >= start && offset - start < width
                                    })
                            })
                            .ok_or_else(malformed)?;
                        let scalar = self.read_element_view_scalar(
                            source,
                            element,
                            &leaf.path,
                            leaf.scalar_type,
                        )?;
                        let shift =
                            offset - usize::try_from(leaf.offset).map_err(|_| malformed())?;
                        Ok((encode_scalar(scalar)? >> (shift * 8)) as u8)
                    }
                }
            }

            pub(crate) fn whole_representation_view(
                &self,
                source: ElementView,
                source_layout: RepresentationElementLayout,
                target_layout: RepresentationElementLayout,
            ) -> Result<RepresentationView, TerminalInterpretError> {
                if source.access() != StructuralAccess::SharedBorrow
                    || source_layout.size == 0
                    || target_layout.size == 0
                    || !source_layout.size.is_multiple_of(target_layout.size)
                {
                    return Err(malformed());
                }
                let ratio = usize::try_from(source_layout.size / target_layout.size)
                    .map_err(|_| malformed())?;
                let length = source.len().checked_mul(ratio).ok_or_else(malformed)?;
                Ok(RepresentationView {
                    storage: ByteStorage::Typed {
                        source: source.detached_descriptor(),
                        layout: source_layout,
                    },
                    start: 0,
                    length,
                    layout: target_layout,
                })
            }

            pub(crate) fn write_representation_element(
                &mut self,
                view: &ElementView,
                index: usize,
                path: &[StructuralPathSegment],
                scalar: TerminalScalarValue,
            ) -> Result<(), TerminalInterpretError> {
                if view.access != StructuralAccess::MutableBorrow {
                    return Err(malformed());
                }
                let ElementViewBacking::Representation(representation) = &view.backing else {
                    return Err(malformed());
                };
                let (position, _) =
                    representation.scalar_position(view, index, path, scalar.scalar_type())?;
                let ByteStorage::Array { storage, offset } = &representation.storage else {
                    return Err(malformed());
                };
                let position = offset.checked_add(position).ok_or_else(malformed)?;
                let width = scalar_width(scalar.scalar_type())?;
                let extent = self.stored_byte_array_length(storage)?;
                if position.checked_add(width).is_none_or(|end| end > extent) {
                    return Err(malformed());
                }
                let bits = encode_scalar(scalar)?;
                for byte_index in 0..width {
                    self.stored_byte_array_write(
                        storage,
                        position + byte_index,
                        (bits >> (byte_index * 8)) as u8,
                    )?;
                }
                Ok(())
            }
        }

        fn scalar_width(scalar: ScalarType) -> Result<usize, TerminalInterpretError> {
            match scalar {
                ScalarType::Integer(integer)
                    if !integer.is_address() && matches!(integer.bits(), 8 | 16 | 32 | 64) =>
                {
                    Ok(usize::from(integer.bits() / 8))
                }
                ScalarType::IeeeFloat(IeeeFloatFormat::Binary32) => Ok(4),
                ScalarType::IeeeFloat(IeeeFloatFormat::Binary64) => Ok(8),
                _ => Err(malformed()),
            }
        }
        fn bit_carrier(scalar: ScalarType) -> Result<ScalarType, TerminalInterpretError> {
            let bits = u16::try_from(scalar_width(scalar)? * 8).map_err(|_| malformed())?;
            Ok(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, bits).map_err(|_| malformed())?,
            ))
        }

        fn decode_scalar(
            scalar: ScalarType,
            bits: u64,
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            let viewed = terminal_semantics::scalar_reference_view_value(
                bit_carrier(scalar)?,
                scalar,
                ScalarViewValue::Integer(IntegerValue::Unsigned(u128::from(bits))),
            )
            .ok_or_else(malformed)?;
            match (scalar, viewed) {
                (ScalarType::Integer(scalar_type), ScalarViewValue::Integer(value)) => {
                    Ok(TerminalScalarValue::Integer { scalar_type, value })
                }
                (ScalarType::IeeeFloat(_), ScalarViewValue::IeeeFloat(value)) => {
                    Ok(TerminalScalarValue::IeeeFloat(value))
                }
                _ => Err(malformed()),
            }
        }

        fn encode_scalar(scalar: TerminalScalarValue) -> Result<u64, TerminalInterpretError> {
            let value = match scalar {
                TerminalScalarValue::Integer { value, .. } => ScalarViewValue::Integer(value),
                TerminalScalarValue::IeeeFloat(value) => ScalarViewValue::IeeeFloat(value),
                _ => return Err(malformed()),
            };
            let viewed = terminal_semantics::scalar_reference_view_value(
                scalar.scalar_type(),
                bit_carrier(scalar.scalar_type())?,
                value,
            )
            .ok_or_else(malformed)?;
            let ScalarViewValue::Integer(IntegerValue::Unsigned(bits)) = viewed else {
                return Err(malformed());
            };
            u64::try_from(bits).map_err(|_| malformed())
        }
    }
    pub(crate) mod typed {
        use super::{CollectionStorage, ElementView, ElementViewBacking, ElementViewHome};
        use crate::boundary::errors::{TerminalInterpretError, malformed};
        use crate::boundary::values::{TerminalScalarValue, TerminalStructuralValue};
        use crate::engine::execution::TerminalExecution;
        use semantic_vocabulary::{ScalarType, StructuralTypeId};
        use terminal_psi::{StructuralAccess, StructuralArgument, StructuralPathSegment};
        use terminal_semantics::{
            ElementRepresentationLeaf, ElementRepresentationRelation, ScalarRepresentationViewPair,
            ScalarViewValue, scalar_reference_view_value,
        };

        pub(crate) struct TypedElementView {
            collection: Option<TerminalStructuralValue>,
            home: Option<ElementViewHome>,
            pub(crate) relation: ElementRepresentationRelation,
        }

        impl TypedElementView {
            pub(crate) fn retain_storage(
                &self,
                retained: &mut crate::storage::structural_storage::RetainedStorage,
            ) {
                if let Some(home) = &self.home {
                    retained.primitive_home(home.storage);
                }
            }

            fn selected<'a>(
                &'a self,
                view: &ElementView,
                index: usize,
                path: &[StructuralPathSegment],
                scalar: ScalarType,
            ) -> Result<
                (
                    usize,
                    &'a ElementRepresentationLeaf,
                    &'a ElementRepresentationLeaf,
                ),
                TerminalInterpretError,
            > {
                if index >= view.len() {
                    return Err(malformed());
                }
                let position = view.window.start.checked_add(index).ok_or_else(malformed)?;
                let mut matching = self
                    .relation
                    .source_layout
                    .leaves
                    .iter()
                    .zip(&self.relation.target_layout.leaves)
                    .filter(|(_, target)| target.path == path && target.scalar_type == scalar);
                let (Some((source, target)), None) = (matching.next(), matching.next()) else {
                    return Err(malformed());
                };
                Ok((position, source, target))
            }
        }

        fn checked_value(value: TerminalScalarValue, leaf: &ElementRepresentationLeaf) -> bool {
            value.scalar_type() == leaf.scalar_type && crate::engine::scalar_operations::terminal_scalar_belongs_to_type(value)
        && leaf.bounds.is_none_or(|bounds| matches!(value, TerminalScalarValue::Integer { value, .. } if bounds.contains(value)))
        }

        fn reinterpret(
            value: TerminalScalarValue,
            target: ScalarType,
        ) -> Result<TerminalScalarValue, TerminalInterpretError> {
            if value.scalar_type() == target {
                return Ok(value);
            }
            let input = crate::boundary::values::scalar_view_value(&value).ok_or_else(malformed)?;
            let output = scalar_reference_view_value(value.scalar_type(), target, input)
                .or_else(|| {
                    ScalarRepresentationViewPair::classify(value.scalar_type(), target)?.view(input)
                })
                .ok_or_else(malformed)?;
            match (target, output) {
                (ScalarType::Boolean, ScalarViewValue::Boolean(value)) => {
                    Ok(TerminalScalarValue::Boolean(value))
                }
                (ScalarType::Integer(scalar_type), ScalarViewValue::Integer(value)) => {
                    Ok(TerminalScalarValue::Integer { scalar_type, value })
                }
                (ScalarType::IeeeFloat(_), ScalarViewValue::IeeeFloat(value)) => {
                    Ok(TerminalScalarValue::IeeeFloat(value))
                }
                _ => Err(malformed()),
            }
        }

        impl TerminalExecution<'_> {
            pub(crate) fn typed_element_view(
                &self,
                source: &StructuralArgument,
                source_element: StructuralTypeId,
                target_element: StructuralTypeId,
            ) -> Result<TypedElementView, TerminalInterpretError> {
                let relation = terminal_semantics::element_representation_relation(
                    self.structural_types().values(),
                    source_element,
                    target_element,
                    source.access,
                )
                .ok_or_else(malformed)?;
                let CollectionStorage {
                    collection, home, ..
                } = self.element_collection_storage(source)?;
                Ok(TypedElementView {
                    collection,
                    home,
                    relation,
                })
            }

            pub(crate) fn read_typed_element(
                &self,
                view: &ElementView,
                index: usize,
                path: &[StructuralPathSegment],
                expected: ScalarType,
            ) -> Result<TerminalScalarValue, TerminalInterpretError> {
                let ElementViewBacking::Typed(typed) = &view.backing else {
                    return Err(malformed());
                };
                let (position, source, target) = typed.selected(view, index, path, expected)?;
                let value = if let Some((field, stored_type)) = self.element_scalar_field(
                    typed.collection.as_ref(),
                    position,
                    &source.path,
                    source.scalar_type,
                    StructuralAccess::SharedBorrow,
                )? {
                    let value = self
                        .structural_scalar_fields
                        .get(&field)
                        .map(crate::storage::scalar_storage::ScalarCell::read)
                        .transpose()?
                        .ok_or_else(malformed)?;
                    if value.scalar_type() != stored_type {
                        return Err(malformed());
                    }
                    if stored_type == source.scalar_type {
                        value
                    } else {
                        crate::storage::primitive_storage::scalar_reference_observation(
                            value,
                            source.scalar_type,
                        )?
                    }
                } else {
                    if let Some(home) = &typed.home {
                        let source_path = home.selected_path(position, &source.path)?;
                        self.read_primitive_home(home.storage, &source_path, source.scalar_type)?
                    } else {
                        let (storage, logical, shared) = self.resident_element_storage(
                            typed.collection.as_ref().ok_or_else(malformed)?,
                            position,
                            &source.path,
                            source.scalar_type,
                            true,
                        )?;
                        self.read_primitive_storage(
                            view.diagnostic_source,
                            storage,
                            logical,
                            source.scalar_type,
                            shared,
                        )?
                    }
                };
                if !checked_value(value, source) {
                    return Err(malformed());
                }
                let result = reinterpret(value, target.scalar_type)?;
                if !checked_value(result, target) {
                    return Err(malformed());
                }
                Ok(result)
            }

            pub(crate) fn write_typed_element(
                &mut self,
                view: &ElementView,
                index: usize,
                path: &[StructuralPathSegment],
                value: TerminalScalarValue,
            ) -> Result<(), TerminalInterpretError> {
                if view.access != StructuralAccess::MutableBorrow {
                    return Err(malformed());
                }
                let ElementViewBacking::Typed(typed) = &view.backing else {
                    return Err(malformed());
                };
                let (position, source, target) =
                    typed.selected(view, index, path, value.scalar_type())?;
                if !checked_value(value, target) {
                    return Err(malformed());
                }
                let value = reinterpret(value, source.scalar_type)?;
                if !checked_value(value, source) {
                    return Err(malformed());
                }
                if let Some((field, stored_type)) = self.element_scalar_field(
                    typed.collection.as_ref(),
                    position,
                    &source.path,
                    source.scalar_type,
                    StructuralAccess::MutableBorrow,
                )? {
                    if stored_type != source.scalar_type {
                        return Err(malformed());
                    }
                    let stored = self
                        .structural_scalar_fields
                        .get_mut(&field)
                        .ok_or_else(malformed)?;
                    if !checked_value(stored.read()?, source) {
                        return Err(malformed());
                    }
                    stored.write(value)?;
                    Ok(())
                } else {
                    if let Some(home) = &typed.home {
                        let source_path = home.selected_path(position, &source.path)?;
                        self.store_primitive_home(home.storage, &source_path, value)
                    } else {
                        let (storage, logical, _) = self.resident_element_storage(
                            typed.collection.as_ref().ok_or_else(malformed)?,
                            position,
                            &source.path,
                            source.scalar_type,
                            false,
                        )?;
                        self.store_primitive_storage(
                            view.diagnostic_source,
                            storage,
                            logical,
                            value,
                        )
                    }
                }
            }
        }
    }

    #[cfg(test)]
    pub(crate) mod tests {
        use super::ElementView;
        use crate::boundary::values::TerminalStructuralValue;
        use semantic_vocabulary::{PlaceId, StructuralDomainId, StructuralTypeId};
        use terminal_psi::{StructuralAccess, StructuralPathQualification, StructuralPathSegment};

        #[test]
        fn descriptor_home_survives_checked_membership_changes() {
            let original =
                TerminalStructuralValue::minted(17, StructuralTypeId::new(3).unwrap(), Vec::new());
            let view = ElementView::structural(
                original.clone(),
                4,
                StructuralAccess::MutableBorrow,
                None,
                PlaceId::new(9).unwrap(),
            )
            .with_descriptor_home(original.clone());
            let mut qualified = original.clone();
            qualified
                .qualifications
                .push(StructuralDomainId::new(2).unwrap());
            qualified
                .projected_qualifications
                .push(StructuralPathQualification {
                    path: vec![StructuralPathSegment::FixedIndex(1)],
                    domain: StructuralDomainId::new(5).unwrap(),
                });
            assert!(view.matches_descriptor_home(&qualified));
            assert!(view.matches_bound_descriptor_home(&original, &qualified));
            assert!(ElementView::same_descriptor_home(&qualified, &original));
            assert!(original.qualifications.is_empty());
            assert!(original.projected_qualifications.is_empty());
        }

        #[test]
        fn descriptor_home_keeps_occurrence_path_and_carrier_distinct() {
            let original =
                TerminalStructuralValue::minted(17, StructuralTypeId::new(3).unwrap(), Vec::new());
            let view = ElementView::structural(
                original.clone(),
                4,
                StructuralAccess::MutableBorrow,
                None,
                PlaceId::new(9).unwrap(),
            )
            .with_descriptor_home(original.clone());
            let mut another = original.clone();
            another.opaque_identity += 1;
            assert!(!view.matches_descriptor_home(&another));
            assert!(!view.matches_bound_descriptor_home(&original, &another));
            let mut projected = original.clone();
            projected.path.push(StructuralPathSegment::FixedIndex(0));
            assert!(!view.matches_descriptor_home(&projected));
            let mut recast = original.clone();
            recast.structural_type = StructuralTypeId::new(4).unwrap();
            assert!(!view.matches_descriptor_home(&recast));
        }
    }
}
