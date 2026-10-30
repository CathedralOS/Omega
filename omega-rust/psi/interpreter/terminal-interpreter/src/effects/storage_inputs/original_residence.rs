//! Realize only authenticated exposed cells in their original owned allocation.
//! All host replies validate before local payloads are replaced by residents.
//! The ordinary staged restoration path remains with its existing owner.

use super::{BoundaryStorageInputs, TerminalBoundaryStorageBuffer};
use crate::{
    effects::{TerminalEffectHandler, original_storage::*},
    errors::{TerminalInterpretError, malformed},
    execution::TerminalExecution,
    primitive_storage::{PrimitiveStorage, PrimitiveStorageEndpoint},
    scalar_storage::ScalarCell,
    values::{StructuralFieldNamespace, StructuralRuntimePlace, TerminalStructuralValue},
};
use semantic_vocabulary::{BoundaryMachineId, CanonicalStructuralPathSegment as Path};
use terminal_psi::StructuralAccess;

struct CapturedOriginal {
    owner: StructuralRuntimePlace,
    request: TerminalOriginalStorageRequest,
    endpoints: Vec<PrimitiveStorageEndpoint>,
}

enum OriginalArrayCell {
    Frame {
        home: crate::primitive_storage::PrimitiveStorageHome,
        index: usize,
    },
    Structural {
        array: StructuralRuntimePlace,
        index: usize,
    },
}

