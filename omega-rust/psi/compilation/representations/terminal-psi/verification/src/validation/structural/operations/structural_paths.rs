//! Canonical structural argument prefixes and field store write paths.

use crate::validation::structural::operations::crash_continuations::caller_structural_root_type;
use crate::validation::{
    CanonicalStructuralPathSegment, OperationKind, PlaceId, StructuralArgument,
    StructuralFieldType, StructuralPathSegment, StructuralTypeShape, TerminalMachine,
    TerminalModule,
};

pub(crate) fn structural_argument_canonical_prefix(
    module: &TerminalModule,
    caller: &TerminalMachine,
    argument: &StructuralArgument,
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    let mut structural_type = caller_structural_root_type(caller, argument.place)?;
    if let Some((path, _)) = terminal_semantics::canonical_static_projection(
        module.structural_types.iter(),
        structural_type,
        &argument.path,
    ) {
        return Some(path);
    }
    let mut prefix = Vec::with_capacity(argument.path.len());
    for (position, segment) in argument.path.iter().enumerate() {
        match segment {
            // A runtime index has no canonical owned-field spelling: callers
            // either omit an unreferenced binder or reject, never substitute
            // the whole backing for a dynamically selected element.
            StructuralPathSegment::Case(_)
            | StructuralPathSegment::Referent
            | StructuralPathSegment::FixedByteRange { .. }
            | StructuralPathSegment::RuntimeIndex { .. } => {
                return None;
            }
            StructuralPathSegment::Field(identity) => {
                let field = module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == structural_type)
                    .and_then(|declaration| match &declaration.shape {
                        StructuralTypeShape::Record { fields }
                        | StructuralTypeShape::Mixed { fields, .. } => {
                            fields.iter().find(|field| {
                                field.identity == *identity && !field.relevance.is_erased()
                            })
                        }
                        StructuralTypeShape::PrimitiveScalar(_)
                        | StructuralTypeShape::FusedServiceBinding
                        | StructuralTypeShape::Reference { .. }
                        | StructuralTypeShape::ByteSequence(_)
                        | StructuralTypeShape::ElementView { .. }
                        | StructuralTypeShape::FixedArray { .. }
                        | StructuralTypeShape::Sum { .. } => None,
                    })?;
                prefix.push(CanonicalStructuralPathSegment::Field(field.id));
                match field.field_type {
                    StructuralFieldType::Structural(next) => structural_type = next,
                    // This query names the exact actual endpoint after call
                    // admission has checked its type and access. A byte-field
                    // descriptor denotes that field's live view; unlike a
                    // window, its canonical field path loses no information.
                    StructuralFieldType::Scalar(_)
                    | StructuralFieldType::IeeeFloat(_)
                    | StructuralFieldType::ByteSequence(_)
                    | StructuralFieldType::PredicateByteSequence { .. }
                        if position + 1 == argument.path.len() =>
                    {
                        return Some(prefix);
                    }
                    _ => return None,
                }
            }
            StructuralPathSegment::FixedIndex(index) => {
                let element = module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == structural_type)
                    .and_then(|declaration| match declaration.shape {
                        StructuralTypeShape::FixedArray { element, length } if *index < length => {
                            Some(element)
                        }
                        _ => None,
                    })?;
                prefix.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                structural_type = element;
            }
        }
    }
    Some(prefix)
}

/// Resolve one name-spelled structural path beneath `root` into canonical
/// field-id/fixed-index segments. `None` means the path cannot be scoped
/// against the declared carriers — callers either forget the whole root or
/// reject through their own validation.
pub(crate) fn canonical_field_path(
    module: &TerminalModule,
    machine: &TerminalMachine,
    root: PlaceId,
    path: &[StructuralPathSegment],
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    canonical_field_path_from_type(module, caller_structural_root_type(machine, root)?, path)
}

