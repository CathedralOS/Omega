//! Explicit initialized fixed-array inputs and their fieldless original backing.
pub(crate) mod storage;

use crate::byte_sequences::ByteSequenceView;
use crate::byte_sequences::binding::ByteSequenceBinding;
use crate::custody::resolve_structural_arguments;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::StructuralByteSequenceRuntimeField;
use crate::values::StructuralRuntimePlace;
use crate::values::TerminalStructuralValue;
use semantic_vocabulary::ScalarType;
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeMap;
use storage::{ArrayStorage, ArrayStorageHome};
use terminal_psi::StructuralAccess;
use terminal_psi::StructuralArgument;
use terminal_psi::StructuralFieldType;
use terminal_psi::StructuralMultiplicity;
use terminal_psi::StructuralParameterDeclaration;
use terminal_psi::StructuralPathSegment;
use terminal_psi::StructuralTypeDeclaration;
use terminal_psi::StructuralTypeShape;

/// Initialized contents of a true fixed u8 array reachable from an entry input.
/// The static field/index path is relative to the dense structural argument. Exactly the declared
/// positive number of bytes is required; missing contents are never initialized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalStructuralByteArrayValue {
    pub argument_index: u32,
    pub path: Vec<StructuralPathSegment>,
    pub bytes: Vec<u8>,
}

pub(crate) fn byte_array_length(
    types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    array: StructuralTypeId,
) -> Option<u64> {
    // A bounded view's capacity is the fixed byte extent; its element is the
    // implicit u8, so no element declaration is needed.
    if let StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedView {
        capacity,
        ..
    }) = types.get(&array)?.shape
    {
        return (capacity != 0).then_some(capacity);
    }
    let StructuralTypeShape::FixedArray { element, length } = types.get(&array)?.shape else {
        return None;
    };
    let StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(element)) =
        types.get(&element)?.shape
    else {
        return None;
    };
    (length != 0
        && !element.is_address()
        && element.sign() == semantic_vocabulary::IntegerSign::Unsigned
        && element.bits() == 8)
        .then_some(length)
}

pub(crate) fn array_path_type(
    types: &BTreeMap<StructuralTypeId, StructuralTypeDeclaration>,
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in path {
        structural_type = match (segment, &types.get(&structural_type)?.shape) {
            (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields }) => {
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                next
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            _ => return None,
        };
    }
    Some(structural_type)
}

