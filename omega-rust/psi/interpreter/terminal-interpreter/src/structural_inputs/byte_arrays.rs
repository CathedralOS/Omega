//! Explicit initialized fixed-array inputs and their fieldless original backing.
pub(crate) mod storage;

use crate::byte_sequences::ByteSequenceView;
use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::custody::resolve_single_structural_argument;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::StructuralRuntimePlace;
use crate::values::TerminalStructuralValue;
use semantic_vocabulary::ScalarType;
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeMap;
use storage::{ArrayStorage, ArrayStorageHome};
use terminal_psi::StructuralAccess;
use terminal_psi::StructuralArgument;
use terminal_psi::StructuralParameterDeclaration;
use terminal_psi::StructuralPathSegment;
use terminal_psi::StructuralTypeDeclaration;
use terminal_psi::StructuralTypeShape;

/// Initialized contents of a true fixed u8 array reachable from an entry input.
/// The static field/index path is relative to the dense structural argument. Exactly the declared
/// number of bytes is required, including an established empty backing;
/// missing contents are never initialized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalStructuralByteArrayValue {
    pub argument_index: u32,
    pub path: Vec<StructuralPathSegment>,
    pub bytes: Vec<u8>,
}

pub(crate) fn byte_array_length(
    types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    array: StructuralTypeId,
) -> Option<u64> {
    // A bounded view's capacity is the fixed byte extent; its element is the
    // implicit u8, so no element declaration is needed.
    if let StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedView {
        capacity,
        ..
    }) = types.get(&array)?.shape
    {
        return Some(capacity);
    }
    let StructuralTypeShape::FixedArray { element, length } = types.get(&array)?.shape else {
        return None;
    };
    let StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(element)) =
        types.get(&element)?.shape
    else {
        return None;
    };
    (!element.is_address()
        && element.sign() == semantic_vocabulary::IntegerSign::Unsigned
        && element.bits() == 8)
        .then_some(length)
}

/// Type geometry for an initialized array selection. Case activity, live
/// backing and loan access remain with the execution and custody owners.
pub(crate) fn array_path_type(
    types: &crate::program::ProgramDeclarations<'_, StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    terminal_semantics::runtime_structural_path_tip(types.values(), structural_type, path)
        .map(|declaration| declaration.id)
}

