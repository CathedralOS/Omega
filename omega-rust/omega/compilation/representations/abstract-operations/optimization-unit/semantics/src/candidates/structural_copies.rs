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
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::{StructuralFieldType, StructuralPathSegment, StructuralTypeShape};

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

/// The verifier-handle spelling of one ordinary leaf-copy projection
/// beneath `root`: a `Field` step resolves its source-level identity to the
/// reached `Record`/`Mixed` common-field roster's `StructuralFieldId`,
/// keeping only a `Structural` carrier — a scalar, byte-sequence, or erased
/// leaf mints a carrier this walk cannot follow into stored contents — and
/// a `FixedIndex` step keeps its element descent inside an in-bounds fixed
/// array. `None` when the roster lacks the name, the index leaves a fixed
/// array, or the segment has no canonical step — `Case`, `RuntimeIndex`,
/// `Referent`, `FixedByteRange`.
pub(crate) fn canonicalize_ordinary_path(
    unit: &PsiOptimizationUnit,
    mut current: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    let mut canonical = Vec::with_capacity(path.len());
    for segment in path {
        match segment {
            StructuralPathSegment::Field(identity) => {
                let fields = match shape(unit, current)? {
                    StructuralTypeShape::Record { fields }
                    | StructuralTypeShape::Mixed { fields, .. } => fields,
                    _ => return None,
                };
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                canonical.push(CanonicalStructuralPathSegment::Field(field.id));
                current = next;
            }
            StructuralPathSegment::FixedIndex(index) => match shape(unit, current)? {
                StructuralTypeShape::FixedArray { element, length } if index < length => {
                    canonical.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                    current = *element;
                }
                _ => return None,
            },
            StructuralPathSegment::Case(_)
            | StructuralPathSegment::RuntimeIndex { .. }
            | StructuralPathSegment::Referent
            | StructuralPathSegment::FixedByteRange { .. } => return None,
        }
    }
    Some(canonical)
}

/// The declared structural shape of `type_id` in `unit`.
fn shape(unit: &PsiOptimizationUnit, type_id: StructuralTypeId) -> Option<&StructuralTypeShape> {
    unit.structural_types
        .as_slice()
        .iter()
        .find(|entry| entry.id == type_id)
        .map(|entry| &entry.shape)
}