impl TerminalExecution {
    pub(crate) fn bind_byte_arrays(
        &mut self,
        arguments: &[TerminalStructuralByteArrayValue],
    ) -> Result<(), TerminalInterpretError> {
        let machine = self
            .program
            .machines
            .get(&self.current_machine)
            .ok_or_else(malformed)?;
        let mut storage = BTreeMap::new();
        let mut descriptors = BTreeMap::new();
        for argument in arguments {
            let parameter = machine
                .structural_parameters
                .get(argument.argument_index as usize)
                .ok_or_else(malformed)?;
            if !matches!(
                parameter.access,
                StructuralAccess::Owned
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::SharedBorrow
            ) || !matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
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
                &self.program.structural_types,
                root.structural_type,
                &argument.path,
            )
            .ok_or_else(malformed)?;
            let length = byte_array_length(&self.program.structural_types, array_type)
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
                        .structural_types
                        .get(&array_type)
                        .map(|row| &row.shape),
                    Some(StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedView { .. }
                    ))
                )
                && matches!(
                    parameter.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
                )
            {
                let binding = if parameter.access == StructuralAccess::MutableBorrow {
                    ByteSequenceBinding::MutableArray {
                        array: array.clone().into(),
                        array_type,
                        offset: 0,
                        length,
                        referent: root.clone(),
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

    /// Observe the current contents of an explicitly initialized raw fixed array.
    /// No field identity, capacity, or mutable live-length header is involved.
    pub fn structural_byte_array(
        &self,
        opaque_identity: u64,
        path: &[StructuralPathSegment],
    ) -> Option<&[u8]> {
        self.structural_byte_storage
            .get(&StructuralRuntimePlace {
                opaque_identity,
                path: path.to_vec(),
            })
            .map(ByteSequenceView::bytes)
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
            if self.structural_values.contains_key(&place)
                || self.scalar_case_values.contains_key(&place)
            {
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
                        qualifications: Vec::new(),
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
        if !matches!(
            source.access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) {
            return Err(malformed());
        }
        if collection.storage == terminal_semantics::CollectionStorage::Borrowed {
            // A pointer-only bounded collection already carries its original
            // array home. Re-establishment changes descriptor type, never the
            // owner's activation, backing identity, or retained window.
            if !source.path.is_empty() {
                return Err(malformed());
            }
            let value = self
                .structural_values
                .get(&source.place)
                .ok_or_else(malformed)?;
            let binding = self
                .byte_sequence_values
                .get(&source.place)
                .ok_or_else(malformed)?;
            // A borrowed backing is pointer-carried, so its declared capacity
            // is the descriptor's whole extent.
            let terminal_semantics::CollectionExtent::Fixed(capacity) = collection.extent else {
                return Err(malformed());
            };
            if !value.qualifications.is_empty()
                || self.byte_sequence_length(source.place)? != capacity
            {
                return Err(malformed());
            }
            let mut binding = self.reborrow_byte_sequence_binding(binding, value, source.access)?;
            let mut value = value.clone();
            value.structural_type = view_type;
            match &mut binding {
                ByteSequenceBinding::MutableArray { referent, .. } => {
                    *referent = value.clone();
                    binding.validate_mutable_referent(&self.program.structural_types, &value)?;
                }
                ByteSequenceBinding::Immutable(bytes) => {
                    *bytes = bytes.clone().with_descriptor_home(value.clone());
                }
                ByteSequenceBinding::MutableField { .. } => {
                    return Err(malformed());
                }
            }
            return Ok((value, binding));
        }
        if source.access == StructuralAccess::SharedBorrow {
            // An owned bounded byte carrier keeps its live byte sequence in
            // the field store; the shared snapshot binds exactly that live
            // sequence, not the declared capacity.
            if matches!(
                collection.extent,
                terminal_semantics::CollectionExtent::LiveLength { .. }
            ) {
                let Some((StructuralPathSegment::Field(identity), prefix)) =
                    source.path.split_last()
                else {
                    return Err(malformed());
                };
                let parent = resolve_structural_arguments(
                    &self.program.structural_types,
                    &self.structural_values,
                    &[StructuralArgument {
                        place: source.place,
                        path: prefix.to_vec(),
                        access: source.access,
                    }],
                )?
                .pop()
                .ok_or_else(malformed)?;
                let declaration = self
                    .program
                    .structural_types
                    .get(&parent.structural_type)
                    .ok_or_else(malformed)?;
                let StructuralTypeShape::Record { fields } = &declaration.shape else {
                    return Err(malformed());
                };
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())
                    .ok_or_else(malformed)?;
                if !matches!(
                    field.field_type.byte_sequence_carrier(),
                    Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
                ) {
                    return Err(malformed());
                }
                let bytes = self
                    .structural_byte_sequence_fields
                    .get(&StructuralByteSequenceRuntimeField {
                        parent: StructuralRuntimePlace::from(&parent),
                        field: field.id,
                    })
                    .ok_or_else(malformed)?
                    .bytes()
                    .to_vec();
                return Ok((
                    TerminalStructuralValue {
                        opaque_identity: source.place.get(),
                        structural_type: view_type,
                        qualifications: Vec::new(),
                        path: Vec::new(),
                    },
                    ByteSequenceBinding::Immutable(ByteSequenceView::new(bytes)),
                ));
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
                    path: Vec::new(),
                },
                ByteSequenceBinding::Immutable(ByteSequenceView::new(bytes)),
            ));
        }
        let (root, home) =
            self.array_storage_root_staged(source.place, identity_cursor, &mut BTreeMap::new())?;
        let array_type = array_path_type(
            &self.program.structural_types,
            root.structural_type,
            &source.path,
        )
        .ok_or_else(malformed)?;
        let length =
            byte_array_length(&self.program.structural_types, array_type).ok_or_else(malformed)?;
        let values = BTreeMap::from([(source.place, root)]);
        let mut referent = resolve_structural_arguments(
            &self.program.structural_types,
            &values,
            std::slice::from_ref(source),
        )?
        .pop()
        .ok_or_else(malformed)?;
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
        };
        binding.validate_mutable_referent(&self.program.structural_types, &referent)?;
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
            StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
        ) || !self
            .program
            .structural_types
            .get(&parameter.structural_type)
            .is_some_and(|declaration| declaration.shape.is_borrowed_byte_carrier())
        {
            return Ok(None);
        }
        if parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || argument.access != parameter.access
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
            if parameter.access != StructuralAccess::SharedBorrow
                || value.structural_type != parameter.structural_type
                || !value.qualifications.is_empty()
            {
                return Err(malformed());
            }
            let binding = self.reborrow_byte_sequence_binding(binding, value, parameter.access)?;
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
            if value.structural_type != parameter.structural_type {
                return Err(malformed());
            }
            return Ok(Some((
                value.clone(),
                self.reborrow_byte_sequence_binding(binding, value, parameter.access)?,
            )));
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
            &self.program.structural_types,
            root.structural_type,
            backing_path,
        ) else {
            return Ok(None);
        };
        let Some(length) = byte_array_length(&self.program.structural_types, array_type) else {
            return Ok(None);
        };
        let machine = self
            .program
            .machines
            .get(&self.current_machine)
            .ok_or_else(malformed)?;
        let source = machine
            .structural_parameters
            .iter()
            .chain(
                machine
                    .blocks
                    .values()
                    .flat_map(|block| &block.structural_parameters),
            )
            .find(|source| source.place == argument.place)
            .cloned()
            .or_else(|| {
                // An initialized local owns its backing. Recover that fact from
                // its checked producer, never from the requested argument access.
                let place = machine
                    .structural_places
                    .iter()
                    .find(|place| place.id == argument.place)?;
                let semantic_vocabulary::StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } = place.kind
                else {
                    return None;
                };
                let result = machine
                    .blocks
                    .values()
                    .flat_map(|block| &block.operations)
                    .find(|operation| operation.id == producer)?
                    .result
                    .structural()?;
                if result.place != argument.place
                    || result.structural_type != structural_type
                    || !matches!(
                        result.multiplicity,
                        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                    )
                    || !result.claims.is_empty()
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                {
                    return None;
                }
                Some(StructuralParameterDeclaration {
                    place: result.place,
                    position: 0,
                    structural_type,
                    multiplicity: result.multiplicity,
                    access: StructuralAccess::Owned,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                })
            })
            .ok_or_else(malformed)?;
        if source.structural_type != root.structural_type
            || !source.qualifications.is_empty()
            || !source.projected_qualifications.is_empty()
        {
            return Err(malformed());
        }
        let window = terminal_semantics::fixed_byte_array_window(
            self.program.structural_types.values(),
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
        let mut referent = resolve_structural_arguments(
            &self.program.structural_types,
            values,
            std::slice::from_ref(&backing_argument),
        )?
        .pop()
        .ok_or_else(malformed)?;
        if !root.qualifications.is_empty() || !referent.qualifications.is_empty() {
            return Err(malformed());
        }
        let array = ArrayStorage {
            referent: StructuralRuntimePlace::from(&referent),
            home,
        };
        if self.stored_byte_array_length(&array)? as u128 != u128::from(length) {
            return Err(malformed());
        }
        referent.structural_type = parameter.structural_type;
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
            },
        )))
    }
}
