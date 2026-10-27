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

impl BoundaryStorageInputs {
    pub(crate) fn prepare_original_storage(
        &mut self,
        execution: &mut TerminalExecution<'_>,
        handler: &mut impl TerminalEffectHandler,
        boundary: BoundaryMachineId,
    ) -> Result<(), TerminalInterpretError> {
        if !handler.requires_original_storage(boundary) {
            return Ok(());
        }
        let mut captures = Vec::new();
        for binding in &self.scalars.bindings {
            let parent = endpoint_parent(&binding.storage.endpoint)?;
            let mut source = TerminalStructuralValue::minted(
                parent.opaque_identity,
                execution.actual_storage_type_from_owner(parent)?,
                Vec::new(),
            );
            source.path = parent.path.clone();
            let original = execution.original_storage_owner(&source)?;
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
                binding.argument_index,
                binding.access,
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
                _ => return Err(malformed()),
            }
            .ok_or_else(malformed)?;
            *cell = resident;
        }
        for request in requests {
            if !request.argument_path.is_empty() {
                continue;
            }
            let loan = TerminalOriginalStorageLoan {
                root: request.root,
                referent: request.referent,
            };
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
    execution: &TerminalExecution<'_>,
    source: semantic_vocabulary::PlaceId,
    source_path: &[terminal_psi::StructuralPathSegment],
    referent: &TerminalStructuralValue,
    argument_index: usize,
    access: StructuralAccess,
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
    let original = execution.original_storage_owner(referent)?;
    let (selected, path) =
        execution.actual_storage_projection(&original, &StructuralRuntimePlace::from(referent))?;
    if selected != referent.structural_type {
        return Err(malformed());
    }
    let leaves = contents::describe(execution, referent, access, None)?;
    let (_, scalars) = contents::capture(execution, source, referent, None, access, &leaves, &[])?;
    let values = scalars
        .iter()
        .map(|scalar| execution.original_scalar(&original, &scalar.storage))
        .collect::<Result<Vec<_>, _>>()?;
    let mut request =
        execution.original_storage_request(&original, argument_index, access, path, values)?;
    request.argument_path = argument_path.clone();
    request.view_type = referent.structural_type;
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
            argument_index,
            child_access,
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
            .into_iter()
            .map(|scalar| scalar.storage.endpoint)
            .collect(),
    });
    Ok(loan)
}