impl BoundaryStorageInputs {
    pub(crate) fn prepare_original_storage(
        &mut self,
        execution: &mut TerminalExecution<'_>,
        handler: &mut impl TerminalEffectHandler,
        boundary: BoundaryMachineId,
        identity_cursor: &mut Option<u64>,
    ) -> Result<(), TerminalInterpretError> {
        if !handler.requires_original_storage(boundary) {
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
                owner: StructuralRuntimePlace::from(&original),
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
        for ((request, endpoints), backings) in requests.iter().zip(endpoints).zip(returned) {
            if backings.len() != request.values.len() || endpoints.len() != backings.len() {
                return Err(malformed());
            }
            for ((endpoint, original), backing) in
                endpoints.into_iter().zip(&request.values).zip(backings)
            {
                prepared.push((endpoint, ScalarCell::resident(original.value, backing)?));
            }
        }
        // Fund any packed-owner detachment and validate existing resident
        // identity before publishing even the first original root or cell.
        for (endpoint, resident) in &prepared {
            let storage = PrimitiveStorage {
                endpoint: endpoint.clone(),
                encoding: None,
            };
            if let Some(cell) = execution.original_array_cell(&storage)? {
                let (home, index) = match cell {
                    OriginalArrayCell::Frame { home, index } => (home, index),
                    OriginalArrayCell::Structural { array, index } => {
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
                let arrays = if depth == execution.call_stack.len() {
                    &mut execution.scalar_array_values
                } else {
                    &mut execution
                        .call_stack
                        .get_mut(depth)
                        .ok_or_else(malformed)?
                        .scalar_array_values
                };
                arrays
                    .backing_mut(&place)
                    .ok_or_else(malformed)?
                    .prepare_resident_install(index, resident)?;
            }
        }
        let mut original_roots = crate::runtime_rows::RuntimeRows::new(execution.account.as_ref());
        for (owner, request) in owners.into_iter().zip(&requests) {
            if execution
                .original_storage_roots
                .get(&owner)
                .is_some_and(|previous| *previous != request.root)
            {
                return Err(malformed());
            }
            original_roots.insert(owner, request.root)?;
        }
        execution
            .original_storage_roots
            .append(&mut original_roots)?;
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
                    let (home, index) = match execution
                        .original_array_cell(&storage)?
                        .ok_or_else(malformed)?
                    {
                        OriginalArrayCell::Frame { home, index } => (home, index),
                        OriginalArrayCell::Structural { array, index } => {
                            execution
                                .structural_byte_storage
                                .get_mut(&array)
                                .ok_or_else(malformed)?
                                .install_resident(index, resident)?;
                            continue;
                        }
                    };
                    let (depth, place, _) = home.scalar_array_frame();
                    let arrays = if depth == execution.call_stack.len() {
                        &mut execution.scalar_array_values
                    } else {
                        &mut execution
                            .call_stack
                            .get_mut(depth)
                            .ok_or_else(malformed)?
                            .scalar_array_values
                    };
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
                })
                .ok_or_else(malformed)?;
            match buffer {
                TerminalBoundaryStorageBuffer::Scalar(buffer) => {
                    buffer.original_storage = Some(loan)
                }
                TerminalBoundaryStorageBuffer::Structural(buffer) => {
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
    /// Resolve packed endpoints through their captured frame, including a byte
    /// window within a nested array. The descriptor supplies no new allocation.
    fn original_array_cell(
        &self,
        storage: &PrimitiveStorage,
    ) -> Result<Option<OriginalArrayCell>, TerminalInterpretError> {
        use crate::structural_inputs::byte_arrays::storage::ArrayStorageHome;
        let (home, index) = match &storage.endpoint {
            PrimitiveStorageEndpoint::OwnedArrayElement { home, index } => (*home, *index),
            PrimitiveStorageEndpoint::ArrayElement { array, index } => {
                let (depth, place, structural_type) = match array.home {
                    ArrayStorageHome::Frame {
                        depth,
                        place,
                        structural_type,
                    } => (depth, place, structural_type),
                    ArrayStorageHome::Runtime => {
                        let payload = self
                            .structural_byte_storage
                            .get(&array.referent)
                            .ok_or_else(malformed)?;
                        if *index >= payload.len() {
                            return Err(malformed());
                        }
                        return Ok(Some(OriginalArrayCell::Structural {
                            array: array.referent.clone(),
                            index: *index,
                        }));
                    }
                };
                let (start, end) = self.frame_byte_array_window(array)?;
                let index = start
                    .checked_add(*index)
                    .filter(|index| *index < end)
                    .ok_or_else(malformed)?;
                let machine = if depth == self.call_stack.len() {
                    self.current_machine
                } else {
                    self.call_stack
                        .get(depth)
                        .ok_or_else(malformed)?
                        .current_machine
                };
                (
                    crate::primitive_storage::PrimitiveStorageHome::captured_frame(
                        depth,
                        machine,
                        place,
                        structural_type,
                    ),
                    index,
                )
            }
            _ => return Ok(None),
        };
        self.owned_array_home(&home)?;
        let (depth, place, structural_type) = home.scalar_array_frame();
        let array = self.frame_array(depth, place).ok_or_else(malformed)?;
        if array.structural_type != structural_type || index >= array.len() {
            return Err(malformed());
        }
        Ok(Some(OriginalArrayCell::Frame { home, index }))
    }

    fn original_scalar_owner(
        &mut self,
        storage: &PrimitiveStorage,
        identity_cursor: &mut Option<u64>,
    ) -> Result<TerminalStructuralValue, TerminalInterpretError> {
        if let Some(cell) = self.original_array_cell(storage)? {
            let home = match cell {
                OriginalArrayCell::Frame { home, .. } => home,
                OriginalArrayCell::Structural { array, .. } => {
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
            .keys()
            .filter(|home| {
                home.opaque_identity == source_home.opaque_identity
                    && source_home.path.starts_with(&home.path)
            })
            .min_by_key(|home| home.path.len());
        if let Some(home) = selected {
            let mut original = TerminalStructuralValue::minted(
                home.opaque_identity,
                self.actual_storage_type_from_owner(home)?,
                Vec::new(),
            );
            original.path = home.path.clone();
            return Ok(original);
        }
        let original = self.original_owned_allocation(source)?.value;
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
                .copied()
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
        if let Some(cell) = self.original_array_cell(storage)? {
            let (home, index) = match cell {
                OriginalArrayCell::Frame { home, index } => (home, index),
                OriginalArrayCell::Structural { array, index } => {
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
            let mut carrier = structural_type;
            let mut ordinal = u64::try_from(index).map_err(|_| malformed())?;
            let types = self.program.structural_types();
            let mut path = Vec::new();
            while let terminal_psi::StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            } = types.get(&carrier).ok_or_else(malformed)?.shape
            {
                let count = if matches!(
                    types.get(&element).ok_or_else(malformed)?.shape,
                    terminal_psi::StructuralTypeShape::PrimitiveScalar(_)
                ) {
                    1
                } else {
                    terminal_semantics::scalar_array_leaf_shape(types.values(), element)
                        .ok_or_else(malformed)?
                        .1
                };
                let selected = ordinal
                    .checked_div(count)
                    .filter(|selected| *selected < length)
                    .ok_or_else(malformed)?;
                ordinal %= count;
                path.push(Path::FixedIndex(selected));
                carrier = element;
            }
            if ordinal != 0 {
                return Err(malformed());
            }
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
        owner: StructuralRuntimePlace::from(&original),
        request,
        endpoints: scalars
            .iter()
            .map(|scalar| scalar.storage.endpoint.clone())
            .collect(),
    });
    Ok(loan)
}
