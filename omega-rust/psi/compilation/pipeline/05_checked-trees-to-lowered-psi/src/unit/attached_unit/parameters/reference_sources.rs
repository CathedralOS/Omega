//! Reference provenance uses an ending `Referent` marker for the borrowed
//! leaf, including a borrow formed at non-reference storage. Ordinary call
//! projections instead cross an actual reference carrier at that segment.

use super::{
    LoweringError, StructuralArgument, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, lower_structural_arguments, unsupported,
};
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralPathSegment,
};

/// Resolve the original source home and typed prefix through the ordinary
/// argument owner, then retain the reference-source leaf marker. Formation,
/// access and escape permission remain with reference custody validation.
pub(crate) fn lower_reference_source(
    source: &CheckedUnitStructuralArgumentPlan,
    structural_types: &[StructuralTypeDeclaration],
    parameters: &[StructuralParameterDeclaration],
    trivial_affine_locals: &[StructuralPlaceDeclaration],
    structural_results: &[(StructuralPlaceDeclaration, bool)],
    primitive_locals: &[super::super::primitive_locals::PrimitiveLocal],
) -> Result<StructuralArgument, LoweringError> {
    if source.access == CheckedStructuralAccess::Owned {
        return unsupported("reference source cannot transfer owned custody");
    }
    let mut prefix = source.clone();
    let has_leaf_marker = matches!(
        prefix.path.last(),
        Some(CheckedUnitStructuralPathSegment::Referent)
    );
    if has_leaf_marker {
        prefix.path.pop();
    }
    let arguments = lower_structural_arguments(
        std::slice::from_ref(&prefix),
        structural_types,
        parameters,
        trivial_affine_locals,
        structural_results,
        &[],
        primitive_locals,
        None,
        &mut std::iter::empty(),
        None,
    )?;
    let [mut argument] = <[StructuralArgument; 1]>::try_from(arguments)
        .map_err(|_| LoweringError::Unsupported("reference source is not one argument"))?;
    if has_leaf_marker {
        argument
            .path
            .push(terminal_psi::StructuralPathSegment::Referent);
    }
    Ok(argument)
}

#[cfg(test)]
mod tests;
