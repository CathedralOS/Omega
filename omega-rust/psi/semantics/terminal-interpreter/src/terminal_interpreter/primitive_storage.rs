//! Activation-local primitive roots share backing only through explicit loans.

use std::collections::BTreeSet;

use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, PlaceId, ScalarType, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralMultiplicity,
    StructuralPathSegment, TerminalModule,
};

use super::{
    TerminalExecution, TerminalInterpretError, TerminalScalarValue, TerminalStructuralValue,
};
use crate::terminal_interpreter::custody::resolve_structural_path_type;
use crate::terminal_interpreter::execution::ExecutableMachine;
use crate::terminal_interpreter::scalar_operations::terminal_scalar_belongs_to_type;
use crate::terminal_interpreter::values::{StructuralRuntimePlace, StructuralScalarRuntimeField};

enum PrimitiveStorage {
    Scalar(StructuralRuntimePlace),
    /// One scalar leaf inside a shared referent's containing record. The key
    /// addresses `structural_scalar_fields` directly: primitive root storage
    /// only ever backs whole primitive referents and locals.
    ScalarField(StructuralScalarRuntimeField),
    ArrayElement {
        array: StructuralRuntimePlace,
        index: usize,
    },
    OwnedArrayElement {
        place: PlaceId,
        index: usize,
    },
}

pub(crate) struct LocalStructuralIdentities {
    reserved: BTreeSet<u64>,
    next: Option<u64>,
}

impl LocalStructuralIdentities {
    #[cfg(test)]
    pub(crate) fn with_reserved_identities(reserved: impl IntoIterator<Item = u64>) -> Self {
        Self {
            reserved: reserved.into_iter().collect(),
            next: Some(0),
        }
    }

    pub(super) fn new(module: &TerminalModule, arguments: &[TerminalStructuralValue]) -> Self {
        Self {
            reserved: module
                .machines
                .iter()
                .flat_map(|machine| &machine.structural_places)
                .map(|place| place.id.get())
                .chain(arguments.iter().map(|value| value.opaque_identity))
                .collect(),
            next: Some(0),
        }
    }

    pub(super) fn allocate(&mut self) -> Result<u64, TerminalInterpretError> {
        let mut cursor = self.next;
        let result = self.allocate_staged(&mut cursor);
        self.next = cursor;
        result
    }

    pub(super) fn cursor(&self) -> Option<u64> {
        self.next
    }

    /// Reserve identities in a transaction-local cursor without changing the
    /// live allocator or cloning its immutable host reservation set.
    pub(super) fn allocate_staged(
        &self,
        cursor: &mut Option<u64>,
    ) -> Result<u64, TerminalInterpretError> {
        loop {
            let identity = cursor.ok_or(TerminalInterpretError::StructuralIdentityExhausted)?;
            *cursor = identity.checked_add(1);
            if !self.reserved.contains(&identity) {
                return Ok(identity);
            }
        }
    }

    pub(super) fn commit_cursor(&mut self, cursor: Option<u64>) {
        self.next = cursor;
    }

    pub(super) fn reserve_host(
        &mut self,
        value: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        let identity = value.opaque_identity;
        // A host result cannot claim an identity allocated to an interpreter local.
        if self.next.is_none_or(|next| identity < next) && !self.reserved.contains(&identity) {
            return Err(TerminalInterpretError::StructuralArgumentAliasing(identity));
        }
        self.reserved.insert(identity);
        Ok(())
    }
}

fn local_type(machine: &ExecutableMachine, place: PlaceId) -> Option<StructuralTypeId> {
    let declaration = machine
        .structural_places
        .iter()
        .find(|entry| entry.id == place)?;
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return None;
    };
    let operation = machine
        .blocks
        .values()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == producer)?;
    let result = operation.result.structural()?;
    (matches!(
        operation.kind,
        OperationKind::EstablishPrimitiveLocal { .. }
    ) && result.place == place
        && result.structural_type == structural_type
        && result.multiplicity == StructuralMultiplicity::Unrestricted
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty())
    .then_some(structural_type)
}

