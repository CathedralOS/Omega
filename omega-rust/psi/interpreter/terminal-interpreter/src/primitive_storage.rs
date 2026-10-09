//! Activation-local primitive roots share backing only through explicit loans.
//!
//! A primitive descriptor alone cannot address an owned array's payload: that
//! payload remains in its owner's activation when an ordinary helper runs.
//! `capture_primitive_loan` therefore binds the visible qualified referent to
//! its exact storage endpoint; calls, successors and stored reference carriers
//! retain that association instead of allocating a shadow scalar. Accessors
//! below validate the owner frame and use that endpoint for every load/store.
//! Shared representation observations stay ordinary language semantics; the
//! stricter host replacement protocol lives in `effects::scalar_loans`.

use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, MachineId, PlaceId, ScalarType, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    live_field_by_id,
};

use crate::custody::resolve_structural_path_type;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::SuspendedCall;
use crate::execution::TerminalExecution;
use crate::scalar_operations::terminal_scalar_belongs_to_type;
use crate::values::{StructuralRuntimePlace, StructuralScalarRuntimeField};
use crate::values::{TerminalScalarValue, TerminalStructuralValue};
use terminal_psi::TerminalMachine;

pub(crate) mod encoding;

pub(crate) use encoding::{scalar_fits_storage, shifted_byte_endpoint};

/// Reference formation and a host snapshot have different atomic permissions.
/// A snapshot under an already-bound exclusive aggregate loan may inspect its
/// atomic resident; no operation here can create a plain reference to that cell.
#[derive(Clone, Copy)]
enum ScalarStorageObservation {
    OrdinaryReference { shared_view: bool },
    ExclusiveSnapshot,
}

/// A loan pins its owner's exact activation until it returns. Stack depth is
/// stable across nested calls; the machine and type prevent namespace rebinding.
/// This is the same frame-home contract used by borrowed byte-array storage.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct PrimitiveStorageHome {
    depth: usize,
    machine: MachineId,
    place: PlaceId,
    structural_type: StructuralTypeId,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum PrimitiveStorageEndpoint {
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
    /// One byte of an immutable shared byte view. A shared loan cannot change
    /// the bytes it observes, so the view's own backing is the endpoint; no
    /// write reaches it.
    ViewByte {
        bytes: crate::byte_sequences::ByteSequenceView,
        index: usize,
    },
}

/// Encoding is retained with the endpoint, not reconstructed from a later
/// formal's semantic scalar type. Canonical storage has no explicit codec.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PrimitiveStorage {
    pub(crate) endpoint: PrimitiveStorageEndpoint,
    pub(crate) stored_scalar: Option<ScalarType>,
}

impl PrimitiveStorage {
    fn canonical(endpoint: PrimitiveStorageEndpoint) -> Self {
        Self {
            endpoint,
            stored_scalar: None,
        }
    }
}

/// One ordinary primitive loan retains a storage endpoint, not copied data.
/// Frame-local place bindings may change; the pinned owner activation does not.
#[derive(Clone)]
pub(crate) struct PrimitiveLoan {
    pub(crate) storage: PrimitiveStorage,
    pub(crate) scalar_type: ScalarType,
    pub(crate) shared_view: bool,
    pub(crate) access: StructuralAccess,
    pub(crate) referent: TerminalStructuralValue,
}

