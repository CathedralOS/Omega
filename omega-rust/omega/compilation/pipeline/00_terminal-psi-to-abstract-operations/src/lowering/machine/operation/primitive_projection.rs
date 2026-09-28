//! Reconstruct a Terminal primitive-leaf projection for the abstract operations.
//!
//! Terminal spells a primitive leaf (`PrimitiveScalarRead`,
//! `WriteOnlyPrimitiveStore`) as one structural path whose elements may be
//! runtime-selected at any depth (`grid[i][j]`, `ents[i].hp`); each runtime
//! element's bound is an obligation the verifier already discharged at the
//! operation. The abstract read and store keep that exact path, so there is
//! one route whatever the mix of fields and literal or runtime elements, and
//! target lowering scales each runtime element into the address as an
//! `(index, stride)` run the way leaf copies do. Lowering reconstructs only
//! the leaf's scalar type and that every selector is an integer carrier the
//! 64-bit address model can extend; it never restates or re-derives a bound.

use semantic_vocabulary::{ScalarType, StructuralTypeId, ValueId};
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration};

/// The primitive leaf `path` selects beneath `root`, when every runtime
/// selector has an integer type of at most 64 bits.
pub(super) fn leaf_type(
    structural_types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    selector_type: impl Fn(ValueId) -> Option<ScalarType>,
) -> Option<ScalarType> {
    path.iter()
        .filter_map(StructuralPathSegment::runtime_index)
        .all(|(index, _)| runtime_selector(selector_type(index)))
        .then(|| terminal_semantics::primitive_projection_type(structural_types.iter(), root, path))
        .flatten()
}

/// Whether a selector's type is an integer carrier the address model scales:
/// narrower carriers are sign- or zero-extended to 64 bits, and a signed
/// selector's `0 <= index` is part of the verified obligation.
pub(super) fn runtime_selector(selector: Option<ScalarType>) -> bool {
    matches!(selector, Some(ScalarType::Integer(integer)) if integer.bits() <= 64)
}