impl TerminalExecution<'_> {
    fn check_byte_view_handback(
        &self,
        value: &TerminalStructuralValue,
        argument: &StructuralArgument,
        parameter: &StructuralParameterDeclaration,
    ) -> Result<(), TerminalInterpretError> {
        if !value.projected_qualifications.is_empty() {
            return Err(malformed());
        }
        let domains = self.program.catalogs.structural_domains.iter();
        let original = terminal_semantics::byte_qualification_predicates_in(
            domains.clone(),
            value.structural_type,
            &value.qualifications,
        )
        .ok_or_else(malformed)?;
        let required = terminal_semantics::byte_parameter_predicates_in(domains, parameter)
            .ok_or_else(malformed)?;
        if !original.implies(required)
            || (argument.access == StructuralAccess::MutableBorrow && !required.implies(original))
        {
            return Err(malformed());
        }
        Ok(())
    }

    pub(crate) fn bind_byte_arrays(
        &mut self,
        arguments: &[TerminalStructuralByteArrayValue],
    ) -> Result<(), TerminalInterpretError> {
        let machine = self
            .program
            .machines()
            .get(&self.current_machine)
            .ok_or_else(malformed)?;
        let mut storage = BTreeMap::new();
        let mut descriptors = BTreeMap::new();
        for argument in arguments {
            // External byte arrays name physical initialized cells. Unlike
            // internal checked projections, this envelope carries no case
            // discriminator, reference resolution, or captured selector.
            if !argument.path.iter().all(|segment| {
                matches!(
                    segment,
                    StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
                )
            }) {
                return Err(malformed());
            }
            let parameter = machine
                .structural_parameters
                .get(argument.argument_index as usize)
                .ok_or_else(malformed)?;
            if !matches!(
                parameter.access,
                StructuralAccess::Owned | terminal_psi::borrowed_access_pair_pattern!()
            ) || !matches!(
                parameter.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            ) || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || self
                    .live_claims
                    .values()
                    .any(|claim| claim.place == Some(parameter.place))
            {
                return Err(malformed());
            }
            let root = self
                .structural_values
                .get(&parameter.place)
                .ok_or_else(malformed)?;
            if !root.qualifications.is_empty() || root.structural_type != parameter.structural_type
            {
                return Err(malformed());
            }
            let array_type = array_path_type(
                &self.program.structural_types(),
                root.structural_type,
                &argument.path,
            )
            .ok_or_else(malformed)?;
            let length = byte_array_length(&self.program.structural_types(), array_type)
                .ok_or_else(malformed)?;
            if argument.bytes.len() as u128 != u128::from(length) {
                return Err(malformed());
            }
            let mut array = StructuralRuntimePlace::from(root);
            array.path.extend_from_slice(&argument.path);
            let bytes = ByteSequenceView::new(argument.bytes.clone());
            // A whole bounded input is a descriptor over the supplied storage,
            // just as an ordinary call argument is. Publish both together so
            // collection operations see the same custody at either entry.
            if argument.path.is_empty()
                && matches!(
                    self.program
                        .structural_types()
                        .get(&array_type)
                        .map(|row| &row.shape),
                    Some(StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                    ))
                )
                && matches!(
                    parameter.access,
                    terminal_psi::reading_borrow_pair_pattern!()
                )
            {
                let binding = if parameter.access == StructuralAccess::MutableBorrow {
                    ByteSequenceBinding::MutableArray {
                        array: array.clone().into(),
                        array_type,
                        offset: 0,
                        length,
                        referent: root.clone(),
                        descriptor_home: None,
                    }
                } else {
                    ByteSequenceBinding::Immutable(bytes.clone())
                };
                if self.byte_sequence_values.contains_key(&parameter.place)
                    || descriptors.insert(parameter.place, binding).is_some()
                {
                    return Err(malformed());
                }
            }
            if self.structural_byte_storage.contains_key(&array)
                || storage.insert(array, bytes).is_some()
            {
                return Err(malformed());
            }
        }
        self.structural_byte_storage.extend(storage);
        self.byte_sequence_values.extend(descriptors);
        Ok(())
    }

    /// Both array arguments and local descriptor establishments pin the same
    /// payload home. The cursor is committed only after the caller validates
    /// the complete operation; no byte payload is copied into another store.
    pub(crate) fn array_storage_root_staged(
        &self,
        place: semantic_vocabulary::PlaceId,
        identity_cursor: &mut Option<u64>,
        local_array_roots: &mut BTreeMap<semantic_vocabulary::PlaceId, TerminalStructuralValue>,
    ) -> Result<(TerminalStructuralValue, ArrayStorageHome), TerminalInterpretError> {
        let root = if let Some(array) = self.scalar_array_values.get(&place) {
            if self.structural_values.contains_key(&place) {
                return Err(malformed());
            }
            // Repeated projections in one argument list share their root
            // identity, so ordinary alias checks still see the same backing.
            let root = match local_array_roots.entry(place) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(TerminalStructuralValue {
                        opaque_identity: self
                            .local_structural_identities
                            .allocate_staged(identity_cursor)?,
                        structural_type: array.structural_type,
                        qualifications: array.qualifications.clone(),
                        projected_qualifications: Vec::new(),
                        path: Vec::new(),
                    })
                }
            };
            (
                root.clone(),
                ArrayStorageHome::Frame {
                    depth: self.call_stack.len(),
                    place,
                    structural_type: array.structural_type,
                },
            )
        } else {
            (
                self.structural_values
                    .get(&place)
                    .ok_or_else(malformed)?
                    .clone(),
                ArrayStorageHome::Runtime,
            )
        };
        Ok(root)
    }

    pub(crate) fn establish_byte_array_binding(
        &self,
        source: &StructuralArgument,
        view_type: StructuralTypeId,
        collection: &terminal_semantics::CollectionBacking,
        identity_cursor: &mut Option<u64>,
    ) -> Result<(TerminalStructuralValue, ByteSequenceBinding), TerminalInterpretError> {
        if !matches!(source.access, terminal_psi::reading_borrow_pair_pattern!()) {
            return Err(malformed());
        }
        if collection.storage == terminal_semantics::CollectionStorage::Borrowed {
            // Fixed and runtime-length descriptors retain the same original
            // backing and selected window, including a descriptor in a field.
            // The fixed form additionally owes its exact declared extent.
            let value = self.resolve_live_projection(source.place, &source.path, source.access)?;
            let binding = self.byte_descriptor_binding(source, &value)?;
            let length = self.byte_descriptor_length(&binding, &value)?;
            if !value.qualifications.is_empty()
                || match collection.extent {
                    terminal_semantics::CollectionExtent::Fixed(capacity) => length != capacity,
                    terminal_semantics::CollectionExtent::StoredLength => false,
                    _ => true,
                }
            {
                return Err(malformed());
            }
            let mut binding =
                self.reborrow_byte_sequence_binding(&binding, &value, source.access)?;
            let mut value = value;
            value.structural_type = view_type;
            match &mut binding {
                ByteSequenceBinding::MutableArray {
                    referent,
                    descriptor_home,
                    ..
                } => {
                    if descriptor_home.is_some() {
                        *descriptor_home = Some(value.clone());
                        referent.structural_type = view_type;
                    } else {
                        *referent = value.clone();
                    }
                    binding.validate_mutable_referent(self, &value)?;
                }
                ByteSequenceBinding::Immutable(bytes) => {
                    *bytes = bytes.clone().with_descriptor_home(value.clone());
                }
                ByteSequenceBinding::MutableField { .. }
                | ByteSequenceBinding::MutableOwner { .. } => {
                    return Err(malformed());
                }
            }
            return Ok((value, binding));
        }
        if source.access == StructuralAccess::SharedBorrow {
            // An owned bounded byte carrier keeps its live byte sequence in
            // its storage home; the shared snapshot binds exactly that live
            // sequence, not the declared capacity.
            if matches!(
                collection.extent,
                terminal_semantics::CollectionExtent::LiveLength { .. }
            ) {
                let (coordinate, bytes) = self.bounded_owner_snapshot(source)?;
                let value = TerminalStructuralValue {
                    opaque_identity: coordinate.opaque_identity,
                    structural_type: view_type,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    path: coordinate.path,
                };
                let bytes = bytes.with_descriptor_home(value.clone());
                return Ok((value, ByteSequenceBinding::Immutable(bytes)));
            }
            // Shared inline collections retain the existing primitive snapshot
            // semantics, including genuinely empty initialized arrays. Host
            // input policy for pointer-only byte arrays remains independent.
            let terminal_semantics::CollectionExtent::Fixed(count) = collection.extent else {
                return Err(malformed());
            };
            let byte_type = semantic_vocabulary::IntegerType::new(
                semantic_vocabulary::IntegerSign::Unsigned,
                8,
            )
            .map_err(|_| malformed())?;
            let mut path = source.path.clone();
            let mut bytes = Vec::with_capacity(usize::try_from(count).map_err(|_| malformed())?);
            for index in 0..count {
                path.push(StructuralPathSegment::FixedIndex(index));
                let value =
                    self.read_primitive_place(source.place, &path, ScalarType::Integer(byte_type))?;
                path.pop();
                let crate::values::TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Unsigned(byte),
                    ..
                } = value
                else {
                    return Err(malformed());
                };
                bytes.push(u8::try_from(byte).map_err(|_| malformed())?);
            }
            return Ok((
                TerminalStructuralValue {
                    opaque_identity: source.place.get(),
                    structural_type: view_type,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    path: Vec::new(),
                },
                ByteSequenceBinding::Immutable(ByteSequenceView::new(bytes)),
            ));
        }
        let (root, home) =
            self.array_storage_root_staged(source.place, identity_cursor, &mut BTreeMap::new())?;
        let array_type = array_path_type(
            &self.program.structural_types(),
            root.structural_type,
            &source.path,
        )
        .ok_or_else(malformed)?;
        let length = byte_array_length(&self.program.structural_types(), array_type)
            .ok_or_else(malformed)?;
        let values = BTreeMap::from([(source.place, root)]);
        let mut referent =
            resolve_single_structural_argument(&self.program.structural_types(), &values, &source)?;
        let array = ArrayStorage {
            referent: StructuralRuntimePlace::from(&referent),
            home,
        };
        if !referent.qualifications.is_empty()
            || collection.extent != terminal_semantics::CollectionExtent::Fixed(length)
            || self.stored_byte_array_length(&array)? as u128 != u128::from(length)
        {
            return Err(malformed());
        }
        referent.structural_type = view_type;
        let binding = ByteSequenceBinding::MutableArray {
            array,
            array_type,
            offset: 0,
            length,
            referent: referent.clone(),
            descriptor_home: None,
        };
        binding.validate_mutable_referent(self, &referent)?;
        Ok((referent, binding))
    }

    pub(crate) fn prepare_array_view_argument_staged(
        &self,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
        identity_cursor: &mut Option<u64>,
        local_array_roots: &mut BTreeMap<semantic_vocabulary::PlaceId, TerminalStructuralValue>,
    ) -> Result<Option<(TerminalStructuralValue, ByteSequenceBinding)>, TerminalInterpretError>
    {
        if !matches!(
            parameter.access,
            terminal_psi::reading_borrow_pair_pattern!()
        ) || !self
            .program
            .structural_types()
            .get(&parameter.structural_type)
            .is_some_and(|declaration| declaration.shape.is_borrowed_byte_carrier())
        {
            return Ok(None);
        }
        // An affine descriptor still borrows its original bytes. Share the
        // arrival contract with call and block consumers instead of treating
        // descriptor lifetime as ownership of the referent.
        if !terminal_semantics::view_descriptor_parameter(
            parameter,
            self.program.structural_types().values(),
        ) || argument.access != parameter.access
            || self
                .live_claims
                .values()
                .any(|claim| claim.place == Some(argument.place))
        {
            return Err(malformed());
        }
        if argument.path.is_empty()
            && let Some(binding @ ByteSequenceBinding::Immutable(_)) =
                self.byte_sequence_values.get(&argument.place)
        {
            let value = self
                .structural_values
                .get(&argument.place)
                .ok_or_else(malformed)?;
            if parameter.access != StructuralAccess::SharedBorrow {
                return Err(malformed());
            }
            self.check_byte_view_handback(value, argument, parameter)?;
            let binding = self.reborrow_byte_sequence_binding(binding, value, parameter.access)?;
            if value.structural_type != parameter.structural_type {
                // A shared bounded arrival retains its authenticated snapshot.
                // A dynamic formal observes that same backing under a new view
                // declaration; its extent is proved from both the declaration
                // and the actual bytes, never from the requested destination.
                let machine = self.program.verified_machine(self.current_machine)?;
                let source = machine
                    .structural_parameters
                    .iter()
                    .chain(machine.block_structural_parameters())
                    .find(|source| source.place == argument.place)
                    .ok_or_else(malformed)?;
                let types = self.program.structural_types();
                if source.structural_type != value.structural_type
                    || source.access != StructuralAccess::SharedBorrow
                    || !matches!(
                        types
                            .get(&source.structural_type)
                            .ok_or_else(malformed)?
                            .shape,
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                        )
                    )
                    || !matches!(
                        types
                            .get(&parameter.structural_type)
                            .ok_or_else(malformed)?
                            .shape,
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                        )
                    )
                {
                    return Err(malformed());
                }
                let window = terminal_semantics::fixed_byte_array_window(
                    types.values(),
                    source,
                    argument,
                    parameter,
                )
                .ok_or_else(malformed)?;
                let ByteSequenceBinding::Immutable(bytes) = binding else {
                    return Err(malformed());
                };
                if !window.backing_path.is_empty()
                    || window.offset != 0
                    || bytes.len() as u128 != u128::from(window.length)
                {
                    return Err(malformed());
                }
                let mut presented = value.clone();
                presented.structural_type = parameter.structural_type;
                presented.qualifications = parameter.qualifications.clone();
                let binding =
                    ByteSequenceBinding::Immutable(bytes.with_descriptor_home(presented.clone()));
                return Ok(Some((presented, binding)));
            }
            return Ok(Some((value.clone(), binding)));
        }
        if argument.path.is_empty()
            && let Some(binding @ ByteSequenceBinding::MutableArray { .. }) =
                self.byte_sequence_values.get(&argument.place)
        {
            let value = self
                .structural_values
                .get(&argument.place)
                .ok_or_else(malformed)?;
            self.check_byte_view_handback(value, argument, parameter)?;
            let mut borrowed =
                self.reborrow_byte_sequence_binding(binding, value, parameter.access)?;
            if value.structural_type != parameter.structural_type {
                // Mutable and shared bounded carriers have distinct identities.
                // Authenticate attenuation against the actual source declaration
                // and backing before presenting the resulting shared snapshot.
                let machine = self.program.verified_machine(self.current_machine)?;
                let source = machine
                    .structural_parameters
                    .iter()
                    .chain(machine.block_structural_parameters())
                    .find(|source| source.place == argument.place)
                    .ok_or_else(malformed)?;
                let types = self.program.structural_types();
                if source.structural_type != value.structural_type
                    || source.access != StructuralAccess::MutableBorrow
                    || parameter.access != StructuralAccess::SharedBorrow
                    || ![source.structural_type, parameter.structural_type]
                        .iter()
                        .all(|identity| {
                            types.get(identity).is_some_and(|declaration| {
                                matches!(
                                    declaration.shape,
                                    StructuralTypeShape::ByteSequence(
                                        terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                                    )
                                )
                            })
                        })
                {
                    return Err(malformed());
                }
                let window = terminal_semantics::fixed_byte_array_window(
                    types.values(),
                    source,
                    argument,
                    parameter,
                )
                .ok_or_else(malformed)?;
                let ByteSequenceBinding::Immutable(bytes) = borrowed else {
                    return Err(malformed());
                };
                if !window.backing_path.is_empty()
                    || window.offset != 0
                    || bytes.len() as u128 != u128::from(window.length)
                    || !value.projected_qualifications.is_empty()
                {
                    return Err(malformed());
                }
                let mut presented = value.clone();
                presented.structural_type = parameter.structural_type;
                presented.qualifications = parameter.qualifications.clone();
                return Ok(Some((
                    presented.clone(),
                    ByteSequenceBinding::Immutable(bytes.with_descriptor_home(presented)),
                )));
            }
            // A formal borrows the original array, not the owned descriptor.
            // Rejoin its referent identity after validating the descriptor home;
            // this keeps a simultaneous descriptor move disjoint from the loan.
            let referent = if let ByteSequenceBinding::MutableArray {
                referent,
                descriptor_home: Some(_),
                ..
            } = binding
            {
                if let ByteSequenceBinding::MutableArray {
                    descriptor_home, ..
                } = &mut borrowed
                {
                    *descriptor_home = None;
                } else if let ByteSequenceBinding::Immutable(bytes) = &mut borrowed {
                    *bytes = bytes.clone().with_descriptor_home(referent.clone());
                }
                referent.clone()
            } else {
                value.clone()
            };
            return Ok(Some((referent, borrowed)));
        }
        // A genuine array projection resolves to its own structural type. The
        // bounded byte-field presentation remains owned by its separate path.
        let (root, home) =
            self.array_storage_root_staged(argument.place, identity_cursor, local_array_roots)?;
        // A window changes the borrowed extent, never the backing's identity.
        // Keep the range out of the storage key so repeated and forwarded calls
        // update the same array and leave bytes outside the loan untouched.
        let backing_path = match argument.path.split_last() {
            Some((StructuralPathSegment::FixedByteRange { .. }, backing)) => backing,
            _ => &argument.path,
        };
        let Some(array_type) = array_path_type(
            &self.program.structural_types(),
            root.structural_type,
            backing_path,
        ) else {
            return Ok(None);
        };
        let Some(length) = byte_array_length(&self.program.structural_types(), array_type) else {
            return Ok(None);
        };
        let machine = self
            .program
            .machines()
            .get(&self.current_machine)
            .ok_or_else(malformed)?;
        let source = machine
            .structural_parameters
            .iter()
            .chain(machine.block_structural_parameters())
            .find(|source| source.place == argument.place)
            .cloned()
            .or_else(|| {
                // An initialized local owns its backing. Recover that fact from
                // its checked producer, never from the requested argument access.
                let place = machine.structural_place(argument.place)?;
                let semantic_vocabulary::StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } = place.kind
                else {
                    return None;
                };
                let result = machine.operation(producer)?.result.structural()?;
                if result.place != argument.place
                    || result.structural_type != structural_type
                    || !matches!(
                        result.multiplicity,
                        terminal_psi::nonlinear_multiplicity_pair_pattern!()
                    )
                    || !result.claims.is_empty()
                {
                    return None;
                }
                Some(StructuralParameterDeclaration {
                    place: result.place,
                    position: 0,
                    structural_type,
                    multiplicity: result.multiplicity,
                    access: StructuralAccess::Owned,
                    qualifications: result.qualifications.clone(),
                    projected_qualifications: result.projected_qualifications.clone(),
                })
            })
            .ok_or_else(malformed)?;
        if source.structural_type != root.structural_type {
            return Err(malformed());
        }
        let window = terminal_semantics::fixed_byte_array_loan(
            self.program.structural_types().values(),
            self.program.catalogs.structural_domains.iter(),
            &source,
            argument,
            parameter,
        )
        .ok_or_else(malformed)?;
        let backing_argument = StructuralArgument {
            place: argument.place,
            path: window.backing_path.to_vec(),
            access: argument.access,
        };
        let local_values;
        let values = if matches!(home, ArrayStorageHome::Frame { .. }) {
            local_values = BTreeMap::from([(argument.place, root.clone())]);
            &local_values
        } else {
            &self.structural_values
        };
        let mut referent = resolve_single_structural_argument(
            &self.program.structural_types(),
            values,
            &backing_argument,
        )?;
        let array = ArrayStorage {
            referent: StructuralRuntimePlace::from(&referent),
            home,
        };
        if self.stored_byte_array_length(&array)? as u128 != u128::from(length) {
            return Err(malformed());
        }
        referent.structural_type = parameter.structural_type;
        referent.qualifications = parameter.qualifications.clone();
        if let Some(segment @ StructuralPathSegment::FixedByteRange { .. }) = argument.path.last() {
            referent.path.push(segment.clone());
        }
        if parameter.access == StructuralAccess::SharedBorrow {
            let start = usize::try_from(window.offset).map_err(|_| malformed())?;
            let end = usize::try_from(
                window
                    .offset
                    .checked_add(window.length)
                    .ok_or_else(malformed)?,
            )
            .map_err(|_| malformed())?;
            let descriptor = self
                .stored_byte_array_snapshot(&array, start, end)?
                .with_descriptor_home(referent.clone());
            return Ok(Some((referent, ByteSequenceBinding::Immutable(descriptor))));
        }
        Ok(Some((
            referent.clone(),
            ByteSequenceBinding::MutableArray {
                array,
                array_type,
                offset: window.offset,
                length: window.length,
                referent,
                descriptor_home: None,
            },
        )))
    }
}
