//! Descriptor observations retain their complete storage coordinate. This
//! joins declaration geometry only; source availability remains the shared
//! ownership walk, and selector bounds remain operation-owned certificates.
use optimization_unit::PsiOptimizationFunction;
use terminal_psi::{StructuralAccess, StructuralArgument, StructuralTypeDeclaration};

pub(in crate::legalization) fn length_geometry(
    function: &PsiOptimizationFunction,
    source: &StructuralArgument,
    types: &[StructuralTypeDeclaration],
) -> Option<crate::structural_inputs::structural_reference_input::ReferenceSourceGeometry> {
    let actual = optimization_unit_semantics::structural_place_access(function, source.place)
        .or_else(|| {
            super::byte_views::descriptor_result(function, source.place, types)
                .map(|_| StructuralAccess::SharedBorrow)
        })?;
    if !matches!(
        source.access,
        StructuralAccess::Owned | StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) || !(actual == source.access
        || actual == StructuralAccess::Owned
        || actual == StructuralAccess::MutableBorrow
            && source.access == StructuralAccess::SharedBorrow)
    {
        return None;
    }
    let root = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source.place)
        .map(|parameter| parameter.structural_type)
        .or_else(|| {
            function
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters)
                .find(|parameter| parameter.place == source.place)
                .map(|parameter| parameter.structural_type)
        })
        .or_else(|| {
            super::structural_case::source_result(function, source.place)
                .ok()
                .map(|(_, result)| result.structural_type)
        })?;
    crate::structural_inputs::structural_reference_input::element_view_length_geometry(
        root,
        &source.path,
        types,
    )
}