impl TerminalExecution {
    fn primitive_access(
        &self,
        place: PlaceId,
        writing: bool,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    ) -> Result<(PrimitiveStorage, ScalarType), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let machine = self
            .machines
            .get(&self.current_machine)
            .ok_or_else(invalid)?;
        let structural_type = local_type(machine, place)
            .or_else(|| {
                machine
                    .structural_parameters
                    .iter()
                    .chain(
                        machine
                            .blocks
                            .values()
                            .flat_map(|block| &block.structural_parameters),
                    )
                    .find(|parameter| parameter.place == place)
                    .filter(|parameter| {
                        ((!path.is_empty() && parameter.access == StructuralAccess::Owned)
                            || if writing {
                                matches!(
                                    parameter.access,
                                    StructuralAccess::MutableBorrow
                                        | StructuralAccess::WriteOnlyBorrow
                                )
                            } else {
                                matches!(
                                    parameter.access,
                                    StructuralAccess::SharedBorrow
                                        | StructuralAccess::MutableBorrow
                                )
                            })
                            && (parameter.multiplicity == StructuralMultiplicity::Unrestricted
                                || (!path.is_empty()
                                    && parameter.multiplicity == StructuralMultiplicity::Affine))
                            && parameter.qualifications.is_empty()
                            && parameter.projected_qualifications.is_empty()
                    })
                    .map(|parameter| parameter.structural_type)
            })
            .or_else(|| {
                if path.is_empty() {
                    return None;
                }
                machine
                    .blocks
                    .values()
                    .flat_map(|block| &block.operations)
                    .filter_map(|operation| operation.result.structural())
                    .find(|result| {
                        result.place == place
                            && result.claims.is_empty()
                            && result.qualifications.is_empty()
                            && result.projected_qualifications.is_empty()
                    })
                    .map(|result| result.structural_type)
            })
            .ok_or_else(invalid)?;
        let scalar_type = terminal_semantics::primitive_place_type(
            self.structural_types.values(),
            structural_type,
            path,
        )
        .ok_or_else(invalid)?;
        if let Some(array) = self.scalar_array_values.get(&place) {
            if array.structural_type != structural_type
                || self.structural_values.contains_key(&place)
            {
                return Err(invalid());
            }
            let mut carrier = structural_type;
            let mut leaf_offset = 0_u64;
            for segment in path {
                let semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(index) =
                    segment
                else {
                    return Err(invalid());
                };
                let terminal_psi::StructuralTypeShape::FixedArray { element, length } = self
                    .structural_types
                    .get(&carrier)
                    .ok_or_else(invalid)?
                    .shape
                else {
                    return Err(invalid());
                };
                if *index >= length {
                    return Err(invalid());
                }
                let leaf_count = match self
                    .structural_types
                    .get(&element)
                    .ok_or_else(invalid)?
                    .shape
                {
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(_) => 1,
                    _ => {
                        terminal_semantics::scalar_array_leaf_shape(
                            self.structural_types.values(),
                            element,
                        )
                        .ok_or_else(invalid)?
                        .1
                    }
                };
                leaf_offset = leaf_offset
                    .checked_add(index.checked_mul(leaf_count).ok_or_else(invalid)?)
                    .ok_or_else(invalid)?;
                carrier = element;
            }
            let index = usize::try_from(leaf_offset).map_err(|_| invalid())?;
            if array
                .elements
                .get(index)
                .is_none_or(|value| value.scalar_type() != scalar_type)
            {
                return Err(invalid());
            }
            return Ok((
                PrimitiveStorage::OwnedArrayElement { place, index },
                scalar_type,
            ));
        }
        let view = self.structural_values.get(&place).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(place),
        )?;
        if view.structural_type != structural_type {
            return Err(invalid());
        }
        let mut projected = view.clone();
        let mut carrier = structural_type;
        for segment in path {
            let declaration = self.structural_types.get(&carrier).ok_or_else(invalid)?;
            match (segment, &declaration.shape) {
                (
                    semantic_vocabulary::CanonicalStructuralPathSegment::Field(identity),
                    terminal_psi::StructuralTypeShape::Record { fields },
                ) => {
                    let field = fields
                        .iter()
                        .find(|field| field.id == *identity && !field.relevance.is_erased())
                        .ok_or_else(invalid)?;
                    let terminal_psi::StructuralFieldType::Structural(child) = field.field_type
                    else {
                        return Err(invalid());
                    };
                    projected
                        .path
                        .push(StructuralPathSegment::Field(field.identity.clone()));
                    carrier = child;
                }
                (
                    semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(index),
                    terminal_psi::StructuralTypeShape::FixedArray { element, length },
                ) if index < length => {
                    projected
                        .path
                        .push(StructuralPathSegment::FixedIndex(*index));
                    carrier = *element;
                }
                _ => return Err(invalid()),
            }
        }
        projected.structural_type = carrier;
        let view = &projected;
        // A callee's whole primitive parameter can be the caller's projected
        // array element. Resolve that original backing, shared with byte-view
        // loans, instead of installing a second scalar copy. The call's typed
        // path was independently checked before it became this runtime view.
        if let Some((StructuralPathSegment::FixedIndex(index), parent_path)) =
            view.path.split_last()
        {
            let array = StructuralRuntimePlace {
                opaque_identity: view.opaque_identity,
                path: parent_path.to_vec(),
            };
            if let Some(bytes) = self.structural_byte_arrays.get(&array) {
                let byte_type =
                    IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| invalid())?;
                let index = usize::try_from(*index).map_err(|_| invalid())?;
                if scalar_type != ScalarType::Integer(byte_type)
                    || bytes.get(index).is_none()
                    || self
                        .structural_primitive_storage
                        .contains_key(&StructuralRuntimePlace::from(view))
                {
                    return Err(invalid());
                }
                return Ok((PrimitiveStorage::ArrayElement { array, index }, scalar_type));
            }
        }
        // Other overlaps cannot silently fall back to independent storage.
        if self.structural_byte_arrays.keys().any(|array| {
            array.opaque_identity == view.opaque_identity && view.path.starts_with(&array.path)
        }) {
            return Err(invalid());
        }
        // A shared `&T` parameter can bind one scalar leaf inside the pinned
        // referent's containing record. That leaf's content lives in
        // `structural_scalar_fields` under the parent runtime place and the
        // field's declared identity — not in primitive root storage.
        if let Some((StructuralPathSegment::Field(identity), parent_path)) = view.path.split_last()
        {
            let field = self.scalar_leaf_field(view, identity, parent_path, scalar_type)?;
            return Ok((PrimitiveStorage::ScalarField(field), scalar_type));
        }
        Ok((
            PrimitiveStorage::Scalar(StructuralRuntimePlace::from(view)),
            scalar_type,
        ))
    }

    /// Map a bound scalar-leaf view's spelled field tip back to its declared
    /// `structural_scalar_fields` key. The view's runtime path names the leaf;
    /// the containing record's declared type is recovered from the referent
    /// root, which the pinned owner keeps bound whole in the current frame or
    /// a suspended caller while any view of it is live.
    fn scalar_leaf_field(
        &self,
        view: &TerminalStructuralValue,
        identity: &str,
        parent_path: &[StructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<StructuralScalarRuntimeField, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let root = self
            .structural_values
            .values()
            .chain(
                self.call_stack
                    .iter()
                    .flat_map(|frame| frame.structural_values.values()),
            )
            .filter(|value| value.path.is_empty())
            .find_map(|value| {
                (value.opaque_identity == view.opaque_identity).then_some(value.structural_type)
            })
            .ok_or_else(invalid)?;
        let parent = resolve_structural_path_type(&self.structural_types, root, parent_path)?;
        let declaration = self.structural_types.get(&parent).ok_or_else(invalid)?;
        let fields = match &declaration.shape {
            terminal_psi::StructuralTypeShape::Record { fields }
            | terminal_psi::StructuralTypeShape::Mixed { fields, .. } => fields,
            _ => return Err(invalid()),
        };
        let field = fields
            .iter()
            .find(|field| field.identity == *identity && !field.relevance.is_erased())
            .ok_or_else(invalid)?;
        if field.field_type.canonical_leaf_shape()
            != Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(
                scalar_type,
            ))
        {
            return Err(invalid());
        }
        Ok(StructuralScalarRuntimeField {
            parent: StructuralRuntimePlace {
                opaque_identity: view.opaque_identity,
                path: parent_path.to_vec(),
            },
            field: field.id,
        })
    }

    pub(super) fn execute_primitive_establishment(
        &mut self,
        operation: &Operation,
        value: ValueId,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let result = operation.result.structural().ok_or_else(invalid)?;
        let machine = self
            .machines
            .get(&self.current_machine)
            .ok_or_else(invalid)?;
        if local_type(machine, result.place) != Some(result.structural_type)
            || !machine.structural_places.iter().any(|place| {
                place.id == result.place && matches!(place.kind,
                    StructuralPlaceKind::OperationResult { producer, .. } if producer == operation.id)
            })
        {
            return Err(invalid());
        }
        let Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(expected)) = self
            .structural_types
            .get(&result.structural_type)
            .map(|entry| &entry.shape)
        else {
            return Err(invalid());
        };
        let scalar = self
            .values
            .get(&value)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(value))?;
        if scalar.scalar_type() != *expected || !terminal_scalar_belongs_to_type(scalar) {
            return Err(invalid());
        }
        let identity = self.local_structural_identities.allocate()?;
        let view = TerminalStructuralValue {
            opaque_identity: identity,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        // A reentered definition replaces this activation's old local only.
        // Local results cannot escape through owned arguments or structural returns.
        if let Some(previous) = self.structural_values.insert(result.place, view.clone()) {
            self.structural_primitive_storage
                .remove(&StructuralRuntimePlace::from(&previous));
        }
        self.structural_primitive_storage
            .insert(StructuralRuntimePlace::from(&view), scalar);
        Ok(())
    }

    pub(super) fn execute_primitive_read(
        &mut self,
        operation: &Operation,
        source: PlaceId,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    ) -> Result<(), TerminalInterpretError> {
        let result = operation
            .result
            .scalar()
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let (storage, scalar_type) = self.primitive_access(source, false, path)?;
        let scalar = match storage {
            PrimitiveStorage::OwnedArrayElement { place, index } => self
                .scalar_array_values
                .get(&place)
                .and_then(|array| array.elements.get(index))
                .copied()
                .ok_or(TerminalInterpretError::StructuralPrimitiveStorageMissing(
                    source,
                )),
            PrimitiveStorage::Scalar(storage) => self
                .structural_primitive_storage
                .get(&storage)
                .copied()
                .ok_or(TerminalInterpretError::StructuralPrimitiveStorageMissing(
                    source,
                )),
            PrimitiveStorage::ScalarField(field) => {
                self.structural_scalar_fields.get(&field).copied().ok_or(
                    TerminalInterpretError::StructuralScalarFieldMissing {
                        source,
                        field: field.field,
                    },
                )
            }
            PrimitiveStorage::ArrayElement { array, index } => {
                let ScalarType::Integer(integer) = scalar_type else {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                };
                self.structural_byte_arrays
                    .get(&array)
                    .and_then(|bytes| bytes.get(index))
                    .map(|byte| TerminalScalarValue::Integer {
                        scalar_type: integer,
                        value: IntegerValue::Unsigned(u128::from(*byte)),
                    })
                    .ok_or(TerminalInterpretError::StructuralPrimitiveStorageMissing(
                        source,
                    ))
            }
        }?;
        if result.scalar_type != scalar_type
            || scalar.scalar_type() != scalar_type
            || !terminal_scalar_belongs_to_type(scalar)
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        self.values.insert(result.id, scalar);
        Ok(())
    }

    pub(super) fn execute_primitive_store(
        &mut self,
        operation: &Operation,
        destination: PlaceId,
        value: ValueId,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    ) -> Result<(), TerminalInterpretError> {
        if operation.result != OperationResult::Unit {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let (storage, scalar_type) = self.primitive_access(destination, true, path)?;
        let scalar = self
            .values
            .get(&value)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(value))?;
        if scalar.scalar_type() != scalar_type || !terminal_scalar_belongs_to_type(scalar) {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        match storage {
            PrimitiveStorage::OwnedArrayElement { place, index } => {
                let stored = self
                    .scalar_array_values
                    .get_mut(&place)
                    .and_then(|array| array.elements.get_mut(index))
                    .ok_or(TerminalInterpretError::StructuralPrimitiveStorageMissing(
                        destination,
                    ))?;
                *stored = scalar;
            }
            PrimitiveStorage::Scalar(storage) => {
                let stored = self.structural_primitive_storage.get_mut(&storage).ok_or(
                    TerminalInterpretError::StructuralPrimitiveStorageMissing(destination),
                )?;
                *stored = scalar;
            }
            PrimitiveStorage::ScalarField(field) => {
                let stored = self.structural_scalar_fields.get_mut(&field).ok_or(
                    TerminalInterpretError::StructuralScalarFieldMissing {
                        source: destination,
                        field: field.field,
                    },
                )?;
                *stored = scalar;
            }
            PrimitiveStorage::ArrayElement { array, index } => {
                let TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(value),
                    ..
                } = scalar
                else {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                };
                let byte = u8::try_from(value)
                    .map_err(|_| TerminalInterpretError::VerifiedOperationMalformed)?;
                self.structural_byte_arrays
                    .get_mut(&array)
                    .and_then(|bytes| bytes.replace_byte(index, byte))
                    .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
            }
        }
        Ok(())
    }

    /// The runtime index is a `u64` operand, not a path segment: once its
    /// exact value is resolved the store proceeds through the same verified
    /// access walk as a literal-indexed store, which re-checks the element
    /// against the declared extent segment by segment.
    pub(super) fn execute_indexed_primitive_store(
        &mut self,
        operation: &Operation,
        destination: PlaceId,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        index: ValueId,
        value: ValueId,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let TerminalScalarValue::Integer {
            scalar_type,
            value: IntegerValue::Unsigned(raw),
        } = self
            .values
            .get(&index)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(index))?
        else {
            return Err(invalid());
        };
        if scalar_type != IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())? {
            return Err(invalid());
        }
        let index = u64::try_from(raw).map_err(|_| invalid())?;
        let mut projected = path.to_vec();
        projected.push(semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(index));
        self.execute_primitive_store(operation, destination, value, &projected)
    }

    pub(super) fn primitive_local_count(&self) -> usize {
        self.machines
            .get(&self.current_machine)
            .map_or(0, |machine| {
                self.structural_values
                    .keys()
                    .filter(|place| local_type(machine, **place).is_some())
                    .count()
            })
    }

    pub(super) fn retire_plain_locals(&mut self) {
        let Some(machine) = self.machines.get(&self.current_machine) else {
            return;
        };
        for place in &machine.structural_places {
            if local_type(machine, place.id).is_some()
                && let Some(view) = self.structural_values.remove(&place.id)
            {
                self.structural_primitive_storage
                    .remove(&StructuralRuntimePlace::from(&view));
            }
        }
        self.retire_unrestricted_records();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BTreeSet, LocalStructuralIdentities, StructuralTypeId, TerminalInterpretError,
        TerminalStructuralValue,
    };

    #[test]
    fn fresh_identities_skip_reserved_values_and_do_not_wrap() {
        let mut identities = LocalStructuralIdentities {
            reserved: BTreeSet::from([0, 2]),
            next: Some(0),
        };
        assert_eq!(identities.allocate(), Ok(1));
        assert_eq!(identities.allocate(), Ok(3));
        identities.next = Some(u64::MAX);
        assert_eq!(identities.allocate(), Ok(u64::MAX));
        assert_eq!(
            identities.allocate(),
            Err(TerminalInterpretError::StructuralIdentityExhausted)
        );
    }

    #[test]
    fn host_results_cannot_reintroduce_allocated_local_identity() {
        let mut identities = LocalStructuralIdentities {
            reserved: BTreeSet::from([0]),
            next: Some(0),
        };
        let view = |opaque_identity| TerminalStructuralValue {
            opaque_identity,
            structural_type: StructuralTypeId::new(1).unwrap(),
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        identities.reserve_host(&view(2)).unwrap();
        assert_eq!(identities.allocate(), Ok(1));
        assert_eq!(
            identities.reserve_host(&view(1)),
            Err(TerminalInterpretError::StructuralArgumentAliasing(1))
        );
        identities.reserve_host(&view(0)).unwrap();
        assert_eq!(identities.allocate(), Ok(3));
        assert_eq!(
            identities.reserved,
            BTreeSet::from([0, 2]),
            "allocation does not retain one tombstone per dead local"
        );
    }
}