fn local_type(machine: &TerminalMachine, place: PlaceId) -> Option<StructuralTypeId> {
    let declaration = machine.structural_place(place)?;
    let (producer, structural_type) = declaration.kind.as_operation_result()?;
    let operation = machine.operation(producer)?;
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

impl TerminalExecution<'_> {
    /// Prepare the same original endpoint for ordinary calls and host staging.
    /// In particular, a local array's scalar payload stays in its owner's
    /// activation; its observation descriptor never creates another scalar row.
    pub(crate) fn capture_primitive_loan(
        &self,
        parameter: &StructuralParameterDeclaration,
        argument: &StructuralArgument,
        identity_cursor: &mut Option<u64>,
    ) -> Result<PrimitiveLoan, TerminalInterpretError> {
        let terminal_psi::StructuralTypeShape::PrimitiveScalar(expected) = self
            .program
            .structural_types()
            .get(&parameter.structural_type)
            .ok_or_else(malformed)?
            .shape
        else {
            return Err(malformed());
        };
        if parameter.access != argument.access
            || !matches!(
                parameter.access,
                StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            )
            || parameter.multiplicity != StructuralMultiplicity::Unrestricted
        {
            return Err(malformed());
        }
        if argument.path.is_empty()
            && let Some(loan) = self.primitive_loan_values.get(&argument.place)
        {
            let value = self
                .structural_values
                .get(&argument.place)
                .ok_or_else(malformed)?;
            if *value != loan.referent
                || value.structural_type != parameter.structural_type
                || (parameter.access != StructuralAccess::SharedBorrow
                    && loan.access == StructuralAccess::SharedBorrow)
                || (parameter.access == StructuralAccess::SharedBorrow
                    && loan.access == StructuralAccess::WriteOnlyBorrow)
                || (parameter.access == StructuralAccess::MutableBorrow
                    && loan.access == StructuralAccess::WriteOnlyBorrow)
            {
                return Err(malformed());
            }
            let mut rebound = loan.clone();
            rebound.access = parameter.access;
            return Ok(rebound);
        }
        let retained_reference = if let Some(crossing) = argument
            .path
            .iter()
            .rposition(|segment| *segment == StructuralPathSegment::Referent)
            .filter(|crossing| *crossing + 1 == argument.path.len())
        {
            let carrier = self.resolve_live_projection(
                argument.place,
                &argument.path[..crossing],
                argument.access,
            )?;
            self.reference_referents
                .get(&StructuralRuntimePlace::from(&carrier))
                .and_then(|referent| referent.primitive_loan.as_ref())
        } else {
            None
        };
        let (referent, storage, scalar_type, shared_view) = if let Some(loan) = retained_reference {
            // Reference-path resolution independently checks every crossing's
            // declared access. Its view retains actual qualifications.
            let referent =
                self.resolve_live_projection(argument.place, &argument.path, argument.access)?;
            if referent.structural_type != parameter.structural_type {
                return Err(malformed());
            }
            (
                referent,
                loan.storage.clone(),
                expected,
                parameter.access == StructuralAccess::SharedBorrow,
            )
        } else if self.scalar_array_values.contains_key(&argument.place)
            || matches!(
                self.byte_sequence_values.get(&argument.place),
                Some(crate::byte_sequences::binding::ByteSequenceBinding::MutableArray { .. })
            )
            || (!argument.path.is_empty()
                && matches!(
                    self.byte_sequence_values.get(&argument.place),
                    Some(crate::byte_sequences::binding::ByteSequenceBinding::Immutable(_))
                ))
        {
            let (storage, scalar_type, shared_view) = self.primitive_access(
                argument.place,
                parameter.access != StructuralAccess::SharedBorrow,
                &argument.path,
            )?;
            let (opaque_identity, path) = match &storage.endpoint {
                PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                    let mut path = array.referent.path.clone();
                    path.push(StructuralPathSegment::FixedIndex(
                        u64::try_from(*index).map_err(|_| malformed())?,
                    ));
                    (array.referent.opaque_identity, path)
                }
                PrimitiveStorageEndpoint::OwnedArrayElement { .. } => (
                    self.local_structural_identities
                        .allocate_staged(identity_cursor)?,
                    argument.path.clone(),
                ),
                // The view descriptor's own identity names its observed byte.
                PrimitiveStorageEndpoint::ViewByte { index, .. } => {
                    let view = self
                        .structural_values
                        .get(&argument.place)
                        .ok_or_else(malformed)?;
                    let mut path = view.path.clone();
                    path.push(StructuralPathSegment::FixedIndex(
                        u64::try_from(*index).map_err(|_| malformed())?,
                    ));
                    (view.opaque_identity, path)
                }
                _ => return Err(malformed()),
            };
            (
                TerminalStructuralValue {
                    opaque_identity,
                    structural_type: parameter.structural_type,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    path,
                },
                storage,
                scalar_type,
                shared_view,
            )
        } else {
            let value =
                self.resolve_live_projection(argument.place, &argument.path, argument.access)?;
            if value.structural_type != parameter.structural_type {
                return Err(malformed());
            }
            let (storage, scalar_type, shared_view) = self.primitive_view_access(
                &value,
                &[],
                expected,
                parameter.access == StructuralAccess::SharedBorrow,
            )?;
            (value, storage, scalar_type, shared_view)
        };
        if scalar_type != expected {
            return Err(malformed());
        }
        Ok(PrimitiveLoan {
            storage,
            scalar_type,
            shared_view,
            access: parameter.access,
            referent,
        })
    }

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
            (Some(_), Some(_)) => Err(malformed()),
        }
    }

    /// `path` is static here: the execution loop already replaced each
    /// runtime-selected element with the fixed index its value names. The final
    /// flag retains shared observation authority from the current formal; only
    /// that authority permits a stored primitive to expose a different carrier.
    pub(crate) fn primitive_access(
        &self,
        place: PlaceId,
        writing: bool,
        path: &[StructuralPathSegment],
    ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
        if let Some(crossing) = path
            .iter()
            .rposition(|segment| *segment == StructuralPathSegment::Referent)
        {
            let access = if writing {
                StructuralAccess::MutableBorrow
            } else {
                StructuralAccess::SharedBorrow
            };
            // Resolve the captured pointer before applying the primitive
            // suffix. Its descriptor home is never the referent's storage.
            let (referent, view) =
                self.resolve_live_view_projection(place, &path[..=crossing], access)?;
            if let Some(view) = view {
                return self.aggregate_view_primitive_access(
                    &referent,
                    view,
                    &path[crossing + 1..],
                );
            }
            let suffix = &path[crossing + 1..];
            // A retained byte region selects its scalar interpretation from
            // the authored catalog, not canonical primitive-place grammar.
            // Inline integer fields otherwise never reach their stored codec.
            let mut projected = referent.clone();
            projected.path.extend_from_slice(suffix);
            if let Some((storage, scalar_type)) =
                self.reference_byte_region_access(&referent, &projected)?
            {
                return Ok((storage, scalar_type, !writing));
            }
            let projection_type = if writing {
                terminal_semantics::primitive_projection_type
            } else {
                terminal_semantics::primitive_read_projection_type
            };
            let scalar_type = projection_type(
                self.program.structural_types().values(),
                referent.structural_type,
                suffix,
            )
            .ok_or_else(malformed)?;
            if suffix.is_empty() {
                let carrier = self.resolve_live_projection(place, &path[..crossing], access)?;
                if let Some(captured) = self
                    .reference_referents
                    .get(&StructuralRuntimePlace::from(&carrier))
                    && let Some(loan) = &captured.primitive_loan
                {
                    if captured.view != loan.referent
                        || (writing && loan.access != StructuralAccess::MutableBorrow)
                    {
                        return Err(malformed());
                    }
                    return Ok((loan.storage.clone(), scalar_type, !writing));
                }
            }
            let canonical_path = if writing {
                terminal_semantics::canonical_primitive_path
            } else {
                terminal_semantics::canonical_primitive_read_path
            };
            let canonical = canonical_path(
                self.program.structural_types().values(),
                referent.structural_type,
                suffix,
            )
            .ok_or_else(malformed)?;
            return self.primitive_view_access(&referent, canonical.as_slice(), scalar_type, false);
        }
        self.primitive_home_access(self.capture_primitive_home(place)?, writing, path)
    }

    /// A selected element of a byte carrier is a `u8`. A borrowed view's
    /// extent is runtime state, so the static judgment proves no element; the
    /// caller checks the selected index against the binding's live extent.
    fn byte_element_scalar(
        &self,
        structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
    ) -> Result<ScalarType, TerminalInterpretError> {
        if matches!(
            self.program
                .structural_types()
                .get(&structural_type)
                .ok_or_else(malformed)?
                .shape,
            terminal_psi::StructuralTypeShape::ByteSequence(_)
        ) {
            return Ok(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?,
            ));
        }
        terminal_semantics::primitive_projection_type(
            self.program.structural_types().values(),
            structural_type,
            path,
        )
        .ok_or_else(malformed)
    }

    /// A scalar leaf observed through an aggregate re-view lives in the
    /// backing cell at the same position. When the two declarations carry
    /// it differently (`i16` seen as `u16`), the view's carrier becomes the
    /// stored interpretation, so reads take the modular image and writes
    /// store the image of the view's value through the existing codec.
    fn aggregate_view_primitive_access(
        &self,
        referent: &TerminalStructuralValue,
        view: crate::reference::AggregateViewTip,
        suffix: &[StructuralPathSegment],
    ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
        let types = self.program.structural_types();
        let crate::reference::AggregateViewTip::Aggregate { view, backing } = view else {
            return Err(malformed());
        };
        if backing != referent.structural_type {
            return Err(malformed());
        }
        let (suffix, tip) =
            crate::reference::translate_aggregate_view_path(&types, view, backing, suffix)
                .ok_or_else(malformed)?;
        let primitive = |structural_type: StructuralTypeId| match types
            .get(&structural_type)
            .map(|declaration| &declaration.shape)
        {
            Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(scalar)) => Some(*scalar),
            _ => None,
        };
        let (observed, stored) = match tip {
            crate::reference::AggregateViewTip::Scalar { view, backing } => (view, backing),
            crate::reference::AggregateViewTip::Aggregate { view, backing } => (
                primitive(view).ok_or_else(malformed)?,
                primitive(backing).ok_or_else(malformed)?,
            ),
        };
        let canonical =
            terminal_semantics::canonical_primitive_path(types.values(), backing, &suffix)
                .ok_or_else(malformed)?;
        let (mut storage, _, shared_view) =
            self.primitive_view_access(referent, canonical.as_slice(), stored, false)?;
        if observed != stored {
            if storage.stored_scalar.is_some()
                || !terminal_semantics::scalar_reference_view_is_total(stored, observed)
            {
                return Err(malformed());
            }
            storage.stored_scalar = Some(observed);
        }
        Ok((storage, observed, shared_view))
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

    pub(crate) fn primitive_home_access(
        &self,
        home: PrimitiveStorageHome,
        writing: bool,
        path: &[StructuralPathSegment],
    ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
        let place = home.place;
        let (
            machine_id,
            structural_values,
            scalar_array_values,
            byte_sequence_values,
            primitive_loan_values,
            element_view_values,
        ) = if home.depth == self.call_stack.len() {
            (
                self.current_machine,
                &self.structural_values,
                &self.scalar_array_values,
                &self.byte_sequence_values,
                &self.primitive_loan_values,
                &self.element_view_values,
            )
        } else {
            let frame = self.call_stack.get(home.depth).ok_or_else(malformed)?;
            (
                frame.current_machine,
                &frame.structural_values,
                &frame.scalar_array_values,
                &frame.byte_sequence_values,
                &frame.primitive_loan_values,
                &frame.element_view_values,
            )
        };
        if machine_id != home.machine {
            return Err(malformed());
        }
        if let Some(loan) = element_view_values.get(&place)
            && matches!(
                self.program
                    .structural_types()
                    .get(&home.structural_type)
                    .ok_or_else(malformed)?
                    .shape,
                terminal_psi::StructuralTypeShape::FixedArray { .. }
            )
        {
            let path = terminal_semantics::canonical_primitive_path(
                self.program.structural_types().values(),
                home.structural_type,
                path,
            )
            .ok_or_else(malformed)?;
            return loan.scalar_storage(self, &path, writing);
        }
        let machine = self
            .program
            .machines()
            .get(&machine_id)
            .ok_or_else(malformed)?;
        if let Some(loan) = primitive_loan_values.get(&place) {
            let parameter = machine.structural_parameter(place).ok_or_else(malformed)?;
            if !path.is_empty()
                || structural_values.get(&place) != Some(&loan.referent)
                || loan.referent.structural_type != home.structural_type
                || parameter.structural_type != home.structural_type
                || parameter.access != loan.access
                || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                || if writing {
                    !matches!(
                        loan.access,
                        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                    )
                } else {
                    !matches!(
                        loan.access,
                        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                    )
                }
            {
                return Err(malformed());
            }
            // Ordinary ingress already binds the exact qualified view. This
            // endpoint association does not erase its qualifiers or convert
            // stored payload; host replacement admission remains separate.
            return Ok((loan.storage.clone(), loan.scalar_type, loan.shared_view));
        }
        let (structural_type, shared_view) = local_type(machine, place)
            .map(|structural_type| (structural_type, false))
            .or_else(|| {
                machine
                    .structural_parameter(place)
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
                            && parameter.admits_scalar_read_at(path.is_empty())
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
                    .operations()
                    .filter_map(|operation| operation.result.structural())
                    .find(|result| {
                        result.place == place
                            && result.claims.is_empty()
                            && result.qualifications.is_empty()
                            && result.projected_qualifications.is_empty()
                    })
                    .map(|result| (result.structural_type, false))
            })
            .ok_or_else(malformed)?;
        if structural_type != home.structural_type {
            return Err(malformed());
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
            let value = structural_values.get(&place).ok_or_else(malformed)?;
            binding.validate_mutable_referent(&self.program.structural_types(), value)?;
            let [StructuralPathSegment::FixedIndex(index)] = path else {
                return Err(malformed());
            };
            let scalar_type = self.byte_element_scalar(structural_type, path)?;
            let byte_type = IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?;
            let extent = crate::structural_inputs::byte_arrays::byte_array_length(
                &self.program.structural_types(),
                *array_type,
            )
            .ok_or_else(malformed)?;
            if value.structural_type != structural_type
                || scalar_type != ScalarType::Integer(byte_type)
                || *index >= *length
                || self.stored_byte_array_length(array)? as u128 != u128::from(extent)
            {
                return Err(malformed());
            }
            let index = usize::try_from(offset.checked_add(*index).ok_or_else(malformed)?)
                .map_err(|_| malformed())?;
            return Ok((
                PrimitiveStorage::canonical(PrimitiveStorageEndpoint::ArrayElement {
                    array: array.clone(),
                    index,
                }),
                scalar_type,
                shared_view,
            ));
        }
        // A shared byte view's element is a byte of the immutable backing the
        // view observes; that backing cannot change while the loan lives.
        if let Some(crate::byte_sequences::binding::ByteSequenceBinding::Immutable(bytes)) =
            byte_sequence_values.get(&place)
        {
            let value = structural_values.get(&place).ok_or_else(malformed)?;
            let [StructuralPathSegment::FixedIndex(index)] = path else {
                return Err(malformed());
            };
            let scalar_type = self.byte_element_scalar(structural_type, path)?;
            let byte_type = IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?;
            let index = usize::try_from(*index).map_err(|_| malformed())?;
            if writing
                || value.structural_type != structural_type
                || !bytes.matches_descriptor_home(value)
                || scalar_type != ScalarType::Integer(byte_type)
                || index >= bytes.len()
            {
                return Err(malformed());
            }
            return Ok((
                PrimitiveStorage::canonical(PrimitiveStorageEndpoint::ViewByte {
                    bytes: bytes.clone(),
                    index,
                }),
                scalar_type,
                shared_view,
            ));
        }
        // Read geometry can retain a selected payload without copying its
        // owner. Writes keep the narrower storage-formation contract. The
        // execution loop checks every case discriminator before this lookup.
        let projection_type = if writing {
            terminal_semantics::primitive_projection_type
        } else {
            terminal_semantics::primitive_read_projection_type
        };
        let scalar_type = projection_type(
            self.program.structural_types().values(),
            structural_type,
            path,
        )
        .ok_or_else(malformed)?;
        let canonical_path = if writing {
            terminal_semantics::canonical_primitive_path
        } else {
            terminal_semantics::canonical_primitive_read_path
        };
        let path = canonical_path(
            self.program.structural_types().values(),
            structural_type,
            path,
        )
        .ok_or_else(malformed)?;
        let path = path.as_slice();
        if let Some(array) = scalar_array_values.get(&place) {
            if array.structural_type != structural_type || structural_values.contains_key(&place) {
                return Err(malformed());
            }
            let mut carrier = structural_type;
            let mut leaf_offset = 0_u64;
            for segment in path {
                let semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(index) =
                    segment
                else {
                    return Err(malformed());
                };
                let terminal_psi::StructuralTypeShape::FixedArray { element, length } = self
                    .program
                    .structural_types()
                    .get(&carrier)
                    .ok_or_else(malformed)?
                    .shape
                else {
                    return Err(malformed());
                };
                if *index >= length {
                    return Err(malformed());
                }
                let leaf_count = match self
                    .program
                    .structural_types()
                    .get(&element)
                    .ok_or_else(malformed)?
                    .shape
                {
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(_) => 1,
                    _ => {
                        terminal_semantics::scalar_array_leaf_shape(
                            self.program.structural_types().values(),
                            element,
                        )
                        .ok_or_else(malformed)?
                        .1
                    }
                };
                leaf_offset = leaf_offset
                    .checked_add(index.checked_mul(leaf_count).ok_or_else(malformed)?)
                    .ok_or_else(malformed)?;
                carrier = element;
            }
            let index = usize::try_from(leaf_offset).map_err(|_| malformed())?;
            if array
                .elements
                .get(index)
                .is_none_or(|value| value.scalar_type() != scalar_type)
            {
                return Err(malformed());
            }
            return Ok((
                PrimitiveStorage::canonical(PrimitiveStorageEndpoint::OwnedArrayElement {
                    home,
                    index,
                }),
                scalar_type,
                shared_view,
            ));
        }
        let view = crate::custody::verified_structural_place(structural_values, place)?;
        if view.structural_type != structural_type {
            return Err(malformed());
        }
        self.primitive_view_access(view, path, scalar_type, shared_view)
    }

    /// Resolve a typed primitive suffix against its original runtime backing.
    /// Borrowed formals and stored references share this storage selection;
    /// neither creates an independent copy of the observed or written leaf.
    pub(crate) fn primitive_view_access(
        &self,
        view: &TerminalStructuralValue,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
        shared_view: bool,
    ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
        self.resolve_scalar_view_storage(
            view,
            path,
            scalar_type,
            ScalarStorageObservation::OrdinaryReference { shared_view },
        )
    }

    /// Called only after argument binding authenticates the aggregate loan.
    /// Shared snapshots retain ordinary observation rules; exclusive snapshots
    /// retain original atomic backing without forming a scalar reference.
    pub(crate) fn boundary_scalar_storage(
        &self,
        view: &TerminalStructuralValue,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
        access: StructuralAccess,
    ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
        let observation = match access {
            StructuralAccess::MutableBorrow => ScalarStorageObservation::ExclusiveSnapshot,
            StructuralAccess::SharedBorrow => {
                ScalarStorageObservation::OrdinaryReference { shared_view: true }
            }
            _ => return Err(malformed()),
        };
        self.resolve_scalar_view_storage(view, path, scalar_type, observation)
    }

    fn resolve_scalar_view_storage(
        &self,
        view: &TerminalStructuralValue,
        path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
        scalar_type: ScalarType,
        observation: ScalarStorageObservation,
    ) -> Result<(PrimitiveStorage, ScalarType, bool), TerminalInterpretError> {
        let shared_view = matches!(
            observation,
            ScalarStorageObservation::OrdinaryReference { shared_view: true }
        );
        let backing_view = view;
        let mut projected = view.clone();
        let mut carrier = view.structural_type;
        let mut selected_case = None;
        for (position, segment) in path.iter().enumerate() {
            let declaration = self
                .program
                .structural_types()
                .get(&carrier)
                .ok_or_else(malformed)?;
            match (segment, &declaration.shape) {
                (
                    semantic_vocabulary::CanonicalStructuralPathSegment::Case(case),
                    terminal_psi::StructuralTypeShape::Sum { cases }
                    | terminal_psi::StructuralTypeShape::Mixed { cases, .. },
                ) => {
                    if selected_case.is_some() {
                        return Err(malformed());
                    }
                    let actual = self
                        .structural_cases
                        .get(&StructuralRuntimePlace::from(&projected))
                        .ok_or_else(malformed)?;
                    if actual.structural_type != carrier || actual.case != *case {
                        return Err(malformed());
                    }
                    selected_case = Some(
                        cases
                            .iter()
                            .find(|row| row.id == *case)
                            .ok_or_else(malformed)?,
                    );
                }
                (semantic_vocabulary::CanonicalStructuralPathSegment::Field(identity), _) => {
                    let payload = selected_case.is_some();
                    let fields = if let Some(case) = selected_case.take() {
                        case.fields.as_slice()
                    } else {
                        // A field without a Case step names a record member or
                        // a mixed shape's common member, never payload storage.
                        declaration.shape.direct_fields().ok_or_else(malformed)?
                    };
                    let field = live_field_by_id(fields, *identity).ok_or_else(malformed)?;
                    if position + 1 == path.len()
                        && (!matches!(
                            field.field_type,
                            terminal_psi::StructuralFieldType::Atomic(_)
                        ) || matches!(observation, ScalarStorageObservation::ExclusiveSnapshot))
                        && field.field_type.scalar_type() == Some(scalar_type)
                    {
                        let mut leaf = projected.clone();
                        leaf.path
                            .push(StructuralPathSegment::Field(field.identity.clone()));
                        if let Some((storage, logical_scalar)) =
                            self.reference_byte_region_access(backing_view, &leaf)?
                        {
                            // A representation view proves a byte codec, not
                            // dedicated atomic-cell identity. Exclusive capture
                            // still requires original atomic resident backing.
                            if logical_scalar != scalar_type
                                || matches!(
                                    field.field_type,
                                    terminal_psi::StructuralFieldType::Atomic(_)
                                )
                            {
                                return Err(malformed());
                            }
                            return Ok((storage, scalar_type, shared_view));
                        }
                        return Ok((
                            PrimitiveStorage::canonical(PrimitiveStorageEndpoint::ScalarField(
                                if payload {
                                    StructuralScalarRuntimeField::case_payload(
                                        StructuralRuntimePlace::from(&projected),
                                        field.id,
                                    )
                                } else {
                                    StructuralScalarRuntimeField::member(
                                        StructuralRuntimePlace::from(&projected),
                                        field.id,
                                    )
                                },
                            )),
                            scalar_type,
                            shared_view,
                        ));
                    }
                    let Some(child) = field.field_type.as_structural() else {
                        return Err(malformed());
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
                _ => return Err(malformed()),
            }
        }
        projected.structural_type = carrier;
        let view = &projected;
        if let Some((storage, logical_scalar)) =
            self.reference_byte_region_access(backing_view, view)?
        {
            if logical_scalar != scalar_type {
                return Err(malformed());
            }
            return Ok((storage, scalar_type, shared_view));
        }
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
            if let Some(bytes) = self.structural_byte_storage.get(&array) {
                let byte_type =
                    IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?;
                let index = usize::try_from(*index).map_err(|_| malformed())?;
                if scalar_type != ScalarType::Integer(byte_type)
                    || bytes.get(index).is_none()
                    || self
                        .structural_primitive_storage
                        .contains_key(&StructuralRuntimePlace::from(view))
                {
                    return Err(malformed());
                }
                return Ok((
                    PrimitiveStorage::canonical(PrimitiveStorageEndpoint::ArrayElement {
                        array: array.into(),
                        index,
                    }),
                    scalar_type,
                    shared_view,
                ));
            }
        }
        // Other overlaps cannot silently fall back to independent storage.
        if self.structural_byte_storage.keys().any(|array| {
            array.opaque_identity == view.opaque_identity && view.path.starts_with(&array.path)
        }) {
            return Err(malformed());
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
                PrimitiveStorage::canonical(PrimitiveStorageEndpoint::ScalarField(field)),
                scalar_type,
                shared_view,
            ));
        }
        Ok((
            PrimitiveStorage::canonical(PrimitiveStorageEndpoint::Scalar(
                StructuralRuntimePlace::from(view),
            )),
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
            .ok_or_else(malformed)?;
        let parent =
            resolve_structural_path_type(&self.program.structural_types(), root, parent_path)?;
        let declaration = self
            .program
            .structural_types()
            .get(&parent)
            .ok_or_else(malformed)?;
        let fields = match &declaration.shape {
            terminal_psi::StructuralTypeShape::Record { fields, .. }
            | terminal_psi::StructuralTypeShape::Mixed { fields, .. } => fields,
            _ => return Err(malformed()),
        };
        let field = terminal_psi::live_field_by_identity(fields, identity).ok_or_else(malformed)?;
        let shape = if shared_view {
            field.field_type.shared_observation_shape()
        } else {
            field.field_type.canonical_leaf_shape()
        };
        let Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(stored_type)) = shape else {
            return Err(malformed());
        };
        if stored_type != scalar_type
            && !(shared_view
                && terminal_semantics::scalar_reference_view_is_total(stored_type, scalar_type))
        {
            return Err(malformed());
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
        let result = operation.result.structural().ok_or_else(malformed)?;
        let machine = self
            .program
            .machines()
            .get(&self.current_machine)
            .ok_or_else(malformed)?;
        // Block entry retired any earlier visit's local, so a value still
        // here is a second live definition.
        if local_type(machine, result.place) != Some(result.structural_type)
            || !machine.structural_places.iter().any(|place| {
                place.id == result.place && matches!(place.kind,
                    StructuralPlaceKind::OperationResult { producer, .. } if producer == operation.id)
            })
            || self.structural_values.contains_key(&result.place)
        {
            return Err(malformed());
        }
        let Some(terminal_psi::StructuralTypeShape::PrimitiveScalar(expected)) = self
            .program
            .structural_types()
            .get(&result.structural_type)
            .map(|entry| &entry.shape)
        else {
            return Err(malformed());
        };
        let scalar = self.scalar_value(value)?;
        if scalar.scalar_type() != *expected || !terminal_scalar_belongs_to_type(scalar) {
            return Err(malformed());
        }
        let identity = self.local_structural_identities.allocate()?;
        let view = TerminalStructuralValue {
            opaque_identity: identity,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            path: Vec::new(),
        };
        self.structural_values.insert(result.place, view.clone());
        self.structural_primitive_storage
            .insert(StructuralRuntimePlace::from(&view), scalar);
        Ok(())
    }

    /// Observe the final scalar field under its original runtime record.
    /// Admission checked the complete primitive-read path before the loop
    /// replaced runtime selectors with fixed indices. Resolve every stored
    /// reference in that selected path before locating the field; rechecking
    /// read eligibility on a suffix would lose the preceding selectors.
    /// Restricted fields retain their declared scalar carrier and stored value,
    /// without manufacturing primitive homes or granting store authority.
    fn scalar_field_read(
        &self,
        source: PlaceId,
        path: &[StructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<Option<TerminalScalarValue>, TerminalInterpretError> {
        let Some((StructuralPathSegment::Field(identity), carrier)) = path.split_last() else {
            return Ok(None);
        };
        let (record, view) =
            self.resolve_live_view_projection(source, carrier, StructuralAccess::SharedBorrow)?;
        // Through an aggregate re-view the field is the view's; its backing
        // field at the same position holds the cell under its own carrier.
        let translated;
        let (identity, stored_type) = match view {
            None => (identity, scalar_type),
            Some(crate::reference::AggregateViewTip::Aggregate { view, backing })
                if backing == record.structural_type =>
            {
                let (path, tip) = crate::reference::translate_aggregate_view_path(
                    &self.program.structural_types(),
                    view,
                    backing,
                    &path[path.len() - 1..],
                )
                .ok_or_else(malformed)?;
                let (
                    [StructuralPathSegment::Field(backing_identity)],
                    crate::reference::AggregateViewTip::Scalar {
                        view: observed,
                        backing: stored,
                    },
                ) = (path.as_slice(), tip)
                else {
                    return Ok(None);
                };
                if observed != scalar_type {
                    return Err(malformed());
                }
                translated = backing_identity.clone();
                (&translated, stored)
            }
            Some(_) => return Err(malformed()),
        };
        let Some(fields) = self
            .program
            .structural_types()
            .record_fields(&record.structural_type)
        else {
            return Ok(None);
        };
        let mut matching = fields.iter().filter(|field| field.identity == *identity);
        let (Some(field), None) = (matching.next(), matching.next()) else {
            return Err(malformed());
        };
        // A field holding a structural child is an ordinary primitive leaf
        // projection; only a scalar field ends this route.
        if matches!(
            field.field_type,
            terminal_psi::StructuralFieldType::Atomic(_)
        ) {
            return Err(malformed());
        }
        let Some(declared) = field.field_type.scalar_type() else {
            return Ok(None);
        };
        if field.relevance.is_erased() || declared != stored_type {
            return Err(malformed());
        }
        let mut leaf = record.clone();
        leaf.path
            .push(StructuralPathSegment::Field(identity.clone()));
        if let Some((storage, logical_scalar)) =
            self.reference_byte_region_access(&record, &leaf)?
        {
            return self
                .read_primitive_storage(source, storage, logical_scalar, scalar_type, false)
                .map(Some);
        }
        let cell =
            StructuralScalarRuntimeField::member(StructuralRuntimePlace::from(&record), field.id);
        let mut value = self.structural_scalar_fields.get(&cell).copied().ok_or(
            TerminalInterpretError::StructuralScalarFieldMissing {
                source,
                field: field.id,
            },
        )?;
        if value.scalar_type() != stored_type {
            return Err(malformed());
        }
        if stored_type != scalar_type {
            value = scalar_reference_observation(value, scalar_type)?;
        }
        Ok(Some(value))
    }

    pub(crate) fn execute_primitive_read(
        &mut self,
        operation: &Operation,
        source: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<(), TerminalInterpretError> {
        let result = operation.result.scalar().ok_or(malformed())?;
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
        if let Some(value) = self.scalar_field_read(source, path, expected)? {
            if !terminal_scalar_belongs_to_type(value) {
                return Err(malformed());
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

    pub(crate) fn read_primitive_storage(
        &self,
        source: PlaceId,
        storage: PrimitiveStorage,
        scalar_type: ScalarType,
        expected: ScalarType,
        shared_view: bool,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        if expected != scalar_type {
            return Err(malformed());
        }
        if let Some(stored_scalar) = storage.stored_scalar {
            return self.read_encoded_primitive(source, &storage.endpoint, stored_scalar, expected);
        }
        let scalar = match storage.endpoint {
            PrimitiveStorageEndpoint::OwnedArrayElement { home, index } => {
                self.owned_array_home(&home)?;
                let arrays = if home.depth == self.call_stack.len() {
                    &self.scalar_array_values
                } else {
                    &self.frame_at_depth(home.depth)?.scalar_array_values
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
            PrimitiveStorageEndpoint::Scalar(storage) => self
                .structural_primitive_storage
                .get(&storage)
                .copied()
                .ok_or(TerminalInterpretError::StructuralPrimitiveStorageMissing(
                    source,
                )),
            PrimitiveStorageEndpoint::ScalarField(field) => {
                self.structural_scalar_fields.get(&field).copied().ok_or(
                    TerminalInterpretError::StructuralScalarFieldMissing {
                        source,
                        field: field.field,
                    },
                )
            }
            PrimitiveStorageEndpoint::ViewByte { bytes, index } => {
                let Some(integer) = scalar_type.as_integer() else {
                    return Err(malformed());
                };
                bytes
                    .get(index)
                    .map(|byte| TerminalScalarValue::Integer {
                        scalar_type: integer,
                        value: IntegerValue::Unsigned(u128::from(*byte)),
                    })
                    .ok_or_else(malformed)
            }
            PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                let Some(integer) = scalar_type.as_integer() else {
                    return Err(malformed());
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
            return Err(malformed());
        }
        if scalar.scalar_type() == scalar_type {
            return Ok(scalar);
        }
        if !shared_view {
            return Err(malformed());
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
            return Err(malformed());
        }
        let scalar = self.scalar_value(value)?;
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

    pub(crate) fn store_primitive_storage(
        &mut self,
        destination: PlaceId,
        storage: PrimitiveStorage,
        scalar_type: ScalarType,
        scalar: TerminalScalarValue,
    ) -> Result<(), TerminalInterpretError> {
        if scalar.scalar_type() != scalar_type || !terminal_scalar_belongs_to_type(scalar) {
            return Err(malformed());
        }
        if let Some(stored_scalar) = storage.stored_scalar {
            return self.store_encoded_primitive(
                destination,
                &storage.endpoint,
                stored_scalar,
                scalar,
            );
        }
        match storage.endpoint {
            PrimitiveStorageEndpoint::OwnedArrayElement { home, index } => {
                self.owned_array_home(&home)?;
                let arrays = if home.depth == self.call_stack.len() {
                    &mut self.scalar_array_values
                } else {
                    &mut self
                        .call_stack
                        .get_mut(home.depth)
                        .ok_or(malformed())?
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
                    return Err(malformed());
                }
                *stored = scalar;
            }
            PrimitiveStorageEndpoint::Scalar(storage) => {
                let stored = self.structural_primitive_storage.get_mut(&storage).ok_or(
                    TerminalInterpretError::StructuralPrimitiveStorageMissing(destination),
                )?;
                if stored.scalar_type() != scalar_type {
                    return Err(malformed());
                }
                *stored = scalar;
            }
            PrimitiveStorageEndpoint::ScalarField(field) => {
                let stored = self.structural_scalar_fields.get_mut(&field).ok_or(
                    TerminalInterpretError::StructuralScalarFieldMissing {
                        source: destination,
                        field: field.field,
                    },
                )?;
                if stored.scalar_type() != scalar_type {
                    return Err(malformed());
                }
                *stored = scalar;
            }
            PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                let TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(value),
                    ..
                } = scalar
                else {
                    return Err(malformed());
                };
                let byte = u8::try_from(value).map_err(|_| malformed())?;
                self.stored_byte_array_write(&array, index, byte)?;
            }
            PrimitiveStorageEndpoint::ViewByte { .. } => return Err(malformed()),
        }
        Ok(())
    }

    pub(crate) fn retire_plain_locals(&mut self) {
        self.retire_unrestricted_views();
        let Some(machine) = self.program.machines().get(&self.current_machine) else {
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
    }

    /// Entering a block starts a fresh execution of its operations, so every
    /// result place they produce is dead at entry: each producer dominates
    /// its uses, and a value an earlier visit left there authorizes nothing.
    /// That earlier value owes no cleanup when its custody already ended, and
    /// its activation-local backing is reclaimed here, just as a return
    /// reclaims plain locals. A result still in the affine frontier or named
    /// by a live claim would leak its obligation, so that arrival fails
    /// closed rather than overwriting it.
    pub(crate) fn retire_reentered_results(
        &mut self,
        target: semantic_vocabulary::BlockId,
    ) -> Result<(), TerminalInterpretError> {
        let machine = self.program.verified_machine(self.current_machine)?;
        let block = machine
            .block(&target)
            .ok_or(TerminalInterpretError::VerifiedBlockMissing)?;
        for place in block
            .operations
            .iter()
            .filter_map(|operation| operation.result.structural())
            .map(|result| result.place)
        {
            if self.live_affine_discard_at(place).is_some()
                || self
                    .live_claims
                    .values()
                    .any(|claim| claim.place == Some(place))
            {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            if let Some(previous) = self.structural_values.remove(&place)
                && local_type(machine, place).is_some()
            {
                self.structural_primitive_storage
                    .remove(&StructuralRuntimePlace::from(&previous));
            }
            self.scalar_array_values.remove(&place);
            self.byte_sequence_values.remove(&place);
            self.primitive_loan_values.remove(&place);
            self.element_view_values.remove(&place);
        }
        Ok(())
    }

    /// Resolve the machine owning a suspended home frame, rejecting homes
    /// recorded against a different machine.
    fn owned_array_home(
        &self,
        home: &PrimitiveStorageHome,
    ) -> Result<MachineId, TerminalInterpretError> {
        let owner_machine = if home.depth == self.call_stack.len() {
            self.current_machine
        } else {
            self.call_stack
                .get(home.depth)
                .ok_or_else(malformed)?
                .current_machine
        };
        if owner_machine != home.machine {
            return Err(malformed());
        }
        Ok(owner_machine)
    }

    /// The suspended frame at `depth`, or the malformed-state error.
    fn frame_at_depth(&self, depth: usize) -> Result<&SuspendedCall, TerminalInterpretError> {
        self.call_stack.get(depth).ok_or_else(malformed)
    }
}

pub(crate) fn scalar_reference_observation(
    stored: TerminalScalarValue,
    target: ScalarType,
) -> Result<TerminalScalarValue, TerminalInterpretError> {
    use terminal_semantics::ScalarViewValue;
    let input = crate::values::scalar_view_value(&stored);
    let observed =
        terminal_semantics::scalar_reference_view_value(stored.scalar_type(), target, input)
            .ok_or(malformed())?;
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
        _ => Err(malformed()),
    }
}
