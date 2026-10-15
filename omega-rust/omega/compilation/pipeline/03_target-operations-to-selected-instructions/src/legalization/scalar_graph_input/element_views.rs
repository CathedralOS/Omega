//! Descriptor observations retain their complete storage coordinate. This
//! joins declaration geometry only; source availability remains the shared
//! ownership walk, and selector bounds remain operation-owned certificates.
use optimization_unit::PsiOptimizationFunction;
use terminal_psi::{StructuralAccess, StructuralArgument, StructuralTypeDeclaration};

pub(in crate::legalization) fn descriptor_geometry(
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
        })
        .or_else(|| {
            super::literals::declaration_producer(function, source.place)
                .map(|(_, identity)| identity)
        })?;
    let requested = if source.access == StructuralAccess::Owned {
        StructuralAccess::SharedBorrow
    } else {
        source.access
    };
    if !optimization_unit_semantics::descriptor_argument_accessible(
        function, source, types, requested,
    ) {
        return None;
    }
    crate::structural_inputs::structural_reference_input::descriptor_source_geometry(
        root,
        &source.path,
        types,
    )
}

/// Element counts use the selected two-word descriptor, in elements.
pub(in crate::legalization) fn length_geometry(
    function: &PsiOptimizationFunction,
    source: &StructuralArgument,
    types: &[StructuralTypeDeclaration],
) -> Option<crate::structural_inputs::structural_reference_input::ReferenceSourceGeometry> {
    let mut geometry = descriptor_geometry(function, source, types)?;
    if !geometry.readable
        || !types.iter().any(|declaration| {
            declaration.id == geometry.selected
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::ElementView { .. }
                )
        })
    {
        return None;
    }
    geometry.byte_offset = geometry.byte_offset.checked_add(8)?;
    Some(geometry)
}

/// Byte counts select their own carrier: inline owners have a live header,
/// dynamic views a two-word descriptor, and bounded views a declared extent.
pub(in crate::legalization) fn byte_length_geometry(
    function: &PsiOptimizationFunction,
    source: &StructuralArgument,
    types: &[StructuralTypeDeclaration],
) -> Option<(
    crate::structural_inputs::structural_reference_input::ReferenceSourceGeometry,
    terminal_psi::ByteSequenceCarrier,
)> {
    let mut geometry = descriptor_geometry(function, source, types)?;
    if !geometry.readable {
        return None;
    }
    let terminal_psi::StructuralTypeShape::ByteSequence(carrier) = types
        .iter()
        .find(|declaration| declaration.id == geometry.selected)?
        .shape
    else {
        return None;
    };
    if matches!(
        carrier,
        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
    ) {
        geometry.byte_offset = geometry.byte_offset.checked_add(8)?;
    }
    Some((geometry, carrier))
}
