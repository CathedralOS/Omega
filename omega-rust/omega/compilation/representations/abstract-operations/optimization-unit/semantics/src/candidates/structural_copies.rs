//! Leaf-copy projection predicates shared by candidate replays.
//!
//! A `StructuralLeafCopy` result holds exactly its source's contents at the
//! copied projection: the copy is fresh storage of an `Unrestricted` or
//! owned non-reference leaf, so an observation of the result replays
//! against the source's own establishment chain with the projection
//! prefixed. A `StructuralCopyPath::Case` projection already spells
//! verifier handles; a `StructuralCopyPath::Ordinary` projection spells
//! source-level field identities, which the canonicalizer resolves to the
//! reached roster's verifier `StructuralFieldId` before any replay step
//! consumes the path — a name absent from the roster, a non-structural
//! field carrier, and every segment with no canonical step leave the copy
//! uncrossed.

use abstract_operations::{AbstractOperation as O, StructuralCopyPath};
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};

/// The `StructuralLeafCopy` that produces `place` — its source place and
/// spelled projection — or `None` when `place` is not that copy's operation
/// result.
pub(crate) fn leaf_copy_source<'a>(
    function: &'a PsiOptimizationFunction,
    place: PlaceId,
) -> Option<(PlaceId, &'a StructuralCopyPath)> {
    let StructuralPlaceKind::OperationResult { producer, .. } = function
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)?
        .kind
    else {
        return None;
    };
    function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            O::StructuralLeafCopy {
                psi_operation,
                result,
                source,
                path,
            } if *psi_operation == producer && result.place == place => Some((*source, path)),
            _ => None,
        })
}
