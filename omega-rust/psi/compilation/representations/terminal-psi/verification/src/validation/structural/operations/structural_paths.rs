//! Canonical structural argument prefixes and field store write paths.

use crate::validation::structural::operations::crash_continuations::caller_structural_root_type;
use crate::validation::{
    CanonicalStructuralPathSegment, OperationKind, PlaceId, StructuralAccess, StructuralArgument,
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
                    StructuralFieldType::Scalar(_) | StructuralFieldType::IeeeFloat(_)
                        if position + 1 == argument.path.len() =>
                    {
                        return Some(prefix);
                    }
                    StructuralFieldType::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                    )
                    | StructuralFieldType::PredicateByteSequence {
                        carrier: terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
                        ..
                    } if argument.access == StructuralAccess::MutableBorrow
                        && position + 1 == argument.path.len() =>
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
    // A root window writes the whole referent: its hole is the empty
    // canonical path, which every observation of the root overlaps.
    let Some(field) = field else {
        return (window && path.is_empty()).then_some((root, Vec::new()));
    };
    let (root_type, path) = if window {
        crate::validation::references::mutable_record_field_destination(module, machine, root, path)
            .unwrap_or((caller_structural_root_type(machine, root)?, path.as_slice()))
    } else {
        (caller_structural_root_type(machine, root)?, path.as_slice())
    };
    // A window operation whose spelled path crosses a runtime element names
    // no canonical place beneath it: the hole is the statically known prefix
    // — the whole array subtree the runtime step selects in — which the
    // region rule treats as overlapping every element. The other store kinds
    // keep canonical_field_path's own refusal of runtime segments.
    let first_runtime = path
        .iter()
        .position(|segment| segment.runtime_index().is_some());
    let statically = match (window, first_runtime) {
        (true, Some(position)) => &path[..position],
        _ => path,
    };
    let mut written = canonical_field_path_from_type(module, root_type, statically)?;
    match path.last() {
        // An element leaf: the path ends in an element selection and `field`
        // names the last field segment's own declaration — the array the
        // element rides in. The hole is the spelled element, or the whole
        // array prefix when a runtime step selected it.
        Some(StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. })
            if window =>
        {
            let field_position = path
                .iter()
                .rposition(|segment| matches!(segment, StructuralPathSegment::Field(_)))?;
            let owner = crate::validation::foundation::resolve_runtime_projection(
                module,
                root_type,
                &path[..field_position],
            )?;
            let declaration = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == owner)?;
            let fields = match &declaration.shape {
                StructuralTypeShape::Record { fields }
                | StructuralTypeShape::Mixed { fields, .. } => fields,
                _ => return None,
            };
            fields
                .iter()
                .any(|candidate| candidate.id == field && !candidate.relevance.is_erased())
                .then_some((root, written))
        }
        _ => {
            let structural_type = if window {
                crate::validation::foundation::resolve_runtime_projection(module, root_type, path)?
            } else {
                crate::validation::foundation::resolve_structural_path(module, root_type, path)?
            };
            let carrier = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == structural_type)?;
            match &carrier.shape {
                StructuralTypeShape::Record { fields }
                | StructuralTypeShape::Mixed { fields, .. }
                    if fields.iter().any(|candidate| {
                        candidate.id == field && !candidate.relevance.is_erased()
                    }) =>
                {
                    // Beneath a runtime crossing the leaf field is only a type
                    // check — the hole already stops at the array prefix.
                    if first_runtime.is_none() {
                        written.push(CanonicalStructuralPathSegment::Field(field));
                    }
                    Some((root, written))
                }
                _ => None,
            }
        }
    }
}
