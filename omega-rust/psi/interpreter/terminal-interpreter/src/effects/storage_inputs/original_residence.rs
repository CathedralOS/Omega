//! Realize only authenticated exposed cells in their original owned allocation.
//! All host replies validate before local payloads are replaced by residents.
//! The ordinary staged restoration path remains with its existing owner.

use super::{BoundaryStorageInputs, TerminalBoundaryStorageBuffer};
use crate::{
    effects::{TerminalEffectHandler, original_storage::*},
    errors::{TerminalInterpretError, malformed},
    execution::TerminalExecution,
    execution::frame_field_mut,
    primitive_storage::{ArrayCell, PrimitiveStorage, PrimitiveStorageEndpoint},
    scalar_storage::ScalarCell,
    values::{StructuralFieldNamespace, StructuralRuntimePlace, TerminalStructuralValue},
};
use semantic_vocabulary::{BoundaryMachineId, CanonicalStructuralPathSegment as Path};
use terminal_psi::StructuralAccess;

struct CapturedOriginal {
    owner: Option<StructuralRuntimePlace>,
    request: TerminalOriginalStorageRequest,
    endpoints: Vec<PrimitiveStorageEndpoint>,
}

fn capture_byte_view(
    execution: &mut TerminalExecution<'_>,
    binding: &super::byte_views::BoundaryByteViewBinding,
    identity_cursor: &mut Option<u64>,
) -> Result<CapturedOriginal, TerminalInterpretError> {
    let (mut capture, offset, length) = if let Some(window) = binding.array_window()? {
        let original = execution
            .original_byte_array_owner(&window.array, identity_cursor)
            .map_err(|error| error.in_argument_preparation("resolve original byte-array owner"))?;
        // The selected array has u8 geometry, but its allocation can be a
        // containing record. Each scalar below retains its exact original-root
        // path; only the native layout owner derives physical byte offsets.
        if crate::structural_inputs::byte_arrays::byte_array_length(
            &execution.program.structural_types(),
            window.array_type,
        )
        .is_none()
        {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "original byte-view window requires its actual whole-u8 array geometry",
            ));
        }
        let end = window
            .offset
            .checked_add(window.length)
            .ok_or_else(malformed)?;
        if end > execution.stored_byte_array_length(&window.array)? {
            return Err(malformed());
        }
        if window.length != binding.entry_bytes().len() {
            return Err(malformed());
        }
        if binding.access == StructuralAccess::MutableBorrow
            && !execution.normal_qualifications_cover(
                &original,
                &original.qualifications,
                &original.projected_qualifications,
                &[],
            )?
        {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "mutable original byte array lacks its owner qualification preservation",
            ));
        }
        let mut values = Vec::new();
        let mut endpoints = Vec::new();
        for index in window.offset..end {
            let endpoint = PrimitiveStorageEndpoint::ArrayElement {
                array: window.array.clone(),
                index,
            };
            let storage = PrimitiveStorage {
                endpoint: endpoint.clone(),
                encoding: None,
            };
            if binding.access == StructuralAccess::MutableBorrow {
                execution.validate_scalar_loan_qualifications(&storage)?;
            }
            let value = execution
                .original_scalar(&original, &storage)
                .map_err(|error| {
                    error.in_argument_preparation("capture original byte-array cell")
                })?;
            if crate::structural_inputs::byte_arrays::storage::scalar_byte(&value.value)?
                != binding.entry_bytes()[index - window.offset]
            {
                return Err(malformed());
            }
            values.push(value);
            endpoints.push(endpoint);
        }
        let request = execution.original_storage_request(
            &original,
            binding.argument_index,
            binding.access,
            Vec::new(),
            values,
        )?;
        (
            CapturedOriginal {
                owner: Some(StructuralRuntimePlace::from(&original)),
                request,
                endpoints,
            },
            window.offset,
            window.length,
        )
    } else {
        let bytes = binding.raw_bytes()?;
        let (namespace, extent, offset) = bytes.original_byte_allocation().ok_or(
            TerminalInterpretError::UnsupportedSemanticVariant(
                "original byte allocation identity is exhausted",
            ),
        )?;
        let length = bytes.len();
        if binding.access != StructuralAccess::SharedBorrow
            || offset.checked_add(length).is_none_or(|end| end > extent)
        {
            return Err(malformed());
        }
        let scalar_type =
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .map_err(|_| malformed())?;
        let values = bytes
            .bytes()
            .iter()
            .enumerate()
            .map(|(relative, byte)| {
                let index = offset.checked_add(relative).ok_or_else(malformed)?;
                let backing = execution
                    .original_byte_cells
                    .get(&(namespace, index))
                    .map(|cell| {
                        if !cell.owner.shares_backing(bytes) {
                            return Err(malformed());
                        }
                        Ok(cell.backing.clone())
                    })
                    .transpose()?;
                Ok(TerminalOriginalScalarValue {
                    path: vec![Path::FixedIndex(
                        u64::try_from(index).map_err(|_| malformed())?,
                    )],
                    value: crate::TerminalScalarValue::Integer {
                        scalar_type,
                        value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(*byte)),
                    },
                    backing,
                })
            })
            .collect::<Result<Vec<_>, TerminalInterpretError>>()?;
        (
            CapturedOriginal {
                owner: None,
                request: TerminalOriginalStorageRequest {
                    root: TerminalOriginalStorageRoot {
                        artifact: execution.program.identity.semantic(),
                        namespace,
                        allocation: 1,
                    },
                    structural_type: binding.view_type(),
                    argument_index: binding.argument_index,
                    referent: Vec::new(),
                    access: binding.access,
                    values,
                    argument_path: Vec::new(),
                    view_type: binding.view_type(),
                    references: Vec::new(),
                    scalar_projection: None,
                    byte_extent: Some(extent),
                    byte_window: None,
                },
                endpoints: Vec::new(),
            },
            offset,
            length,
        )
    };
    capture.request.view_type = binding.view_type();
    capture.request.byte_window = Some((offset, length));
    capture.request.scalar_projection = Some(
        capture
            .request
            .values
            .iter()
            .enumerate()
            .map(|(relative, value)| {
                Ok((
                    vec![Path::FixedIndex(
                        u64::try_from(relative).map_err(|_| malformed())?,
                    )],
                    value.path.clone(),
                ))
            })
            .collect::<Result<Vec<_>, TerminalInterpretError>>()?,
    );
    Ok(capture)
}

