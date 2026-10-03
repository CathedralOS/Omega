//! Activation-local primitive roots share backing only through explicit loans.

use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, MachineId, PlaceId, ScalarType, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralMultiplicity,
    StructuralPathSegment,
};

use crate::custody::resolve_structural_path_type;
use crate::errors::TerminalInterpretError;
use crate::execution::ExecutableMachine;
use crate::execution::TerminalExecution;
use crate::scalar_operations::terminal_scalar_belongs_to_type;
use crate::values::{StructuralRuntimePlace, StructuralScalarRuntimeField};
use crate::values::{TerminalScalarValue, TerminalStructuralValue};

/// A loan pins its owner's exact activation until it returns. Stack depth is
/// stable across nested calls; the machine and type prevent namespace rebinding.
/// This is the same frame-home contract used by borrowed byte-array storage.
#[derive(Clone, Copy)]
pub(crate) struct PrimitiveStorageHome {
    depth: usize,
    machine: MachineId,
    place: PlaceId,
    structural_type: StructuralTypeId,
}

enum PrimitiveStorage {
    Scalar(StructuralRuntimePlace),
    /// One scalar leaf inside a shared referent's containing record. The key
    /// addresses `structural_scalar_fields` directly: primitive root storage
    /// only ever backs whole primitive referents and locals.
    ScalarField(StructuralScalarRuntimeField),
    ArrayElement {
        array: crate::structural_inputs::byte_arrays::storage::ArrayStorage,
        index: usize,
    },
    OwnedArrayElement {
        home: PrimitiveStorageHome,
        index: usize,
    },
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
    /// Initialized arrays carry their own payload instead of an opaque row.
    /// Every storage observer must resolve that same exclusive live root.
    pub(crate) fn live_structural_type(
        &self,
        place: PlaceId,
    ) -> Result<StructuralTypeId, TerminalInterpretError> {
        match (
            self.structural_values.get(&place),
            self.scalar_array_values.get(&place),
        ) {
            (Some(value), None) => Ok(value.structural_type),
            (None, Some(array)) => Ok(array.structural_type),
            (None, None) => Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                place,
            )),
            (Some(_), Some(_)) => Err(TerminalInterpretError::VerifiedOperationMalformed),
        }
    }

    /// `path` is static here: the execution loop already replaced each
    /// runtime-selected element with the fixed index its value names. The final
    /// flag retains shared observation authority from the current formal; only
    /// that authority permits a stored primitive to expose a different carrier.
    fn primitive_access(
        &self,
        place: PlaceId,
        writing: bool,
        path: &[StructuralPathSegment],
    ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
        self.primitive_home_access(self.capture_primitive_home(place)?, writing, path)
    }

    pub(crate) fn capture_primitive_home(
        &self,
        place: PlaceId,
    ) -> Result<PrimitiveStorageHome, TerminalInterpretError> {
        Ok(PrimitiveStorageHome {
            depth: self.call_stack.len(),
            machine: self.current_machine,
            place,
            structural_type: self.live_structural_type(place)?,
        })
    }

    fn primitive_home_access(
        &self,
        home: PrimitiveStorageHome,
        writing: bool,
        path: &[StructuralPathSegment],
    ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let place = home.place;
        let (machine_id, structural_values, scalar_array_values, byte_sequence_values) =
            if home.depth == self.call_stack.len() {
                (
                    self.current_machine,
                    &self.structural_values,
                    &self.scalar_array_values,
                    &self.byte_sequence_values,
                )
            } else {
                let frame = self.call_stack.get(home.depth).ok_or_else(invalid)?;
                (
                    frame.current_machine,
                    &frame.structural_values,
                    &frame.scalar_array_values,
                    &frame.byte_sequence_values,
                )
            };
        if machine_id != home.machine {
            return Err(invalid());
        }
        let machine = self.program.machines.get(&machine_id).ok_or_else(invalid)?;
        let (structural_type, shared_view) = local_type(machine, place)
            .map(|structural_type| (structural_type, false))
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
                    .map(|parameter| {
                        (
                            parameter.structural_type,
                            parameter.access == StructuralAccess::SharedBorrow,
                        )
                    })
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
                    .map(|result| (result.structural_type, false))
            })
            .ok_or_else(invalid)?;
        if structural_type != home.structural_type {
            return Err(invalid());
        }
        // Array loans are presented to callees as bounded byte views. Their
        // indices are relative to the loan window, not a second scalar store.
        if let Some(
            binding @ crate::byte_sequences::binding::ByteSequenceBinding::MutableArray {
                array,
                array_type,
                offset,
                length,
                ..
            },
        ) = byte_sequence_values.get(&place)
        {
            let value = structural_values.get(&place).ok_or_else(invalid)?;
            binding.validate_mutable_referent(&self.program.structural_types, value)?;
            let [StructuralPathSegment::FixedIndex(index)] = path else {
                return Err(invalid());
            };
            let scalar_type = terminal_semantics::primitive_projection_type(
                self.program.structural_types.values(),
                structural_type,
                path,
            )
            .ok_or_else(invalid)?;
            let byte_type = IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| invalid())?;
            let extent = crate::structural_inputs::byte_arrays::byte_array_length(
                &self.program.structural_types,
                *array_type,
            )
            .ok_or_else(invalid)?;
            if value.structural_type != structural_type
                || scalar_type != ScalarType::Integer(byte_type)
                || *index >= *length
                || self.stored_byte_array_length(array)? as u128 != u128::from(extent)
            {
                return Err(invalid());
            }
            let index = usize::try_from(offset.checked_add(*index).ok_or_else(invalid)?)
                .map_err(|_| invalid())?;
            return Ok((
                PrimitiveStorage::ArrayElement {
                    array: array.clone(),
                    index,
                },
                scalar_type,
                shared_view,
            ));
        }
        let path = terminal_semantics::canonical_primitive_path(
            self.program.structural_types.values(),
            structural_type,
            path,
        )
        .ok_or_else(invalid)?;
        let path = path.as_slice();
        let scalar_type = terminal_semantics::primitive_place_type(
            self.program.structural_types.values(),
            structural_type,
            path,
        )
        .ok_or_else(invalid)?;
        if let Some(array) = scalar_array_values.get(&place) {
            if array.structural_type != structural_type || structural_values.contains_key(&place) {
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
                    .program
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
                    .program
                    .structural_types
                    .get(&element)
                    .ok_or_else(invalid)?
                    .shape
                {
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(_) => 1,
                    _ => {
                        terminal_semantics::scalar_array_leaf_shape(
                            self.program.structural_types.values(),
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
                PrimitiveStorage::OwnedArrayElement { home, index },
                scalar_type,
                shared_view,
            ));
        }
        let view = structural_values.get(&place).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(place),
        )?;
        if view.structural_type != structural_type {
            return Err(invalid());
        }
        let mut projected = view.clone();
        let mut carrier = structural_type;
        for (position, segment) in path.iter().enumerate() {
            let declaration = self
                .program
                .structural_types
                .get(&carrier)
                .ok_or_else(invalid)?;
            match (segment, &declaration.shape) {
                (
                    semantic_vocabulary::CanonicalStructuralPathSegment::Field(identity),
                    terminal_psi::StructuralTypeShape::Record { fields },
                ) => {
                    let field = fields
                        .iter()
                        .find(|field| field.id == *identity && !field.relevance.is_erased())
                        .ok_or_else(invalid)?;
                    if position + 1 == path.len()
                        && matches!(
                            field.field_type,
                            terminal_psi::StructuralFieldType::Scalar(ScalarType::IeeeFloat(_))
                                | terminal_psi::StructuralFieldType::IeeeFloat(_)
                        )
                        && field.field_type.scalar_type() == Some(scalar_type)
                    {
                        return Ok((
                            PrimitiveStorage::ScalarField(StructuralScalarRuntimeField::member(
                                StructuralRuntimePlace::from(&projected),
                                field.id,
                            )),
                            scalar_type,
                            shared_view,
                        ));
                    }
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
                return Ok((
                    PrimitiveStorage::ArrayElement {
                        array: array.into(),
                        index,
                    },
                    scalar_type,
                    shared_view,
                ));
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
            let field =
                self.scalar_leaf_field(view, identity, parent_path, scalar_type, shared_view)?;
            return Ok((
                PrimitiveStorage::ScalarField(field),
                scalar_type,
                shared_view,
            ));
        }
        Ok((
            PrimitiveStorage::Scalar(StructuralRuntimePlace::from(view)),
            scalar_type,
            shared_view,
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
        shared_view: bool,
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
        let parent =
            resolve_structural_path_type(&self.program.structural_types, root, parent_path)?;
        let declaration = self
            .program
            .structural_types
            .get(&parent)
            .ok_or_else(invalid)?;
        let fields = match &declaration.shape {
            terminal_psi::StructuralTypeShape::Record { fields }
            | terminal_psi::StructuralTypeShape::Mixed { fields, .. } => fields,
            _ => return Err(invalid()),
        };
        let field = fields
            .iter()
            .find(|field| field.identity == *identity && !field.relevance.is_erased())
            .ok_or_else(invalid)?;
        let Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(stored_type)) =
            field.field_type.canonical_leaf_shape()
        else {
            return Err(invalid());
        };
        if stored_type != scalar_type
            && !(shared_view
                && terminal_semantics::scalar_reference_view_is_total(stored_type, scalar_type))
        {
            return Err(invalid());
        }
        Ok(StructuralScalarRuntimeField::member(
            StructuralRuntimePlace {
                opaque_identity: view.opaque_identity,
                path: parent_path.to_vec(),
            },
            field.id,
        ))
    }

    pub(crate) fn execute_primitive_establishment(
        &mut self,
        operation: &Operation,
        value: ValueId,
    ) -> Result<(), TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let result = operation.result.structural().ok_or_else(invalid)?;
        let machine = self
            .program
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
            .program
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

    /// A read that ends at a scalar field of a record element it selected at
    /// run time (`cells[i].hp`). The loop has already replaced each selector
    /// with the element its value names, so `path` is static here and the
    /// read is the same observation `IntegerStructuralField` or
    /// `BooleanStructuralField` makes of that element's field: the field's
    /// value lives in `structural_scalar_fields` under its record's runtime
    /// place. `None` when the path ends at a primitive leaf instead.
    fn element_field_read(
        &self,
        source: PlaceId,
        path: &[StructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<Option<TerminalScalarValue>, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::VerifiedOperationMalformed;
        let Some((StructuralPathSegment::Field(identity), carrier)) = path.split_last() else {
            return Ok(None);
        };
        let root = self.observation_root(source)?;
        let Some((carrier, record)) = terminal_semantics::canonical_static_projection(
            self.program.structural_types.values(),
            root.structural_type,
            carrier,
        ) else {
            return Ok(None);
        };
        let terminal_psi::StructuralTypeShape::Record { fields } = &record.shape else {
            return Ok(None);
        };
        let mut matching = fields.iter().filter(|field| field.identity == *identity);
        let (Some(field), None) = (matching.next(), matching.next()) else {
            return Err(invalid());
        };
        // A field holding a structural child is an ordinary primitive leaf
        // projection; only a scalar field ends this route.
        let terminal_psi::StructuralFieldType::Scalar(declared) = field.field_type else {
            return Ok(None);
        };
        if field.relevance.is_erased() || declared != scalar_type {
            return Err(invalid());
        }
        let cell = self.observed_field_cell(source, &carrier, field.id)?;
        let value = self.structural_scalar_fields.get(&cell).copied().ok_or(
            TerminalInterpretError::StructuralScalarFieldMissing {
                source,
                field: field.id,
            },
        )?;
        if value.scalar_type() != scalar_type {
            return Err(invalid());
        }
        Ok(Some(value))
    }

    pub(crate) fn execute_primitive_read(
        &mut self,
        operation: &Operation,
        source: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<(), TerminalInterpretError> {
        let result = operation
            .result
            .scalar()
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let scalar = self.read_primitive_place(source, path, result.scalar_type)?;
        self.values.insert(result.id, scalar);
        Ok(())
    }

    /// Ordinary reads and collection descriptors observe the same resolved
    /// primitive storage; neither may manufacture an alternate backing.
    pub(crate) fn read_primitive_place(
        &self,
        source: PlaceId,
        path: &[StructuralPathSegment],
        expected: ScalarType,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        if let Some(value) = self.element_field_read(source, path, expected)? {
            if !terminal_scalar_belongs_to_type(value) {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            return Ok(value);
        }
        let (storage, scalar_type, shared_view) = self.primitive_access(source, false, path)?;
        self.read_primitive_storage(source, storage, scalar_type, expected, shared_view)
    }

    pub(crate) fn read_primitive_home(
        &self,
        home: PrimitiveStorageHome,
        path: &[StructuralPathSegment],
        expected: ScalarType,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        let (storage, scalar_type, shared_view) = self.primitive_home_access(home, false, path)?;
        self.read_primitive_storage(home.place, storage, scalar_type, expected, shared_view)
    }

    fn read_primitive_storage(
        &self,
        source: PlaceId,
        storage: PrimitiveStorage,
        scalar_type: ScalarType,
        expected: ScalarType,
        shared_view: bool,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        let scalar = match storage {
            PrimitiveStorage::OwnedArrayElement { home, index } => {
                let arrays = if home.depth == self.call_stack.len() {
                    &self.scalar_array_values
                } else {
                    &self
                        .call_stack
                        .get(home.depth)
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?
                        .scalar_array_values
                };
                arrays
                    .get(&home.place)
                    .filter(|array| array.structural_type == home.structural_type)
                    .and_then(|array| array.elements.get(index))
                    .copied()
                    .ok_or(TerminalInterpretError::StructuralPrimitiveStorageMissing(
                        source,
                    ))
            }
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
                self.stored_byte_array_read(&array, index).map(|byte| {
                    TerminalScalarValue::Integer {
                        scalar_type: integer,
                        value: IntegerValue::Unsigned(u128::from(byte)),
                    }
                })
            }
        }?;
        if expected != scalar_type || !terminal_scalar_belongs_to_type(scalar) {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        if scalar.scalar_type() == scalar_type {
            return Ok(scalar);
        }
        if !shared_view {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        // Observation re-views the original cell; the stored scalar is never
        // converted or replaced, so subsequent observations share its lifetime.
        scalar_reference_observation(scalar, scalar_type)
    }

    pub(crate) fn execute_primitive_store(
        &mut self,
        operation: &Operation,
        destination: PlaceId,
        value: ValueId,
        path: &[StructuralPathSegment],
    ) -> Result<(), TerminalInterpretError> {
        if operation.result != OperationResult::Unit {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let scalar = self
            .values
            .get(&value)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(value))?;
        self.store_primitive_place(destination, path, scalar)
    }

    pub(crate) fn store_primitive_place(
        &mut self,
        destination: PlaceId,
        path: &[StructuralPathSegment],
        scalar: TerminalScalarValue,
    ) -> Result<(), TerminalInterpretError> {
        let (storage, scalar_type, _) = self.primitive_access(destination, true, path)?;
        self.store_primitive_storage(destination, storage, scalar_type, scalar)
    }

    pub(crate) fn store_primitive_home(
        &mut self,
        home: PrimitiveStorageHome,
        path: &[StructuralPathSegment],
        scalar: TerminalScalarValue,
    ) -> Result<(), TerminalInterpretError> {
        let (storage, scalar_type, _) = self.primitive_home_access(home, true, path)?;
        self.store_primitive_storage(home.place, storage, scalar_type, scalar)
    }

    fn store_primitive_storage(
        &mut self,
        destination: PlaceId,
        storage: PrimitiveStorage,
        scalar_type: ScalarType,
        scalar: TerminalScalarValue,
    ) -> Result<(), TerminalInterpretError> {
        if scalar.scalar_type() != scalar_type || !terminal_scalar_belongs_to_type(scalar) {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        match storage {
            PrimitiveStorage::OwnedArrayElement { home, index } => {
                let arrays = if home.depth == self.call_stack.len() {
                    &mut self.scalar_array_values
                } else {
                    &mut self
                        .call_stack
                        .get_mut(home.depth)
                        .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?
                        .scalar_array_values
                };
                let stored = arrays
                    .get_mut(&home.place)
                    .filter(|array| array.structural_type == home.structural_type)
                    .and_then(|array| array.elements.get_mut(index))
                    .ok_or(TerminalInterpretError::StructuralPrimitiveStorageMissing(
                        destination,
                    ))?;
                if stored.scalar_type() != scalar_type {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                }
                *stored = scalar;
            }
            PrimitiveStorage::Scalar(storage) => {
                let stored = self.structural_primitive_storage.get_mut(&storage).ok_or(
                    TerminalInterpretError::StructuralPrimitiveStorageMissing(destination),
                )?;
                if stored.scalar_type() != scalar_type {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                }
                *stored = scalar;
            }
            PrimitiveStorage::ScalarField(field) => {
                let stored = self.structural_scalar_fields.get_mut(&field).ok_or(
                    TerminalInterpretError::StructuralScalarFieldMissing {
                        source: destination,
                        field: field.field,
                    },
                )?;
                if stored.scalar_type() != scalar_type {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                }
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
                self.stored_byte_array_write(&array, index, byte)?;
            }
        }
        Ok(())
    }

    pub(crate) fn primitive_local_count(&self) -> usize {
        self.program
            .machines
            .get(&self.current_machine)
            .map_or(0, |machine| {
                self.structural_values
                    .keys()
                    .filter(|place| local_type(machine, **place).is_some())
                    .count()
            })
    }

    pub(crate) fn retire_plain_locals(&mut self) {
        self.retire_unrestricted_views();
        let Some(machine) = self.program.machines.get(&self.current_machine) else {
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
        self.retire_unrestricted_aggregates();
    }
}

fn scalar_reference_observation(
    stored: TerminalScalarValue,
    target: ScalarType,
) -> Result<TerminalScalarValue, TerminalInterpretError> {
    use terminal_semantics::ScalarViewValue;
    let input = match stored {
        TerminalScalarValue::Boolean(value) => ScalarViewValue::Boolean(value),
        TerminalScalarValue::Integer { value, .. } => ScalarViewValue::Integer(value),
        TerminalScalarValue::IeeeFloat(value) => ScalarViewValue::IeeeFloat(value),
    };
    let observed =
        terminal_semantics::scalar_reference_view_value(stored.scalar_type(), target, input)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
    match (observed, target) {
        (ScalarViewValue::Boolean(value), ScalarType::Boolean) => {
            Ok(TerminalScalarValue::Boolean(value))
        }
        (ScalarViewValue::Integer(value), ScalarType::Integer(scalar_type)) => {
            Ok(TerminalScalarValue::Integer { scalar_type, value })
        }
        (ScalarViewValue::IeeeFloat(value), ScalarType::IeeeFloat(_)) => {
            Ok(TerminalScalarValue::IeeeFloat(value))
        }
        _ => Err(TerminalInterpretError::VerifiedOperationMalformed),
    }
}
