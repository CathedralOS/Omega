//! Projected owned operands move a declared subtree out of an operation
//! result carrier. When the accumulated projections cover every admissible
//! field, the terminal frontier retires the carrier at the consuming call,
//! so the return roster must not schedule a whole-root discard for it.
//!
//! The coverage math mirrors the verifier's `partial_affine_root_type`,
//! `is_partial_affine_path` and `partial_affine_residuals` over the emitted
//! terminal declarations.

use std::collections::{BTreeMap, BTreeSet};

use crate::unit::{
    ByteSequenceCarrier, Operation, OperationKind, PlaceId, StructuralAccess, StructuralFieldType,
    StructuralMultiplicity, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape,
};
use terminal_psi::StructuralPathSegment;

/// Every subtree an owned argument projects out of an operation-result
/// place, keyed by that place. The terminal frontier opens partial custody
/// for each of these paths when the consuming operation applies.
pub(super) fn moved_subtrees(
    operations: &[Operation],
) -> BTreeMap<PlaceId, BTreeSet<Vec<StructuralPathSegment>>> {
    let mut moved: BTreeMap<PlaceId, BTreeSet<Vec<StructuralPathSegment>>> = BTreeMap::new();
    for operation in operations {
        let arguments = match &operation.kind {
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
            } => structural_arguments.as_slice(),
            _ => &[],
        };
        for argument in arguments {
            if argument.access == StructuralAccess::Owned && !argument.path.is_empty() {
                moved
                    .entry(argument.place)
                    .or_default()
                    .insert(argument.path.clone());
            }
        }
    }
    moved
}

/// Whether `moved` covers every admissible field of `place`'s declared
/// result type, letting the frontier retire the carrier at the consuming
/// call. Mirrors `partial_affine_root_type` followed by an empty
/// `partial_affine_residuals` row.
pub(super) fn retires(
    types: &[StructuralTypeDeclaration],
    operations: &[Operation],
    place: &StructuralPlaceDeclaration,
    moved: &BTreeSet<Vec<StructuralPathSegment>>,
) -> bool {
    if moved.is_empty() {
        return false;
    }
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = place.kind
    else {
        return false;
    };
    let Some(operation) = operations.iter().find(|candidate| candidate.id == producer) else {
        return false;
    };
    if !matches!(
        operation.kind,
        OperationKind::CallStructuralWithScalarArguments { .. }
            | OperationKind::BoundaryCall { .. }
            | OperationKind::EstablishRecord { .. }
    ) {
        return false;
    }
    let Some(result) = operation.result.structural() else {
        return false;
    };
    if result.place != place.id
        || result.structural_type != structural_type
        || result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return false;
    }
    // `is_partial_affine_path`: every moved path must resolve inside the
    // declared result type, and no path may be a prefix of another.
    if moved
        .iter()
        .any(|path| path.is_empty() || resolve(types, structural_type, path).is_none())
        || moved.iter().enumerate().any(|(index, path)| {
            moved
                .iter()
                .enumerate()
                .any(|(other, candidate)| index != other && path.starts_with(candidate))
        })
    {
        return false;
    }
    let moved_paths: Vec<&[StructuralPathSegment]> = moved.iter().map(Vec::as_slice).collect();
    visit(types, structural_type, &moved_paths).is_some()
}

/// `resolve_structural_path` over the emitted declaration roster: fields
/// match their declared identity, fixed indices stay inside the extent.
fn resolve(
    types: &[StructuralTypeDeclaration],
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in path {
        let declaration = types
            .iter()
            .find(|declaration| declaration.id == structural_type)?;
        structural_type = match (segment, &declaration.shape) {
            (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields }) => {
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
                match &field.field_type {
                    StructuralFieldType::Structural(next) => *next,
                    leaf => {
                        let shape = leaf.canonical_leaf_shape()?;
                        types
                            .iter()
                            .find(|declaration| declaration.shape == shape)
                            .map(|declaration| declaration.id)?
                    }
                }
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            _ => return None,
        };
    }
    Some(structural_type)
}

/// `partial_affine_residuals` with a zero residual budget: `Some` only when
/// `moved` covers every admissible child, leaving nothing for the carrier.
fn visit(
    types: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    moved: &[&[StructuralPathSegment]],
) -> Option<()> {
    let declaration = types
        .iter()
        .find(|declaration| declaration.id == structural_type)?;
    match &declaration.shape {
        StructuralTypeShape::Record { fields } => {
            if fields.is_empty()
                || fields
                    .iter()
                    .any(|field| field.relevance.is_erased() || !admissible(&field.field_type))
            {
                return None;
            }
            for field in fields.iter().rev() {
                if let StructuralFieldType::Structural(child_type) = field.field_type {
                    child(
                        types,
                        child_type,
                        StructuralPathSegment::Field(field.identity.clone()),
                        moved,
                    )?;
                }
            }
        }
        StructuralTypeShape::FixedArray { element, length } if *length != 0 => {
            if u128::from(*length) > moved.len() as u128 {
                return None;
            }
            for index in (0..*length).rev() {
                child(
                    types,
                    *element,
                    StructuralPathSegment::FixedIndex(index),
                    moved,
                )?;
            }
        }
        _ => return None,
    }
    Some(())
}

fn child(
    types: &[StructuralTypeDeclaration],
    child_type: StructuralTypeId,
    segment: StructuralPathSegment,
    moved: &[&[StructuralPathSegment]],
) -> Option<()> {
    let descendants: Vec<&[StructuralPathSegment]> = moved
        .iter()
        .filter_map(|path| {
            let (head, tail) = path.split_first()?;
            (head == &segment).then_some(tail)
        })
        .collect();
    if descendants.is_empty() {
        // An untouched child leaves a residual the carrier must still own.
        None
    } else if descendants.iter().all(|path| !path.is_empty()) {
        visit(types, child_type, descendants.as_slice())
    } else {
        // A single empty descendant ends the moved path exactly at this
        // child; any sibling path would already have failed the antichain.
        (descendants.len() == 1).then_some(())
    }
}

/// `partial_affine_residuals`'s field admissibility: only plain relevant
/// scalar or bounded-owned byte-sequence leaves may sit beside moved
/// subtrees; every structural child must be covered.
fn admissible(field_type: &StructuralFieldType) -> bool {
    matches!(
        field_type,
        StructuralFieldType::Structural(_)
            | StructuralFieldType::Scalar(_)
            | StructuralFieldType::IeeeFloat(_)
            | StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
    )
}