impl BoundaryStorageInputs {
    pub(crate) fn prepare_original_storage(
        &mut self,
        execution: &mut TerminalExecution<'_>,
        handler: &mut impl TerminalEffectHandler,
        boundary: BoundaryMachineId,
        identity_cursor: &mut Option<u64>,
        original: bool,
    ) -> Result<(), TerminalInterpretError> {
        if !original {
            return Ok(());
        }
        let mut captures = Vec::new();
        for binding in &self.scalars.bindings {
            let original = execution.original_scalar_owner(&binding.storage, identity_cursor)?;
            if binding.access == StructuralAccess::MutableBorrow
                && !execution.normal_qualifications_cover(
                    &original,
                    &original.qualifications,
                    &original.projected_qualifications,
                    &[],
                )?
            {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "mutable original scalar storage lacks its owner qualification preservation",
                ));
            }
            let value = execution.original_scalar(&original, &binding.storage)?;
            captures.push(CapturedOriginal {
                owner: Some(StructuralRuntimePlace::from(&original)),
                request: execution.original_storage_request(
                    &original,
                    binding.argument_index,
                    binding.access,
                    value.path.clone(),
                    vec![value],
                )?,
                endpoints: vec![binding.storage.endpoint.clone()],
            });
        }
        for binding in &self.structural {
            capture_graph(
                execution,
                binding.source,
                &binding.source_path,
                &binding.referent,
                binding.aggregate.as_ref(),
                Some(&binding.scalars),
                identity_cursor,
                binding.argument_index,
                binding.access,
                &binding.normal_qualifications,
                Vec::new(),
                &mut Vec::new(),
                &mut captures,
            )?;
        }
        for binding in &self.byte_views {
            captures.push(
                capture_byte_view(execution, binding, identity_cursor)
                    .map_err(|error| error.in_argument_preparation("capture original byte view"))?,
            );
        }
        let owners = captures
            .iter()
            .map(|capture| capture.owner.clone())
            .collect::<Vec<_>>();
        let (requests, endpoints): (Vec<_>, Vec<_>) = captures
            .into_iter()
            .map(|capture| (capture.request, capture.endpoints))
            .unzip();
        let returned = handler
            .prepare_original_storage(boundary, &requests)
            .map_err(TerminalInterpretError::ResidentStorage)?;
        if returned.len() != requests.len() {
            return Err(malformed());
        }
        let mut prepared = Vec::new();
        let mut raw_prepared = Vec::new();
        let mut raw_cells: crate::runtime_rows::RuntimeRows<(u64, usize), OriginalByteCell> =
            crate::runtime_rows::RuntimeRows::new(execution.account.as_ref());
        for ((request, endpoints), backings) in requests.iter().zip(endpoints).zip(returned) {
            if backings.len() != request.values.len() {
                return Err(malformed());
            }
            if request.byte_extent.is_some() {
                if !endpoints.is_empty() || request.access != StructuralAccess::SharedBorrow {
                    return Err(malformed());
                }
                for (value, backing) in request.values.iter().zip(&backings) {
                    ScalarCell::resident(value.value, backing.clone())?;
                    let [Path::FixedIndex(index)] = value.path.as_slice() else {
                        return Err(malformed());
                    };
                    let key = (
                        request.root.namespace,
                        usize::try_from(*index).map_err(|_| malformed())?,
                    );
                    let owner = self
                        .byte_views
                        .iter()
                        .find(|binding| binding.argument_index == request.argument_index)
                        .ok_or_else(malformed)?
                        .raw_bytes()?;
                    if let Some(cell) = execution
                        .original_byte_cells
                        .get(&key)
                        .or_else(|| raw_cells.get(&key))
                    {
                        if !cell.owner.shares_backing(owner)
                            || !std::rc::Rc::ptr_eq(&cell.backing, backing)
                        {
                            return Err(malformed());
                        }
                    } else {
                        raw_cells.insert(
                            key,
                            OriginalByteCell {
                                owner: owner.clone(),
                                backing: backing.clone(),
                            },
                        )?;
                    }
                }
                raw_prepared.push((request.argument_index, backings));
                continue;
            }
            if endpoints.len() != backings.len() {
                return Err(malformed());
            }
            for ((endpoint, original), backing) in
                endpoints.into_iter().zip(&request.values).zip(backings)
            {
                prepared.push((
                    endpoint,
                    ScalarCell::resident(original.value, backing).map_err(|error| {
                        error.in_argument_preparation("validate original resident scalar")
                    })?,
                ));
            }
        }
        // Fund any packed-owner detachment and validate existing resident
        // identity before publishing even the first original root or cell.
        for (endpoint, resident) in &prepared {
            let storage = PrimitiveStorage {
                endpoint: endpoint.clone(),
                encoding: None,
            };
            if let Some(cell) = execution.array_cell(&storage)? {
                let (home, index) = match cell {
                    ArrayCell::Frame { home, index } => (home, index),
                    ArrayCell::Structural { array, index } => {
                        execution
                            .structural_byte_storage
                            .get_mut(&array)
                            .ok_or_else(malformed)?
                            .prepare_resident_install(
                                index,
                                resident,
                                execution.account.as_ref(),
                            )?;
                        continue;
                    }
                };
                let (depth, place, _) = home.scalar_array_frame();
                let arrays = frame_field_mut!(execution, depth, scalar_array_values);
                arrays
                    .backing_mut(&place)
                    .ok_or_else(malformed)?
                    .prepare_resident_install(index, resident)
                    .map_err(|error| {
                        error.in_argument_preparation("prepare original frame-array residence")
                    })?;
            }
        }
        let mut original_roots = crate::runtime_rows::RuntimeRows::new(execution.account.as_ref());
        for (owner, request) in owners.into_iter().zip(&requests) {
            let Some(owner) = owner else {
                continue;
            };
            let allocation = OriginalStorageAllocation {
                root: request.root,
                structural_type: request.structural_type,
            };
            if execution
                .original_storage_roots
                .get(&owner)
                .is_some_and(|previous| *previous != allocation)
            {
                return Err(malformed());
            }
            original_roots.insert(owner, allocation)?;
        }
        execution.original_byte_cells.validate_append(&raw_cells)?;
        execution
            .original_storage_roots
            .append(&mut original_roots)?;
        execution.original_byte_cells.append(&mut raw_cells)?;
        // Host preparation has completed without a provider invocation. Every
        // reply now matches the exact original initialized scalar it replaces.
        for (endpoint, resident) in prepared {
            let cell = match endpoint {
                PrimitiveStorageEndpoint::Scalar(home) => {
                    execution.structural_primitive_storage.get_mut(&home)
                }
                PrimitiveStorageEndpoint::ScalarField(field) => {
                    execution.structural_scalar_fields.get_mut(&field)
                }
                _ => {
                    let storage = PrimitiveStorage {
                        endpoint,
                        encoding: None,
                    };
                    let (home, index) =
                        match execution.array_cell(&storage)?.ok_or_else(malformed)? {
                            ArrayCell::Frame { home, index } => (home, index),
                            ArrayCell::Structural { array, index } => {
                                execution
                                    .structural_byte_storage
                                    .get_mut(&array)
                                    .ok_or_else(malformed)?
                                    .install_resident(index, resident)?;
                                continue;
                            }
                        };
                    let (depth, place, _) = home.scalar_array_frame();
                    let arrays = frame_field_mut!(execution, depth, scalar_array_values);
                    arrays
                        .backing_mut(&place)
                        .ok_or_else(malformed)?
                        .install_resident(index, resident)?;
                    continue;
                }
            }
            .ok_or_else(malformed)?;
            *cell = resident;
        }
        for (argument_index, residents) in raw_prepared {
            self.byte_views
                .iter_mut()
                .find(|binding| binding.argument_index == argument_index)
                .ok_or_else(malformed)?
                .residents = residents;
        }
        for request in requests {
            if !request.argument_path.is_empty() {
                continue;
            }
            let loan = request.loan();
            let buffer = self
                .buffers
                .iter_mut()
                .find(|buffer| match buffer {
                    TerminalBoundaryStorageBuffer::Scalar(buffer) => {
                        buffer.argument_index() == request.argument_index
                    }
                    TerminalBoundaryStorageBuffer::Structural(buffer) => {
                        buffer.argument_index() == request.argument_index
                    }
                    TerminalBoundaryStorageBuffer::ByteView(buffer) => {
                        buffer.argument_index() == request.argument_index
                    }
                })
                .ok_or_else(malformed)?;
            match buffer {
                TerminalBoundaryStorageBuffer::Scalar(buffer) => {
                    buffer.original_storage = Some(loan)
                }
                TerminalBoundaryStorageBuffer::Structural(buffer) => {
                    buffer.original_storage = Some(loan)
                }
                TerminalBoundaryStorageBuffer::ByteView(buffer) => {
                    buffer.original_storage = Some(loan)
                }
            }
        }
        for binding in &mut self.scalars.bindings {
            binding.direct_original = true;
        }
        for binding in &mut self.structural {
            binding.direct_original = true;
        }
        Ok(())
    }
}

