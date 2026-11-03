//! Descriptor observations retain their complete storage coordinate. This
//! joins declaration geometry only; source availability remains the shared
//! ownership walk, and selector bounds remain operation-owned certificates.
use crate::structural_inputs::structural_reference_input::{
    ReferenceSourceGeometry, descriptor_source_geometry,
};
use optimization_unit::PsiOptimizationFunction;
use terminal_psi::{StructuralAccess, StructuralArgument, StructuralTypeDeclaration};

pub(in crate::legalization) fn descriptor_geometry(
    function: &PsiOptimizationFunction,
    source: &StructuralArgument,
    types: &[StructuralTypeDeclaration],
) -> Option<ReferenceSourceGeometry> {
    descriptor_geometry_access(function, source, types, false)
}

/// Extent observations read descriptor metadata, not payload. A write-only
/// source still supplies geometry under its own authority; a stored-referent
/// crossing would turn that observation into a read through the pointer, so
/// this lane keeps refusing it.
pub(in crate::legalization) fn descriptor_metadata_geometry(
    function: &PsiOptimizationFunction,
    source: &StructuralArgument,
    types: &[StructuralTypeDeclaration],
) -> Option<ReferenceSourceGeometry> {
    descriptor_geometry_access(function, source, types, true)
}

fn descriptor_geometry_access(
    function: &PsiOptimizationFunction,
    source: &StructuralArgument,
    types: &[StructuralTypeDeclaration],
    metadata: bool,
) -> Option<ReferenceSourceGeometry> {
    if std::env::var_os("OMEGA_DESCRIPTOR_GEOMETRY_TRACE").is_some() {
        let actual = optimization_unit_semantics::structural_place_backing_access(
            function,
            source.place,
            types.iter(),
        );
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
            });
        let requested = if source.access == StructuralAccess::Owned {
            StructuralAccess::SharedBorrow
        } else {
            source.access
        };
        let accessible = if source.access == StructuralAccess::WriteOnlyBorrow {
            optimization_unit_semantics::descriptor_metadata_argument_accessible(
                function, source, types, requested,
            )
        } else {
            optimization_unit_semantics::descriptor_argument_accessible(
                function, source, types, requested,
            )
        };
        let geometry = root
            .and_then(|root| descriptor_source_geometry(root, &source.path, types))
            .map(|geometry| {
                (
                    geometry.region_root,
                    geometry.selected,
                    geometry.readable,
                    geometry.byte_offset,
                    geometry.pointer_slots.len(),
                    geometry.elements.len(),
                )
            });
        let arrivals = function
            .blocks
            .iter()
            .flat_map(|block| {
                block.structural_parameters.iter().map(move |parameter| {
                    (
                        block.id,
                        parameter.place,
                        parameter.structural_type,
                        parameter.access,
                        parameter.multiplicity,
                    )
                })
            })
            .collect::<Vec<_>>();
        eprintln!(
            "corpus_runner: DESCRIPTOR_GEOMETRY_TRACE entry={:?} metadata={} source={:?} arrival={:?} actual={:?} root={:?} shape={:?} accessible={} geometry={:?} arrivals={:?}",
            function.entry,
            metadata,
            source,
            function.arrival_parameter(source.place),
            actual,
            root,
            root.and_then(|root| types.iter().find(|row| row.id == root))
                .map(|row| &row.shape),
            accessible,
            geometry,
            arrivals
        );
    }
    let actual = optimization_unit_semantics::structural_place_backing_access(
        function,
        source.place,
        types.iter(),
    )?;
    if (!matches!(
        source.access,
        StructuralAccess::Owned | StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) && !(metadata && source.access == StructuralAccess::WriteOnlyBorrow))
        || !(actual == source.access
            || actual == StructuralAccess::Owned
            || actual == StructuralAccess::MutableBorrow
                && source.access == StructuralAccess::SharedBorrow)
    {
        return None;
    }
    if source.access == StructuralAccess::WriteOnlyBorrow
        && source
            .path
            .iter()
            .any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Referent))
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
    if if source.access == StructuralAccess::WriteOnlyBorrow {
        !optimization_unit_semantics::descriptor_metadata_argument_accessible(
            function, source, types, requested,
        )
    } else {
        !optimization_unit_semantics::descriptor_argument_accessible(
            function, source, types, requested,
        )
    } {
        return None;
    }
    descriptor_source_geometry(root, &source.path, types)
}

/// Element counts use the selected two-word descriptor, in elements.
pub(in crate::legalization) fn length_geometry(
    function: &PsiOptimizationFunction,
    source: &StructuralArgument,
    types: &[StructuralTypeDeclaration],
) -> Option<ReferenceSourceGeometry> {
    let mut geometry = descriptor_metadata_geometry(function, source, types)?;
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
) -> Option<(ReferenceSourceGeometry, terminal_psi::ByteSequenceCarrier)> {
    let mut geometry = descriptor_metadata_geometry(function, source, types)?;
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

/// Inline fixed bytes have a declared count and no length word. Geometry still
/// checks the original source access and every reference/selector crossing.
pub(in crate::legalization) fn fixed_byte_length(
    function: &PsiOptimizationFunction,
    source: &StructuralArgument,
    types: &[StructuralTypeDeclaration],
) -> Option<u64> {
    let geometry = descriptor_metadata_geometry(function, source, types)?;
    if !geometry.readable
        || !types.iter().any(|row| {
            row.id == geometry.selected
                && matches!(
                    row.shape,
                    terminal_psi::StructuralTypeShape::FixedArray { .. }
                )
        })
    {
        return None;
    }
    terminal_semantics::fixed_byte_extent(types.iter(), geometry.selected)
}