/// Resolve a structural path once its exact semantic root type has already
/// been reconstructed. Reference carriers use this after the explicit
/// `Referent` crossing; the returned path stays relative to that carrier's
/// captured address rather than being rewritten to a possible origin.
pub(crate) fn canonical_field_path_from_type(
    module: &TerminalModule,
    mut structural_type: semantic_vocabulary::StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    if let Some((path, _)) = terminal_semantics::canonical_static_projection(
        module.structural_types.iter(),
        structural_type,
        path,
    ) {
        return Some(path);
    }
    let mut written = Vec::with_capacity(path.len());
    for segment in path {
        match segment {
            StructuralPathSegment::Case(_)
            | StructuralPathSegment::Referent
            | StructuralPathSegment::FixedByteRange { .. }
            | StructuralPathSegment::RuntimeIndex { .. } => {
                return None;
            }
            StructuralPathSegment::Field(identity) => {
                let declaration = module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == structural_type)?;
                let field = match &declaration.shape {
                    StructuralTypeShape::Record { fields }
                    | StructuralTypeShape::Mixed { fields, .. } => fields
                        .iter()
                        .find(|field| field.identity == *identity && !field.relevance.is_erased()),
                    _ => None,
                }?;
                written.push(CanonicalStructuralPathSegment::Field(field.id));
                structural_type = match field.field_type {
                    StructuralFieldType::Structural(next) => next,
                    _ => return None,
                };
            }
            StructuralPathSegment::FixedIndex(index) => {
                let element = module
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == structural_type)
                    .and_then(|declaration| match declaration.shape {
                        StructuralTypeShape::FixedArray { element, length } if *index < length => {
                            Some(element)
                        }
                        _ => None,
                    })?;
                written.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                structural_type = element;
            }
        }
    }
    Some(written)
}

/// Canonical path beneath the referent of an exact mutable-record carrier.
/// This is deliberately separate from `canonical_field_path`: only consumers
/// that already model a borrow crossing may interpret `Referent`, and their
/// logical root remains the carrier rather than a possible captured origin.
pub(crate) fn canonical_mutable_record_referent_path(
    module: &TerminalModule,
    machine: &TerminalMachine,
    carrier: PlaceId,
    path: &[StructuralPathSegment],
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    let (referent, suffix) = crate::validation::references::mutable_record_field_destination(
        module, machine, carrier, path,
    )?;
    canonical_field_path_from_type(module, referent, suffix)
}

/// Resolve the exact canonical path one field store writes. `path` arrives in
/// the operation's name-spelled segment space; each field identity is resolved
/// against the declared carrier so the result can be compared against exact
/// canonical observation paths. `None` means the write cannot be scoped below
/// the root and callers must forget the whole root instead.
pub(crate) fn structural_field_store_write_path(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Option<(PlaceId, Vec<CanonicalStructuralPathSegment>)> {
    let (root, path, field, window) = match &operation.kind {
        OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            ..
        }
        | OperationKind::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            ..
        }
        | OperationKind::StructuralByteViewFieldStore {
            destination,
            path,
            field,
            ..
        }
        | OperationKind::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            ..
        } => (*destination, path, Some(*field), false),
        OperationKind::StoreStructuralField {
            destination,
            path,
            field,
            ..
        } => (*destination, path, *field, true),
        OperationKind::MoveStructuralField {
            source,
            path,
            field,
        } => (*source, path, *field, true),
        _ => return None,
    };
    let root_type = caller_structural_root_type(machine, root)?;
    if window {
        let (geometry_type, geometry_path) =
            crate::validation::references::mutable_record_field_destination(
                module, machine, root, path,
            )
            .unwrap_or((root_type, path.as_slice()));
        let projection = terminal_semantics::structural_window_projection(
            module.structural_types.iter(),
            geometry_type,
            geometry_path,
            field,
        )?;
        // Runtime selection bounds interference by the array prefix; it does
        // not authorize a more precise hole without a proved selector.
        let length = projection
            .path
            .iter()
            .position(|segment| segment.runtime_index().is_some())
            .unwrap_or(projection.path.len());
        let written =
            canonical_field_path_from_type(module, geometry_type, &projection.path[..length])?;
        return Some((root, written));
    }
    let field = field?;
    let mut written = canonical_field_path_from_type(module, root_type, path)?;
    let structural_type =
        crate::validation::foundation::resolve_structural_path(module, root_type, path)?;
    let carrier = module
        .structural_types
        .iter()
        .find(|row| row.id == structural_type)?;
    let (StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. }) =
        &carrier.shape
    else {
        return None;
    };
    fields
        .iter()
        .any(|candidate| candidate.id == field && !candidate.relevance.is_erased())
        .then_some(())?;
    written.push(CanonicalStructuralPathSegment::Field(field));
    Some((root, written))
}
