//! Canonical structural argument prefixes and field store write paths.

use crate::validation::structural_operations::crash_continuations::caller_structural_root_type;
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
    let mut prefix = Vec::with_capacity(argument.path.len());
    for (position, segment) in argument.path.iter().enumerate() {
        match segment {
            StructuralPathSegment::Referent => return None,
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
                        | StructuralTypeShape::Reference { .. }
                        | StructuralTypeShape::ByteSequence(_)
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
                    ) if argument.access == StructuralAccess::MutableBorrow
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
    let (root, path, field) = match &operation.kind {
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
        | OperationKind::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            ..
        } => (*destination, path, *field),
        _ => return None,
    };
    let mut structural_type = caller_structural_root_type(machine, root)?;
    let mut written = Vec::with_capacity(path.len() + 1);
    for segment in path {
        match segment {
            StructuralPathSegment::Referent => return None,
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
    let carrier = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)?;
    match &carrier.shape {
        StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. }
            if fields
                .iter()
                .any(|candidate| candidate.id == field && !candidate.relevance.is_erased()) =>
        {
            written.push(CanonicalStructuralPathSegment::Field(field));
            Some((root, written))
        }
        _ => None,
    }
}
