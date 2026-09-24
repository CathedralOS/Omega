//! Split a Terminal primitive-leaf projection for the abstract operations.
//!
//! Terminal spells a primitive leaf (`PrimitiveScalarRead`,
//! `WriteOnlyPrimitiveStore`) as one structural path whose elements may be
//! runtime-selected; each runtime element's bound is an obligation the
//! verifier already discharged. The abstract operations still take either a
//! canonical static path, or a canonical path to a fixed array plus one
//! trailing runtime element (`IndexedPrimitiveRead`,
//! `WriteOnlyIndexedPrimitiveStore`). A field or a further index after a
//! runtime element is refused here until lowering composes `(index, stride)`
//! runs for primitive leaves the way leaf copies already do.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ObligationId, ScalarType, StructuralTypeId, ValueId,
};
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

pub(super) enum PrimitiveProjection {
    /// Every element is literal: the canonical path to the primitive leaf.
    Static(Vec<CanonicalStructuralPathSegment>),
    /// A canonical path to a fixed array of primitive `element`s, and the one
    /// runtime element that ends the Terminal path.
    TrailingRuntimeIndex {
        array: Vec<CanonicalStructuralPathSegment>,
        element: ScalarType,
        index: ValueId,
        obligation: ObligationId,
    },
}

pub(super) fn split(
    structural_types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<PrimitiveProjection> {
    let Some((last, prefix)) = path.split_last() else {
        return Some(PrimitiveProjection::Static(Vec::new()));
    };
    let Some((index, obligation)) = last.runtime_index() else {
        return terminal_semantics::canonical_primitive_path(structural_types.iter(), root, path)
            .map(PrimitiveProjection::Static);
    };
    let (array, tip) =
        terminal_semantics::canonical_static_projection(structural_types.iter(), root, prefix)?;
    let StructuralTypeShape::FixedArray { element, .. } = tip.shape else {
        return None;
    };
    let element = match structural_types
        .iter()
        .find(|declaration| declaration.id == element)?
        .shape
    {
        StructuralTypeShape::PrimitiveScalar(scalar) => scalar,
        _ => return None,
    };
    Some(PrimitiveProjection::TrailingRuntimeIndex {
        array,
        element,
        index,
        obligation,
    })
}