fn endpoint_parent(
    endpoint: &PrimitiveStorageEndpoint,
) -> Result<&StructuralRuntimePlace, TerminalInterpretError> {
    match endpoint {
        PrimitiveStorageEndpoint::Scalar(home) => Ok(home),
        PrimitiveStorageEndpoint::ScalarField(field) => Ok(&field.parent),
        _ => Err(TerminalInterpretError::UnsupportedSemanticVariant(
            "original foreign storage requires its captured array or descriptor home",
        )),
    }
}

impl TerminalExecution<'_> {
    fn original_byte_array_owner(
        &mut self,
        array: &crate::structural_inputs::byte_arrays::storage::ArrayStorage,
        identity_cursor: &mut Option<u64>,
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        use crate::structural_inputs::byte_arrays::storage::ArrayStorageHome;
        match array.home {
            ArrayStorageHome::Frame {
                depth,
                place,
                structural_type,
            } => {
                let identity = self
                    .array_storage_identity_staged(depth, place, identity_cursor)
                    .map_err(|error| TerminalInterpretError::StructuralArgumentPreparation {
                        stage: "resolve original frame-array identity",
                        detail: format!(
                            "depth {depth}, place {place:?}, type {structural_type:?}, current depth {}: {error:?}",
                            self.call_stack.len()
                        ),
                    })?;
                Ok(TerminalStructuralValue::minted(
                    identity,
                    structural_type,
                    self.frame_array(depth, place)
                        .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                            "original frame-array payload is absent",
                        ))?
                        .qualifications
                        .clone(),
                ))
            }
            ArrayStorageHome::Runtime => {
                let structural_type = self
                    .structural_byte_storage
                    .get(&array.referent)
                    .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                        "original runtime byte-array payload is absent",
                    ))?
                    .structural_type;
                let mut source = TerminalStructuralValue::minted(
                    array.referent.opaque_identity,
                    structural_type,
                    Vec::new(),
                );
                source.path = array.referent.path.clone();
                self.original_storage_owner(&source).map_err(|error| {
                    error.in_argument_preparation("resolve original runtime-array enclosing owner")
                })
            }
        }
    }

    fn original_scalar_owner(
        &mut self,
        storage: &PrimitiveStorage,
        identity_cursor: &mut Option<u64>,
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        if let Some(cell) = self.array_cell(storage)? {
            let home = match cell {
                ArrayCell::Frame { home, .. } => home,
                ArrayCell::Structural { array, .. } => {
                    let structural_type = self
                        .structural_byte_storage
                        .get(&array)
                        .ok_or_else(malformed)?
                        .structural_type;
                    let mut source = TerminalStructuralValue::minted(
                        array.opaque_identity,
                        structural_type,
                        Vec::new(),
                    );
                    source.path = array.path;
                    return self.original_storage_owner(&source);
                }
            };
            let (depth, place, structural_type) = home.scalar_array_frame();
            let identity = self.array_storage_identity_staged(depth, place, identity_cursor)?;
            return Ok(TerminalStructuralValue::minted(
                identity,
                structural_type,
                self.frame_array(depth, place)
                    .ok_or_else(malformed)?
                    .qualifications
                    .clone(),
            ));
        }
        let parent = endpoint_parent(&storage.endpoint)?;
        let mut source = TerminalStructuralValue::minted(
            parent.opaque_identity,
            self.actual_storage_type_from_owner(parent)?,
            Vec::new(),
        );
        source.path = parent.path.clone();
        self.original_storage_owner(&source)
    }

    fn original_storage_owner(
        &self,
        source: &TerminalStructuralValue,
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        // An authorized whole move can install an already realized physical
        // allocation below a new logical container. Its sealed association
        // travels in ConstructorStorage; resident scalar equality alone cannot
        // establish it. Select only a containing physical root, never a child
        // merely convenient for one leaf of a larger requested view.
        let source_home = StructuralRuntimePlace::from(source);
        let selected = self
            .original_storage_roots
            .iter()
            .filter(|(home, _)| {
                home.opaque_identity == source_home.opaque_identity
                    && source_home.path.starts_with(&home.path)
            })
            .min_by_key(|(home, _)| home.path.len());
        if let Some((home, allocation)) = selected {
            let mut original = TerminalStructuralValue::minted(
                home.opaque_identity,
                allocation.structural_type,
                Vec::new(),
            );
            original.path = home.path.clone();
            return Ok(original);
        }
        let original = self
            .original_owned_allocation(source)
            .map_err(|error| error.in_argument_preparation("resolve live original allocation"))?
            .value;
        if !original.path.is_empty() {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "original storage requires retained enclosing geometry after projected transfer",
            ));
        }
        Ok(original)
    }

    fn original_storage_request(
        &self,
        original: &TerminalStructuralValue,
        argument_index: usize,
        access: StructuralAccess,
        referent: Vec<Path>,
        values: Vec<TerminalOriginalScalarValue>,
    ) -> Result<TerminalOriginalStorageRequest, TerminalInterpretError> {
        Ok(TerminalOriginalStorageRequest {
            root: self
                .original_storage_roots
                .get(&StructuralRuntimePlace::from(original))
                .map(|allocation| allocation.root)
                .unwrap_or(TerminalOriginalStorageRoot {
                    artifact: self.program.identity.semantic(),
                    namespace: self.local_structural_identities.exposure_namespace.ok_or(
                        TerminalInterpretError::UnsupportedSemanticVariant(
                            "original storage occurrence namespaces exhausted",
                        ),
                    )?,
                    allocation: original.opaque_identity,
                }),
            structural_type: original.structural_type,
            argument_index,
            referent,
            access,
            values,
            argument_path: Vec::new(),
            view_type: original.structural_type,
            references: Vec::new(),
            scalar_projection: None,
            byte_extent: None,
            byte_window: None,
        })
    }

    fn original_scalar(
        &self,
        original: &TerminalStructuralValue,
        storage: &PrimitiveStorage,
    ) -> Result<TerminalOriginalScalarValue, TerminalInterpretError> {
        if storage
            .encoding
            .is_some_and(|encoding| encoding.view != encoding.source)
        {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "original scalar storage requires its exact representation-view codec",
            ));
        }
        if let Some(cell) = self.array_cell(storage)? {
            let (home, index) = match cell {
                ArrayCell::Frame { home, index } => (home, index),
                ArrayCell::Structural { array, index } => {
                    let payload = self
                        .structural_byte_storage
                        .get(&array)
                        .ok_or_else(malformed)?;
                    let (selected, mut path) = self.actual_storage_projection(original, &array)?;
                    if selected != payload.structural_type {
                        return Err(malformed());
                    }
                    let (value, backing) = payload.capture_scalar(index)?;
                    if storage
                        .encoding
                        .is_some_and(|encoding| encoding.source != value.scalar_type())
                    {
                        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                            "original byte array scalar requires its exact whole-byte codec",
                        ));
                    }
                    path.push(Path::FixedIndex(
                        u64::try_from(index).map_err(|_| malformed())?,
                    ));
                    return Ok(TerminalOriginalScalarValue {
                        path,
                        value,
                        backing,
                    });
                }
            };
            let (depth, place, structural_type) = home.scalar_array_frame();
            let array = self.frame_array(depth, place).ok_or_else(malformed)?;
            if array.storage_identity != Some(original.opaque_identity)
                || structural_type != original.structural_type
                || !original.path.is_empty()
            {
                return Err(malformed());
            }
            let cell = array.cell(index).ok_or_else(malformed)?;
            if storage
                .encoding
                .is_some_and(|encoding| encoding.source != cell.scalar_type())
            {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "original packed scalar requires its exact whole-cell codec",
                ));
            }
            let path = terminal_semantics::scalar_array_leaf_path(
                self.program.structural_types().values(),
                structural_type,
                u64::try_from(index).map_err(|_| malformed())?,
            )
            .ok_or_else(malformed)?
            .into_iter()
            .map(|step| match step {
                terminal_psi::StructuralPathSegment::FixedIndex(index) => {
                    Ok(Path::FixedIndex(index))
                }
                _ => Err(malformed()),
            })
            .collect::<Result<Vec<_>, _>>()?;
            return Ok(TerminalOriginalScalarValue {
                path,
                value: cell.read()?,
                backing: match cell {
                    ScalarCell::Resident { backing, .. } => Some(backing.clone()),
                    _ => None,
                },
            });
        }
        let parent = endpoint_parent(&storage.endpoint)?;
        let (selected, mut path) = self.actual_storage_projection(original, parent)?;
        let cell = match &storage.endpoint {
            PrimitiveStorageEndpoint::Scalar(home) => self.structural_primitive_storage.get(home),
            PrimitiveStorageEndpoint::ScalarField(field) => {
                if field.namespace == StructuralFieldNamespace::CasePayload {
                    let active = self.structural_cases.get(parent).ok_or_else(malformed)?;
                    if active.structural_type != selected {
                        return Err(malformed());
                    }
                    path.push(Path::Case(active.case));
                }
                path.push(Path::Field(field.field));
                self.structural_scalar_fields.get(field)
            }
            _ => return Err(malformed()),
        }
        .ok_or_else(malformed)?;
        Ok(TerminalOriginalScalarValue {
            path,
            value: cell.read()?,
            backing: match cell {
                ScalarCell::Resident { backing, .. } => Some(backing.clone()),
                _ => None,
            },
        })
    }
}

