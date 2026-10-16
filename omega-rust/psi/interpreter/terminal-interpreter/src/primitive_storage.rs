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

/// A scalar cell keeps its selected namespace and backing declaration together.
/// Resolving the carrier has already checked access and reference custody.
pub(crate) struct ObservedScalarField<'a> {
    pub(crate) carrier: TerminalStructuralValue,
    pub(crate) leaf: TerminalStructuralValue,
    pub(crate) declaration: &'a terminal_psi::StructuralFieldDeclaration,
    pub(crate) stored_type: ScalarType,
    pub(crate) cell: StructuralScalarRuntimeField,
}

/// A final Case is the carrier's field namespace, not a separate runtime value.
/// Resolve the preceding projection before selecting that namespace's cell.
pub(crate) fn scalar_field_projection(
    path: &[StructuralPathSegment],
) -> Option<(&[StructuralPathSegment], &[StructuralPathSegment])> {
    let (StructuralPathSegment::Field(_), carrier) = path.split_last()? else {
        return None;
    };
    let split = if matches!(carrier.last(), Some(StructuralPathSegment::Case(_))) {
        carrier.len() - 1
    } else {
        carrier.len()
    };
    Some(path.split_at(split))
}

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

impl PrimitiveStorageHome {
    /// The activation that owns the lent scalar array. Byte windows reuse this
    /// home; they do not allocate a second payload.
    pub(crate) fn scalar_array_frame(self) -> (usize, PlaceId, StructuralTypeId) {
        (self.depth, self.place, self.structural_type)
    }
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
    pub(crate) encoding: Option<PrimitiveStorageEncoding>,
}

/// Decode the original cell into physical bits, then observe those same bits
/// through the view. Keeping both carriers prevents a narrow view from trying
/// to convert the wider logical source value directly into its interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PrimitiveStorageEncoding {
    pub(crate) source: ScalarType,
    pub(crate) view: ScalarType,
}

impl PrimitiveStorageEncoding {
    /// Host replacement checks the original field in its source-domain value,
    /// not the view's logical scalar. This is the same inverse byte image that
    /// eventual writeback will store; it establishes no new predicate or loan.
    pub(crate) fn source_value(
        self,
        value: TerminalScalarValue,
        source: ScalarType,
    ) -> Result<TerminalScalarValue, TerminalInterpretError> {
        let bits = encoding::encode(self.view, value)?;
        encoding::convert_scalar(encoding::decode(self.source, bits)?, source)
    }
}

impl PrimitiveStorage {
    fn canonical(endpoint: PrimitiveStorageEndpoint) -> Self {
        Self {
            endpoint,
            encoding: None,
        }
    }

    /// Host snapshots expose the observed physical carrier; the pinned
    /// endpoint still retains the original codec for eventual writeback.
    pub(crate) fn storage_scalar(&self) -> Option<ScalarType> {
        self.encoding.map(|encoding| encoding.view)
    }
}

struct PreparedPrimitiveWrite {
    destination: PlaceId,
    endpoint: PrimitiveStorageEndpoint,
    value: TerminalScalarValue,
}

/// One content-independent storage transaction. Preparation checks every owner,
/// extent and codec before any write; commit cannot call user code or rebind a
/// frame. Ordinary scalar stores and completed aggregate replacements share it.
#[derive(Default)]
pub(crate) struct PrimitiveStorageWrites {
    first_write: Option<PreparedPrimitiveWrite>,
    remaining_writes: Vec<PreparedPrimitiveWrite>,
}

