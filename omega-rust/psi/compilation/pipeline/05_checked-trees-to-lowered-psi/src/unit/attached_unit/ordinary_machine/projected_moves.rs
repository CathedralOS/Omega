//! Projected owned operands move a declared subtree out of an operation
//! result carrier. When the accumulated projections cover every admissible
//! field, the terminal frontier retires the carrier at the consuming call,
//! so the return roster must not schedule a whole-root discard for it.
//!
//! Coverage uses the shared type-directed residual query. Producer opcode
//! families do not determine custody: the exact result declaration does,
//! while the receiving verifier independently checks the emitted frontier.

use std::collections::{BTreeMap, BTreeSet};

use crate::direct_calls::call_structural_arguments;
use crate::unit::{
    Operation, PlaceId, StructuralAccess, StructuralMultiplicity, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeDeclaration,
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
        let arguments = call_structural_arguments(&operation.kind);
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
/// call. The zero residual budget avoids enumerating untouched large arrays.
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
    terminal_semantics::partial_affine_residuals(types, structural_type, moved, 0)
        .is_some_and(|residuals| residuals.is_empty())
}
