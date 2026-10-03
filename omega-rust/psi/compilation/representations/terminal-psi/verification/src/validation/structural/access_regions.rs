//! Canonical storage regions observed or mutated by structural operations.
//!
//! An unresolvable projection denotes the whole spelled root. This is the
//! conservative result: consumers may use a precise region to prove sibling
//! disjointness, but malformed or dynamic geometry cannot manufacture it.

use super::operations::{canonical_field_path, canonical_mutable_record_referent_path};
use crate::validation::{
    CanonicalStructuralPathSegment, OperationKind, PlaceId, StructuralPathSegment, TerminalMachine,
    TerminalModule,
};
use terminal_psi::StructuralArgument;

/// One argument's resolved canonical prefix, or its whole root when the
/// spelled path cannot be proven. A trailing `Referent` names the reference
/// target and is therefore part of crossing the carrier, not the target's
/// canonical field region.
pub(in crate::validation) fn argument_region(
    module: &TerminalModule,
    machine: &TerminalMachine,
    argument: &StructuralArgument,
) -> (PlaceId, Vec<CanonicalStructuralPathSegment>) {
    let mut path = argument.path.clone();
    if path.last() == Some(&StructuralPathSegment::Referent) {
        path.pop();
    }
    let region = canonical_mutable_record_referent_path(module, machine, argument.place, &path)
        .or_else(|| canonical_field_path(module, machine, argument.place, &path))
        .unwrap_or_default();
    (argument.place, region)
}

/// The (spelled place, canonical region) pairs an operation reads, writes,
/// moves, or loans. Structural field move/store hole regions are deliberately
/// excluded: restoration-window validation owns their ordered extraction and
/// repair effects rather than treating either as an ordinary observation.
pub(in crate::validation) fn operation_regions(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
) -> Vec<(PlaceId, Vec<CanonicalStructuralPathSegment>)> {
    let canonical_region = |place: PlaceId, path: &[StructuralPathSegment]| {
        canonical_mutable_record_referent_path(module, machine, place, path)
            .or_else(|| canonical_field_path(module, machine, place, path))
    };
    let field_region = |place: &PlaceId, path: &[StructuralPathSegment], field| {
        let region = canonical_region(*place, path).map_or_else(Vec::new, |mut region| {
            region.push(CanonicalStructuralPathSegment::Field(field));
            region
        });
        (*place, region)
    };
    // A primitive leaf's region is its canonical path; a runtime-selected
    // element touches the whole array it selects in.
    let primitive_region = |place: &PlaceId, path: &[StructuralPathSegment]| {
        let static_prefix = path
            .iter()
            .position(|segment| segment.runtime_index().is_some())
            .unwrap_or(path.len());
        (
            *place,
            canonical_region(*place, &path[..static_prefix]).unwrap_or_default(),
        )
    };
    match &operation.kind {
        OperationKind::EstablishByteSequenceConcatenation { left, right, .. } => [left, right]
            .into_iter()
            .map(|operand| match operand {
                terminal_psi::control_flow::ByteSequenceOperand::Whole { source, .. } => {
                    (*source, Vec::new())
                }
                terminal_psi::control_flow::ByteSequenceOperand::Field {
                    source, field, ..
                } => field_region(&source.place, &source.path, *field),
            })
            .collect(),
        OperationKind::EstablishReference { source } => {
            vec![argument_region(module, machine, source)]
        }
        OperationKind::ReleaseReference { source } => vec![(*source, Vec::new())],
        OperationKind::PrimitiveScalarRead { source, path } => {
            vec![primitive_region(source, path)]
        }
        OperationKind::StructuralByteSequenceFieldLength {
            source,
            path,
            field,
            ..
        }
        | OperationKind::StructuralByteSequenceFieldRead {
            source,
            path,
            field,
            ..
        } => vec![field_region(source, path, *field)],
        OperationKind::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            ..
        } => vec![field_region(destination, path, *field)],
        // A runtime element in the carrier touches every element of the
        // array it selects in, as a primitive leaf's does.
        OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            ..
        } => vec![if terminal_psi::is_static_structural_path(path) {
            field_region(destination, path, *field)
        } else {
            primitive_region(destination, path)
        }],
        OperationKind::ByteSequenceStore {
            destination,
            source,
            ..
        } => vec![(*destination, Vec::new()), (*source, Vec::new())],
        OperationKind::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            ..
        }
        | OperationKind::StructuralByteViewFieldStore {
            destination,
            path,
            field,
            source,
            ..
        } => vec![
            field_region(destination, path, *field),
            (*source, Vec::new()),
        ],
        OperationKind::WriteOnlyPrimitiveStore {
            destination, path, ..
        } => vec![primitive_region(destination, path)],
        OperationKind::AtomicAccess {
            place, path, field, ..
        } => vec![field_region(place, path, *field)],
        OperationKind::StructuralCaseMembership { source, path, .. } => {
            vec![(*source, canonical_region(*source, path).unwrap_or_default())]
        }
        // The carrier path is already canonical and excludes the read field,
        // so the observed region is that one scalar leaf. Reads beside an open
        // element window touch no absent storage; reads beneath it overlap.
        OperationKind::BooleanStructuralField {
            source,
            path,
            field,
        }
        | OperationKind::IntegerStructuralField {
            source,
            path,
            field,
        } => {
            let mut region = path.clone();
            region.push(CanonicalStructuralPathSegment::Field(*field));
            vec![(*source, region)]
        }
        OperationKind::ByteSequenceSubslice { source, .. }
        | OperationKind::ByteSequenceLength { source }
        | OperationKind::ByteSequenceRead { source, .. } => vec![(*source, Vec::new())],
        OperationKind::ByteSequenceWrite { destination, .. }
        | OperationKind::ElementViewWrite { destination, .. } => {
            vec![(*destination, Vec::new())]
        }
        OperationKind::EstablishRecord { fields }
        | OperationKind::EstablishStructuralCase { fields, .. } => fields
            .iter()
            .filter_map(|field| match &field.value {
                terminal_psi::RecordFieldValue::Structural(argument) => {
                    Some(argument_region(module, machine, argument))
                }
                terminal_psi::RecordFieldValue::Scalar { .. } => None,
            })
            .collect(),
        OperationKind::CallUnit {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructural {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            structural_arguments,
            ..
        }
        | OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments
            .iter()
            .map(|argument| argument_region(module, machine, argument))
            .collect(),
        _ => Vec::new(),
    }
}