impl PrimitiveStorageWrites {
    pub(crate) fn stage(
        &mut self,
        execution: &TerminalExecution,
        destination: PlaceId,
        storage: PrimitiveStorage,
        scalar_type: ScalarType,
        value: TerminalScalarValue,
    ) -> Result<(), TerminalInterpretError> {
        if value.scalar_type() != scalar_type || !terminal_scalar_belongs_to_type(value) {
            return Err(malformed());
        }
        if let Some(encoding) = storage.encoding {
            let value = if encoding.view != encoding.source {
                encoding.source_value(value, encoding.source)?
            } else {
                value
            };
            return execution.prepare_encoded_primitive(
                self,
                destination,
                &storage.endpoint,
                encoding.source,
                value,
            );
        }
        match &storage.endpoint {
            PrimitiveStorageEndpoint::OwnedArrayElement { home, index } => {
                execution.owned_array_home(home)?;
                let arrays = if home.depth == execution.call_stack.len() {
                    &execution.scalar_array_values
                } else {
                    &execution.frame_at_depth(home.depth)?.scalar_array_values
                };
                let stored = arrays
                    .get(&home.place)
                    .filter(|array| array.structural_type == home.structural_type)
                    .and_then(|array| array.elements.get(*index))
                    .ok_or(TerminalInterpretError::StructuralPrimitiveStorageMissing(
                        destination,
                    ))?;
                if stored.scalar_type() != scalar_type {
                    return Err(malformed());
                }
            }
            PrimitiveStorageEndpoint::Scalar(home) => {
                // A live primitive home may be awaiting its first payload.
                if execution
                    .structural_primitive_storage
                    .get(home)
                    .is_some_and(|stored| stored.scalar_type() != scalar_type)
                {
                    return Err(malformed());
                }
            }
            PrimitiveStorageEndpoint::ScalarField(field) => {
                let stored = execution.structural_scalar_fields.get(field).ok_or(
                    TerminalInterpretError::StructuralScalarFieldMissing {
                        source: destination,
                        field: field.field,
                    },
                )?;
                if stored.scalar_type() != scalar_type {
                    return Err(malformed());
                }
            }
            PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                let TerminalScalarValue::Integer {
                    value: IntegerValue::Unsigned(byte),
                    ..
                } = value
                else {
                    return Err(malformed());
                };
                u8::try_from(byte).map_err(|_| malformed())?;
                if *index >= execution.stored_byte_array_length(array)? {
                    return Err(malformed());
                }
            }
            PrimitiveStorageEndpoint::ViewByte { .. } => return Err(malformed()),
        }
        let write = PreparedPrimitiveWrite {
            destination,
            endpoint: storage.endpoint,
            value,
        };
        if self.first_write.is_none() {
            self.first_write = Some(write);
        } else {
            self.remaining_writes.push(write);
        }
        Ok(())
    }

    pub(crate) fn commit(
        self,
        execution: &mut TerminalExecution,
    ) -> Result<(), TerminalInterpretError> {
        for write in self.first_write.into_iter().chain(self.remaining_writes) {
            execution.commit_primitive_store(write.destination, write.endpoint, write.value)?;
        }
        Ok(())
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

/// A resolved byte endpoint is storage, not a newly formed loan. Loan formation
/// supplies its declared primitive type; ordinary observations consume only the
/// endpoint and its existing descriptor/window authority.
struct ByteElementStorage {
    storage: PrimitiveStorage,
    referent: TerminalStructuralValue,
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
        } else if let Some(mut element) = self.byte_element_storage(argument)? {
            element.referent.structural_type = parameter.structural_type;
            (
                element.referent,
                element.storage,
                ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?,
                ),
                parameter.access == StructuralAccess::SharedBorrow,
            )
        } else if self.scalar_array_values.contains_key(&argument.place) {
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
            let projected = self.resolve_live_primitive_projection(
                argument.place,
                &argument.path,
                argument.access,
                parameter.structural_type,
            )?;
            let value = projected.0;
            if value.structural_type != parameter.structural_type {
                return Err(malformed());
            }
            let (storage, scalar_type, shared_view) = match projected.1 {
                Some(storage) => (
                    storage,
                    expected,
                    parameter.access == StructuralAccess::SharedBorrow,
                ),
                None => self.primitive_view_access(
                    &value,
                    &[],
                    expected,
                    parameter.access == StructuralAccess::SharedBorrow,
                )?,
            };
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

    /// Byte loans select a descriptor coordinate, then its live window. The
    /// descriptor may be a root or an inline field; neither creates scalar
    /// storage or copies the byte when passed to another activation.
    fn byte_element_storage(
        &self,
        argument: &StructuralArgument,
    ) -> Result<Option<ByteElementStorage>, TerminalInterpretError> {
        let Some((StructuralPathSegment::FixedIndex(index), prefix)) = argument.path.split_last()
        else {
            return Ok(None);
        };
        let types = self.program.structural_types();
        let root = self.live_structural_type(argument.place)?;
        if !terminal_semantics::runtime_structural_path_tip(types.values(), root, prefix)
            .is_some_and(|row| {
                row.shape.is_borrowed_byte_carrier() || row.shape.is_bounded_owned_byte_sequence()
            })
        {
            return Ok(None);
        }
        let source = StructuralArgument {
            place: argument.place,
            path: prefix.to_vec(),
            access: argument.access,
        };
        self.validate_descriptor_borrow(&source, argument.access)?;
        let descriptor = self.resolve_live_projection(source.place, prefix, source.access)?;
        let binding = self.byte_descriptor_binding(&source, &descriptor)?;
        self.bound_byte_element_storage(&descriptor, &binding, *index, argument.access)
            .map(Some)
    }

    /// Current and suspended activations resolve their own descriptor binding,
    /// then share the same live-window and original-backing judgment.
    fn bound_byte_element_storage(
        &self,
        descriptor: &TerminalStructuralValue,
        binding: &crate::byte_sequences::binding::ByteSequenceBinding,
        index: u64,
        access: StructuralAccess,
    ) -> Result<ByteElementStorage, TerminalInterpretError> {
        use crate::byte_sequences::binding::ByteSequenceBinding;
        if index >= self.byte_descriptor_length(binding, descriptor)? {
            return Err(malformed());
        }
        let mut referent = descriptor.clone();
        referent.qualifications.clear();
        referent.projected_qualifications.clear();
        referent.path.push(StructuralPathSegment::FixedIndex(index));
        let endpoint = match binding {
            ByteSequenceBinding::Immutable(bytes) if access == StructuralAccess::SharedBorrow => {
                PrimitiveStorageEndpoint::ViewByte {
                    bytes: bytes.clone(),
                    index: usize::try_from(index).map_err(|_| malformed())?,
                }
            }
            ByteSequenceBinding::MutableField { .. } | ByteSequenceBinding::MutableOwner { .. }
                if access == StructuralAccess::SharedBorrow =>
            {
                // Observe this loan's current owner bytes, not the descriptor
                // object. Shared observation prevents intervening mutation;
                // it uses the same immutable backing as an ordinary byte view.
                let (bytes, _) = self.exclusive_owner_bytes(binding)?;
                PrimitiveStorageEndpoint::ViewByte {
                    bytes: bytes.clone(),
                    index: usize::try_from(index).map_err(|_| malformed())?,
                }
            }
            ByteSequenceBinding::MutableArray {
                array,
                array_type,
                offset,
                ..
            } => {
                let extent = crate::structural_inputs::byte_arrays::byte_array_length(
                    &self.program.structural_types(),
                    *array_type,
                )
                .ok_or_else(malformed)?;
                if self.stored_byte_array_length(array)? as u128 != u128::from(extent) {
                    return Err(malformed());
                }
                let index = offset.checked_add(index).ok_or_else(malformed)?;
                referent.opaque_identity = array.referent.opaque_identity;
                referent.path = array.referent.path.clone();
                referent.path.push(StructuralPathSegment::FixedIndex(index));
                PrimitiveStorageEndpoint::ArrayElement {
                    array: array.clone(),
                    index: usize::try_from(index).map_err(|_| malformed())?,
                }
            }
            _ => return Err(malformed()),
        };
        Ok(ByteElementStorage {
            storage: PrimitiveStorage::canonical(endpoint),
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
        self.primitive_home_access(self.capture_primitive_home(place)?, writing, path)
    }

    /// A stored reference crosses into its captured backing independently of
    /// the activation that currently holds its descriptor. Collection loans
    /// retain their supplying home, so a callee must resolve that suspended
    /// caller's pointer before selecting the primitive suffix.
    fn reference_primitive_access(
        &self,
        owner: &TerminalStructuralValue,
        writing: bool,
        path: &[StructuralPathSegment],
    ) -> Result<Option<(PrimitiveStorage, ScalarType, bool)>, TerminalInterpretError> {
        if let Some(crossing) = path
            .iter()
            .rposition(|segment| *segment == StructuralPathSegment::Referent)
        {
            let access = if writing {
                // A pure store needs no observation authority. Request the
                // least access that performs it, so a write-only descriptor
                // remains usable without being widened to mutable access.
                StructuralAccess::WriteOnlyBorrow
            } else {
                StructuralAccess::SharedBorrow
            };
            // Descriptor traversal already retained the physical endpoint.
            // Do not discard that association and reinterpret its presentation
            // identity as a separately owned scalar allocation.
            let captured = self
                .resolve_reference_storage_projection(owner, path, access)?
                .ok_or_else(malformed)?;
            if let Some(storage) = captured.primitive_storage {
                let projection_type = if writing {
                    terminal_semantics::primitive_projection_type
                } else {
                    terminal_semantics::primitive_read_projection_type
                };
                let scalar_type = projection_type(
                    self.program.structural_types().values(),
                    owner.structural_type,
                    path,
                )
                .ok_or_else(malformed)?;
                return Ok(Some((storage, scalar_type, !writing)));
            }
            // Resolve the captured pointer before applying the primitive
            // suffix. Its descriptor home is never the referent's storage.
            let (referent, view) = self
                .resolve_reference_view_path(owner, &path[..=crossing], access)?
                .ok_or_else(malformed)?;
            if let Some(view) = view {
                return self
                    .aggregate_view_primitive_access(&referent, view, &path[crossing + 1..])
                    .map(Some);
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
                return Ok(Some((storage, scalar_type, !writing)));
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
                let (carrier, _) = self
                    .resolve_reference_view_path(owner, &path[..crossing], access)?
                    .ok_or_else(malformed)?;
                if let Some(captured) = self
                    .reference_referents
                    .get(&StructuralRuntimePlace::from(&carrier))
                    && let Some(loan) = &captured.primitive_loan
                {
                    if captured.view != loan.referent
                        || (writing
                            && !matches!(
                                loan.access,
                                terminal_psi::exclusive_borrow_pair_pattern!()
                            ))
                    {
                        return Err(malformed());
                    }
                    return Ok(Some((loan.storage.clone(), scalar_type, !writing)));
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
            return self
                .primitive_view_access(&referent, canonical.as_slice(), scalar_type, false)
                .map(Some);
        }
        Ok(None)
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
    /// backing cell occupying its complete captured byte region. When they carry
    /// it differently (`i16` seen as `u16`), the view's carrier becomes the
    /// stored interpretation, so reads take the modular image and writes
    /// store the image of the view's value through the existing codec.
    pub(crate) fn aggregate_view_primitive_access(
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
        let (suffix, tip) = crate::reference::translate_aggregate_view_path(
            &types, view, backing, suffix,
        )
        .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
            "aggregate re-view has no unique contiguous scalar endpoint",
        ))?;
        let crate::reference::AggregateViewTip::Scalar {
            view: observed,
            backing: stored,
        } = tip
        else {
            return Err(malformed());
        };
        let canonical = crate::reference::backing_scalar_path(&types, backing, &suffix).ok_or(
            TerminalInterpretError::UnsupportedSemanticVariant(
                "aggregate re-view backing scalar has no retained cell coordinate",
            ),
        )?;
        let (mut storage, _, shared_view) = self.primitive_view_access(
            referent,
            canonical.as_slice(),
            stored.logical_scalar,
            false,
        )?;
        if storage
            .encoding
            .is_some_and(|encoding| encoding.source != stored.stored_scalar)
            || !terminal_semantics::reference_storage_view_is_compatible(
                observed.stored_scalar,
                observed.logical_scalar,
            )
        {
            return Err(malformed());
        }
        if stored.stored_scalar != stored.logical_scalar
            || observed.logical_scalar != stored.logical_scalar
            || observed.stored_scalar != stored.stored_scalar
        {
            storage.encoding = Some(PrimitiveStorageEncoding {
                source: stored.stored_scalar,
                view: observed.stored_scalar,
            });
        }
        Ok((storage, observed.logical_scalar, shared_view))
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

    /// Read and storage observations share the declared home contract. A
    /// physically mutable backing never widens a shared or write-only formal.
    fn primitive_home_contract(
        &self,
        home: PrimitiveStorageHome,
        writing: bool,
        path: &[StructuralPathSegment],
    ) -> Result<(StructuralTypeId, bool), TerminalInterpretError> {
        let machine_id = if home.depth == self.call_stack.len() {
            self.current_machine
        } else {
            self.call_stack
                .get(home.depth)
                .ok_or_else(malformed)?
                .current_machine
        };
        if machine_id != home.machine {
            return Err(malformed());
        }
        let machine = self
            .program
            .machines()
            .get(&machine_id)
            .ok_or_else(malformed)?;
        let preserves =
            |root,
             whole: &[semantic_vocabulary::StructuralDomainId],
             projected: &[terminal_psi::StructuralPathQualification]| {
                terminal_semantics::primitive_store_restriction(
                    self.program.structural_types().values(),
                    self.program.catalogs.structural_domains.iter(),
                    root,
                    whole,
                    projected,
                    path,
                )
                .is_some()
            };
        let place = home.place;
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
                                    terminal_psi::exclusive_borrow_pair_pattern!()
                                )
                            } else {
                                matches!(
                                    parameter.access,
                                    terminal_psi::reading_borrow_pair_pattern!()
                                )
                            })
                            // Reads select a copyable leaf below this declared
                            // home; the complete path checks that type below.
                            // Owner custody and domains remain on the owner,
                            // not a restriction on observing another field.
                            && (!writing || ((parameter.multiplicity == StructuralMultiplicity::Unrestricted
                                || (!path.is_empty() && parameter.multiplicity == StructuralMultiplicity::Affine))
                                && preserves(parameter.structural_type, &parameter.qualifications, &parameter.projected_qualifications)))
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
                            && (!writing
                                || (result.claims.is_empty()
                                    && preserves(result.structural_type, &result.qualifications, &result.projected_qualifications)))
                    })
                    .map(|result| (result.structural_type, false))
            })
            .ok_or_else(malformed)?;
        if structural_type != home.structural_type {
            return Err(malformed());
        }
        Ok((structural_type, shared_view))
    }

    /// Runtime selectors have already become fixed coordinates. The catalog
    /// still names a dynamic descriptor, so its live window, not a static array
    /// path judgment, supplies the selected element's bound. Descriptor lookup
    /// retains the exact activation home and every preceding reference crossing.
    fn element_view_scalar_read(
        &self,
        home: PrimitiveStorageHome,
        path: &[StructuralPathSegment],
        expected: ScalarType,
    ) -> Result<Option<TerminalScalarValue>, TerminalInterpretError> {
        let types = self.program.structural_types();
        for (position, segment) in path.iter().enumerate() {
            let StructuralPathSegment::FixedIndex(index) = segment else {
                continue;
            };
            let Some(descriptor) = terminal_semantics::runtime_structural_path_tip(
                types.values(),
                home.structural_type,
                &path[..position],
            ) else {
                continue;
            };
            if !matches!(
                descriptor.shape,
                terminal_psi::StructuralTypeShape::ElementView { .. }
            ) {
                continue;
            }
            self.primitive_home_contract(home, false, path)?;
            let homes = if home.depth == self.call_stack.len() {
                &self.structural_values
            } else {
                &self
                    .call_stack
                    .get(home.depth)
                    .ok_or_else(malformed)?
                    .structural_values
            };
            let owner = homes.get(&home.place).ok_or_else(malformed)?;
            if owner.structural_type != home.structural_type {
                return Err(malformed());
            }
            // This operation demands one exact readable path. An inactive
            // case is not another storage candidate to try after this read.
            let (selected, aggregate_view) = self
                .resolve_reference_view_path(
                    owner,
                    &path[..position],
                    StructuralAccess::SharedBorrow,
                )?
                .ok_or_else(malformed)?;
            if selected.structural_type != descriptor.id || aggregate_view.is_some() {
                return Err(malformed());
            }
            let view = self.element_view_at_resolved_home(&selected)?;
            let value = self.read_element_view_scalar(
                view,
                usize::try_from(*index).map_err(|_| malformed())?,
                &path[position + 1..],
                expected,
            )?;
            return Ok(Some(value));
        }
        Ok(None)
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
            aggregate_argument_backings,
        ) = if home.depth == self.call_stack.len() {
            (
                self.current_machine,
                &self.structural_values,
                &self.scalar_array_values,
                &self.byte_sequence_values,
                &self.primitive_loan_values,
                &self.element_view_values,
                &self.aggregate_argument_backings,
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
                &frame.aggregate_argument_backings,
            )
        };
        if machine_id != home.machine {
            return Err(malformed());
        }
        // Resolve current descriptor projections before canonical scalar paths:
        // a reference to a byte view names its descriptor, whose element lives
        // in the captured byte window, not a scalar cell below that descriptor.
        if home.depth == self.call_stack.len()
            && let Some(element) = self.byte_element_storage(&StructuralArgument {
                place,
                path: path.to_vec(),
                access: if writing {
                    StructuralAccess::MutableBorrow
                } else {
                    StructuralAccess::SharedBorrow
                },
            })?
        {
            let (_, shared_view) = self.primitive_home_contract(home, writing, path)?;
            return Ok((
                element.storage,
                ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?,
                ),
                shared_view,
            ));
        }
        if path.contains(&StructuralPathSegment::Referent) {
            self.primitive_home_contract(home, writing, path)?;
            let owner = structural_values.get(&place).ok_or_else(malformed)?;
            if owner.structural_type != home.structural_type {
                return Err(malformed());
            }
            if let Some(storage) = self.reference_primitive_access(owner, writing, path)? {
                return Ok(storage);
            }
        }
        if let Some(loan) = element_view_values.get(&place) {
            self.primitive_home_contract(home, writing, path)?;
            let owner = structural_values.get(&place).ok_or_else(malformed)?;
            if owner.structural_type != home.structural_type || !loan.matches_descriptor_home(owner)
            {
                return Err(malformed());
            }
            // Fixed arrays and dynamic windows lend the same backing cells.
            // The live descriptor owns the extent; its activation home owns
            // the remaining typed projection and stored scalar carrier.
            return loan.scalar_storage(self, path, writing);
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
                    !matches!(loan.access, terminal_psi::exclusive_borrow_pair_pattern!())
                } else {
                    !matches!(loan.access, terminal_psi::reading_borrow_pair_pattern!())
                }
            {
                return Err(malformed());
            }
            // Ordinary ingress already binds the exact qualified view. This
            // endpoint association does not erase its qualifiers or convert
            // stored payload; host replacement admission remains separate.
            return Ok((loan.storage.clone(), loan.scalar_type, loan.shared_view));
        }
        let (structural_type, shared_view) = self.primitive_home_contract(home, writing, path)?;
        if let Some(backing) = aggregate_argument_backings.get(&place) {
            // The home contract has already authenticated this activation's
            // access and logical path. Translate that path into its retained
            // supplying declaration, exactly as a descriptor crossing does.
            let (storage, scalar_type, _) = self.aggregate_view_primitive_access(
                &backing.value,
                crate::reference::AggregateViewTip::Aggregate {
                    view: backing.region,
                    backing: backing.value.structural_type,
                },
                path,
            )?;
            return Ok((storage, scalar_type, shared_view));
        }
        // Suspended activations retain their original descriptor namespace.
        // Reuse the same endpoint judgment without interpreting that PlaceId
        // in the current callee's frame.
        if let Some(binding) = byte_sequence_values.get(&place) {
            let descriptor = structural_values.get(&place).ok_or_else(malformed)?;
            let [StructuralPathSegment::FixedIndex(index)] = path else {
                return Err(malformed());
            };
            let scalar_type = self.byte_element_scalar(structural_type, path)?;
            let byte_type = IntegerType::new(IntegerSign::Unsigned, 8).map_err(|_| malformed())?;
            if descriptor.structural_type != structural_type
                || scalar_type != ScalarType::Integer(byte_type)
            {
                return Err(malformed());
            }
            let element = self.bound_byte_element_storage(
                descriptor,
                binding,
                *index,
                if writing {
                    StructuralAccess::MutableBorrow
                } else {
                    StructuralAccess::SharedBorrow
                },
            )?;
            return Ok((element.storage, scalar_type, shared_view));
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
                self.scalar_leaf_field(view, *identity, parent_path, scalar_type, shared_view)?;
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
        identity: terminal_psi::TerminalIdentityIndex,
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
            terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
            _ => return Err(malformed()),
        };
        let field = terminal_psi::live_field_by_identity(fields, identity).ok_or_else(malformed)?;
        let shape = if shared_view {
            field.field_type.shared_observation_shape()
        } else {
            // A call loan retains the original field endpoint while presenting
            // the receiving scalar declaration. Its handback predicate was
            // checked by Psi; storage identity does not require manufacturing
            // an owned restricted scalar declaration for a bare formal.
            let types = self.program.structural_types();
            let expected = types.get(&view.structural_type).ok_or_else(malformed)?;
            let selected = field
                .field_type
                .borrowed_presentation_type(StructuralAccess::MutableBorrow, Some(expected))
                .and_then(|leaf| leaf.leaf_declaration(types.values()))
                .ok_or_else(malformed)?;
            if selected.id != view.structural_type {
                return Err(malformed());
            }
            Some(selected.shape.clone())
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
        let restriction = self
            .program
            .structural_types()
            .get(&result.structural_type)
            .and_then(|declaration| declaration.scalar_restriction.as_ref());
        if scalar.scalar_type() != *expected
            || !terminal_scalar_belongs_to_type(scalar)
            || restriction.is_some_and(|restriction| !scalar.satisfies_field(restriction))
        {
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

    /// Join an authority-resolved carrier to its selected scalar cell. A case
    /// namespace stays with the declaration through re-view translation and
    /// active-tag validation; common members and payload fields never alias.
    pub(crate) fn scalar_field_at_carrier(
        &self,
        mut carrier: TerminalStructuralValue,
        view: Option<crate::reference::AggregateViewTip>,
        field_path: &[StructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<Option<ObservedScalarField<'_>>, TerminalInterpretError> {
        let translated;
        let (field_path, stored_type) = match view {
            None => (field_path, scalar_type),
            Some(crate::reference::AggregateViewTip::Aggregate { view, backing })
                if backing == carrier.structural_type =>
            {
                let (path, tip) = crate::reference::translate_aggregate_view_path(
                    &self.program.structural_types(),
                    view,
                    backing,
                    field_path,
                )
                .ok_or_else(malformed)?;
                let crate::reference::AggregateViewTip::Scalar {
                    view: observed,
                    backing: stored,
                } = tip
                else {
                    return Ok(None);
                };
                if observed.logical_scalar != scalar_type {
                    return Err(malformed());
                }
                let Some((prefix, namespace)) = scalar_field_projection(&path) else {
                    return Ok(None);
                };
                carrier = crate::custody::project_shared_structural_observation(
                    &self.program.structural_types(),
                    &carrier,
                    prefix,
                )?;
                translated = namespace.to_vec();
                (translated.as_slice(), stored.logical_scalar)
            }
            Some(_) => return Err(malformed()),
        };
        let Some(field) = terminal_semantics::runtime_structural_field(
            self.program.structural_types().values(),
            carrier.structural_type,
            field_path,
        ) else {
            return Ok(None);
        };
        // Structural children use the primitive-leaf route; atomic fields
        // cannot gain an ordinary scalar observation through this resolver.
        if matches!(
            field.field_type,
            terminal_psi::StructuralFieldType::Atomic(_)
        ) {
            return Err(malformed());
        }
        let Some(declared) = field.field_type.scalar_type() else {
            return Ok(None);
        };
        if declared != stored_type {
            return Err(malformed());
        }
        let (_, namespace) = field_path.split_last().ok_or_else(malformed)?;
        let namespace = match namespace {
            [] => Vec::new(),
            [StructuralPathSegment::Case(case)] => {
                vec![semantic_vocabulary::CanonicalStructuralPathSegment::Case(
                    *case,
                )]
            }
            _ => return Err(malformed()),
        };
        let cell = self.observed_field_cell_in(&carrier, None, &namespace, field.id)?;
        let mut leaf = carrier.clone();
        leaf.path.extend_from_slice(field_path);
        Ok(Some(ObservedScalarField {
            carrier,
            leaf,
            declaration: field,
            stored_type,
            cell,
        }))
    }

    /// Observe a selected field under its original backing. Admission checked
    /// the complete path before runtime selection; a suffix supplies no bounds
    /// or writable authority. Byte-region and re-view reads retain their codec.
    fn scalar_field_read(
        &self,
        source: PlaceId,
        path: &[StructuralPathSegment],
        scalar_type: ScalarType,
    ) -> Result<Option<TerminalScalarValue>, TerminalInterpretError> {
        let Some((prefix, field_path)) = scalar_field_projection(path) else {
            return Ok(None);
        };
        let (carrier, view) =
            self.resolve_live_view_projection(source, prefix, StructuralAccess::SharedBorrow)?;
        if let Some(view) = view {
            let (storage, logical_scalar, shared_view) =
                self.aggregate_view_primitive_access(&carrier, view, field_path)?;
            return self
                .read_primitive_storage(source, storage, logical_scalar, scalar_type, shared_view)
                .map(Some);
        }
        let Some(observed) =
            self.scalar_field_at_carrier(carrier, view, field_path, scalar_type)?
        else {
            return Ok(None);
        };
        if let Some((storage, logical_scalar)) =
            self.reference_byte_region_access(&observed.carrier, &observed.leaf)?
        {
            return self
                .read_primitive_storage(source, storage, logical_scalar, scalar_type, false)
                .map(Some);
        }
        let mut value = self
            .structural_scalar_fields
            .get(&observed.cell)
            .copied()
            .ok_or(TerminalInterpretError::StructuralScalarFieldMissing {
                source,
                field: observed.declaration.id,
            })?;
        if value.scalar_type() != observed.stored_type {
            return Err(malformed());
        }
        if observed.stored_type != scalar_type {
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
        if let Some(value) =
            self.element_view_scalar_read(self.capture_primitive_home(source)?, path, expected)?
        {
            return Ok(value);
        }
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
        if let Some(value) = self.element_view_scalar_read(home, path, expected)? {
            return Ok(value);
        }
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
        if let Some(encoding) = storage.encoding {
            if encoding.view != encoding.source {
                let stored = self.read_encoded_primitive(
                    source,
                    &storage.endpoint,
                    encoding.source,
                    encoding.source,
                )?;
                // An admitted representation view decodes the same physical
                // bits, including constrained integer-to-boolean views. The
                // ordinary shared-observation law cannot establish that cast.
                let bits = encoding::encode(encoding.source, stored)?;
                let viewed = encoding::decode(encoding.view, bits)?;
                return encoding::convert_scalar(viewed, expected);
            }
            return self.read_encoded_primitive(
                source,
                &storage.endpoint,
                encoding.source,
                expected,
            );
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
        let machine = self
            .program
            .machines()
            .get(&self.current_machine)
            .ok_or_else(malformed)?;
        let (root, whole, projected) = machine
            .structural_parameter(destination)
            .map(|parameter| {
                (
                    parameter.structural_type,
                    parameter.qualifications.as_slice(),
                    parameter.projected_qualifications.as_slice(),
                )
            })
            .or_else(|| {
                machine
                    .operations()
                    .filter_map(|operation| operation.result.structural())
                    .find(|result| result.place == destination)
                    .map(|result| {
                        (
                            result.structural_type,
                            result.qualifications.as_slice(),
                            result.projected_qualifications.as_slice(),
                        )
                    })
            })
            .ok_or_else(malformed)?;
        let restriction = terminal_semantics::primitive_store_restriction(
            self.program.structural_types().values(),
            self.program.catalogs.structural_domains.iter(),
            root,
            whole,
            projected,
            path,
        )
        .ok_or_else(malformed)?;
        let predicates = terminal_semantics::primitive_store_byte_predicates(
            self.program.structural_types().values(),
            self.program.catalogs.structural_domains.iter(),
            root,
            whole,
            projected,
            path,
        )
        .ok_or_else(malformed)?;
        if predicates
            .predicates()
            .any(|predicate| predicate == terminal_psi::ByteSequencePredicate::ValidUtf8)
            && !predicates
                .predicates()
                .any(|predicate| predicate == terminal_psi::ByteSequencePredicate::AsciiOnly)
        {
            self.validate_ascii_backing_for_primitive_write(
                destination,
                root,
                &path[..path.len() - 1],
            )?;
        }
        if let Some(restriction) = restriction {
            let literal = match scalar {
                TerminalScalarValue::Integer { scalar_type, value } => {
                    semantic_vocabulary::ScalarTerm::integer(scalar_type, value)
                        .map_err(|_| malformed())?
                }
                TerminalScalarValue::Boolean(value) => {
                    semantic_vocabulary::ScalarTerm::Boolean(value)
                }
                TerminalScalarValue::IeeeFloat(value) => {
                    semantic_vocabulary::ScalarTerm::IeeeFloatLiteral(value)
                }
            };
            if !terminal_semantics::scalar_field_accepts_literal(&restriction, &literal) {
                return Err(malformed());
            }
        }
        self.store_primitive_place(destination, path, scalar)
    }

    /// Check the receiving predicate against the current backing without
    /// creating an application observation or granting a write-only loan read
    /// access. Source preservation and current-graph freshness remain separate
    /// obligations; these internal bits only validate their runtime honesty.
    fn validate_ascii_backing_for_primitive_write(
        &self,
        destination: PlaceId,
        root: StructuralTypeId,
        array_path: &[StructuralPathSegment],
    ) -> Result<(), TerminalInterpretError> {
        let declaration = terminal_semantics::runtime_structural_path_tip(
            self.program.structural_types().values(),
            root,
            array_path,
        )
        .ok_or_else(malformed)?;
        let terminal_psi::StructuralTypeShape::FixedArray { length, .. } = declaration.shape else {
            return Err(malformed());
        };
        let byte_type =
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .map_err(|_| malformed())?;
        let mut selected = array_path.to_vec();
        for index in 0..length {
            selected.push(StructuralPathSegment::FixedIndex(index));
            let (storage, stored_type, shared_view) =
                self.primitive_access(destination, true, &selected)?;
            let byte = self.read_primitive_storage(
                destination,
                storage,
                stored_type,
                semantic_vocabulary::ScalarType::Integer(byte_type),
                shared_view,
            )?;
            selected.pop();
            if !matches!(byte, TerminalScalarValue::Integer { value: IntegerValue::Unsigned(value), .. } if value <= 127)
            {
                return Err(malformed());
            }
        }
        Ok(())
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
        let mut writes = PrimitiveStorageWrites::default();
        writes.stage(self, destination, storage, scalar_type, scalar)?;
        writes.commit(self)
    }

    fn commit_primitive_store(
        &mut self,
        destination: PlaceId,
        endpoint: PrimitiveStorageEndpoint,
        scalar: TerminalScalarValue,
    ) -> Result<(), TerminalInterpretError> {
        // Preparation resolved these exact cells without mutation. No frame or
        // storage topology changes between preparation and this commit.
        match endpoint {
            PrimitiveStorageEndpoint::OwnedArrayElement { home, index } => {
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
                    .and_then(|array| array.elements.get_mut(index))
                    .ok_or(TerminalInterpretError::StructuralPrimitiveStorageMissing(
                        destination,
                    ))?;
                *stored = scalar;
            }
            PrimitiveStorageEndpoint::Scalar(storage) => {
                // The resolved live owner and exact leaf carrier authorize a
                // content-independent store. A structural local establishes a
                // home before its first stores establish payload, so this
                // operation must not require the previous scalar to exist.
                // Reads still require an initialized cell; this creates no
                // owner, loan, predicate membership, or implicit zero value.
                self.structural_primitive_storage.insert(storage, scalar);
            }
            PrimitiveStorageEndpoint::ScalarField(field) => {
                let stored = self.structural_scalar_fields.get_mut(&field).ok_or(
                    TerminalInterpretError::StructuralScalarFieldMissing {
                        source: destination,
                        field: field.field,
                    },
                )?;
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
    /// closed rather than overwriting it. The selected edge has already
    /// removed outgoing owned homes: their captured backing belongs to the
    /// destination and must not be reclaimed through the retired source name.
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