// Follow only the closed semantic descriptor graph already admitted by live
// projection. Stop at recursive types; native memory never discovers a graph.
fn capture_graph(
    execution: &mut TerminalExecution<'_>,
    source: semantic_vocabulary::PlaceId,
    source_path: &[terminal_psi::StructuralPathSegment],
    referent: &TerminalStructuralValue,
    aggregate: Option<&crate::reference::AggregateBacking>,
    captured_scalars: Option<&[super::contents::PinnedScalar]>,
    identity_cursor: &mut Option<u64>,
    argument_index: usize,
    access: StructuralAccess,
    normal_qualifications: &[crate::effects::normal_qualifications::CapturedNormalQualification],
    argument_path: Vec<Path>,
    active: &mut Vec<semantic_vocabulary::StructuralTypeId>,
    captures: &mut Vec<CapturedOriginal>,
) -> Result<TerminalOriginalStorageLoan, TerminalInterpretError> {
    use super::contents::{self, StorageLeafKind};
    use terminal_psi::{StructuralPathSegment, StructuralTypeShape};
    if active.contains(&referent.structural_type) {
        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
            "native original reference graph must have a finite acyclic declaration",
        ));
    }
    active.push(referent.structural_type);
    let backing = aggregate.map_or(referent, |backing| &backing.value);
    let leaves = contents::describe(execution, referent, access, None)?;
    let observed_scalars;
    let scalars = match captured_scalars {
        Some(scalars) => scalars,
        None => {
            observed_scalars = contents::capture(
                execution,
                source,
                referent,
                None,
                aggregate,
                access,
                &leaves,
                normal_qualifications,
            )?
            .1;
            &observed_scalars
        }
    };
    let original = match scalars.first() {
        Some(scalar) => execution.original_scalar_owner(&scalar.storage, identity_cursor)?,
        None => execution.original_storage_owner(backing)?,
    };
    if access == StructuralAccess::MutableBorrow
        && !execution.normal_qualifications_cover(
            &original,
            &original.qualifications,
            &original.projected_qualifications,
            normal_qualifications,
        )?
    {
        return Err(TerminalInterpretError::UnsupportedSemanticVariant(
            "mutable original storage lacks its owner qualification preservation",
        ));
    }
    // Scalar leaves have explicit original-root projections. A containing
    // path is needed only for opaque/reference slots of the typed graph.
    let path = if leaves
        .iter()
        .any(|leaf| matches!(leaf.kind, StorageLeafKind::Opaque { .. }))
    {
        let (selected, path) = execution
            .actual_storage_projection(&original, &StructuralRuntimePlace::from(backing))?;
        if selected != backing.structural_type {
            return Err(malformed());
        }
        path
    } else {
        Vec::new()
    };
    let values = scalars
        .iter()
        .map(|scalar| execution.original_scalar(&original, &scalar.storage))
        .collect::<Result<Vec<_>, _>>()?;
    let mut request =
        execution.original_storage_request(&original, argument_index, access, path, values)?;
    request.argument_path = argument_path.clone();
    request.view_type = referent.structural_type;
    {
        request.scalar_projection = Some(
            scalars
                .iter()
                .zip(&request.values)
                .map(|(scalar, original)| (scalar.path.clone(), original.path.clone()))
                .collect(),
        );
    }
    for leaf in &leaves {
        let StorageLeafKind::Opaque { declaration, .. } = leaf.kind else {
            continue;
        };
        let shape = &execution
            .program
            .structural_types()
            .get(&declaration)
            .ok_or_else(malformed)?
            .shape;
        let StructuralTypeShape::Reference {
            referent: child_type,
            access: child_access,
        } = *shape
        else {
            continue;
        };
        let child_access = if access == StructuralAccess::SharedBorrow {
            StructuralAccess::SharedBorrow
        } else {
            child_access
        };
        if !matches!(
            child_access,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) {
            return Err(malformed());
        }
        let relative = terminal_semantics::canonical_structural_storage_path(
            execution.program.structural_types().values(),
            referent.structural_type,
            &leaf.path,
        )
        .ok_or_else(malformed)?;
        let mut full = source_path.to_vec();
        full.extend_from_slice(&relative);
        full.push(StructuralPathSegment::Referent);
        let child = execution.resolve_live_projection(source, &full, child_access)?;
        if child.structural_type != child_type {
            return Err(malformed());
        }
        let mut child_path = argument_path.clone();
        child_path.extend_from_slice(&leaf.path);
        child_path.push(Path::Referent);
        let target = capture_graph(
            execution,
            source,
            &full,
            &child,
            None,
            None,
            identity_cursor,
            argument_index,
            child_access,
            normal_qualifications,
            child_path,
            active,
            captures,
        )?;
        let mut slot = request.referent.clone();
        slot.extend_from_slice(&leaf.path);
        request.references.push(TerminalOriginalReferenceValue {
            path: slot,
            target,
            referent_type: child_type,
            access: child_access,
        });
    }
    active.pop();
    let loan = request.loan();
    captures.push(CapturedOriginal {
        owner: Some(StructuralRuntimePlace::from(&original)),
        request,
        endpoints: scalars
            .iter()
            .map(|scalar| scalar.storage.endpoint.clone())
            .collect(),
    });
    Ok(loan)
}
