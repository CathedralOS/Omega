//! Decomposes indexed and pointee place-copy shapes into exact byte offsets.

use super::*;

pub(super) fn compiler_place_copy_from_frame_base_indexed_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<(usize, usize, usize, usize, usize, usize), Diagnostic> {
    let target_offset = target.const_offset().ok_or_else(|| {
        Diagnostic::error("final frame-base-indexed copy target is not direct frame storage")
    })?;
    if source.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        || target.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        return Err(Diagnostic::error(
            "final frame-base-indexed copy does not use runtime-frame storage",
        ));
    }
    let (
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) = compiler_single_direct_indexed_place_offsets(source)?;
    if index_region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final frame-base-indexed copy index is not captured in the runtime frame",
        ));
    }
    Ok((
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        target_offset,
    ))
}

pub(super) fn compiler_place_copy_from_machine_indexed_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let target_offset = target.const_offset().ok_or_else(|| {
        Diagnostic::error("final machine-indexed copy target is not direct runtime storage")
    })?;
    if source.region != omega_target_operations::RuntimeStorageRegion::Machine {
        return Err(Diagnostic::error(
            "final machine-indexed copy source is not machine storage",
        ));
    }
    let (
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) = compiler_single_direct_indexed_place_offsets(source)?;
    Ok((
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        target_offset,
    ))
}

pub(super) fn compiler_place_copy_to_machine_indexed_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let source_offset = source.const_offset().ok_or_else(|| {
        Diagnostic::error("final to-machine-indexed copy source is not direct runtime storage")
    })?;
    if target.region != omega_target_operations::RuntimeStorageRegion::Machine {
        return Err(Diagnostic::error(
            "final to-machine-indexed copy target is not machine storage",
        ));
    }
    let (
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) = compiler_single_direct_indexed_place_offsets(target)?;
    Ok((
        source_offset,
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ))
}

pub(super) fn compiler_place_copy_from_frame_base_double_indexed_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let target_offset = target.const_offset().ok_or_else(|| {
        Diagnostic::error("final frame-double-indexed copy target is not direct storage")
    })?;
    if source.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final frame-double-indexed copy source is not frame storage",
        ));
    }
    let mut base_byte_offset = 0usize;
    let mut field_byte_offset = 0usize;
    let mut indices = Vec::new();
    for step in source.steps() {
        match step {
            omega_target_operations::PlaceStep::ConstOffset(offset) if indices.is_empty() => {
                base_byte_offset += *offset;
            }
            omega_target_operations::PlaceStep::ConstOffset(offset) => {
                field_byte_offset += *offset;
            }
            omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            } if indices.len() < 2 => indices.push((
                *index_region,
                *index_offset,
                *index_byte_size,
                *element_byte_size,
            )),
            _ => {
                return Err(Diagnostic::error(
                    "final frame-double-indexed copy source is not doubly indexed inline storage",
                ));
            }
        }
    }
    let [
        (outer_region, outer_offset, outer_size, outer_stride),
        (inner_region, inner_offset, inner_size, inner_stride),
    ] = indices.as_slice()
    else {
        return Err(Diagnostic::error(
            "final frame-double-indexed copy source does not have two indices",
        ));
    };
    Ok((
        base_byte_offset,
        *outer_region,
        *outer_offset,
        *outer_size,
        *outer_stride,
        *inner_region,
        *inner_offset,
        *inner_size,
        *inner_stride,
        field_byte_offset,
        target_offset,
    ))
}

#[allow(clippy::type_complexity)]
pub(super) fn compiler_place_copy_from_machine_double_indexed_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let target_offset = target.const_offset().ok_or_else(|| {
        Diagnostic::error("final machine-double-indexed copy target is not direct storage")
    })?;
    if source.region != omega_target_operations::RuntimeStorageRegion::Machine {
        return Err(Diagnostic::error(
            "final machine-double-indexed copy source is not machine storage",
        ));
    }
    let mut base_byte_offset = 0usize;
    let mut field_byte_offset = 0usize;
    let mut indices = Vec::new();
    for step in source.steps() {
        match step {
            omega_target_operations::PlaceStep::ConstOffset(offset) if indices.is_empty() => {
                base_byte_offset += *offset;
            }
            omega_target_operations::PlaceStep::ConstOffset(offset) => {
                field_byte_offset += *offset;
            }
            omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            } if indices.len() < 2 => indices.push((
                *index_region,
                *index_offset,
                *index_byte_size,
                *element_byte_size,
            )),
            _ => {
                return Err(Diagnostic::error(
                    "final machine-double-indexed source is not doubly indexed inline storage",
                ));
            }
        }
    }
    let [
        (outer_region, outer_offset, outer_size, outer_stride),
        (inner_region, inner_offset, inner_size, inner_stride),
    ] = indices.as_slice()
    else {
        return Err(Diagnostic::error(
            "final machine-double-indexed source does not have two indices",
        ));
    };
    Ok((
        base_byte_offset,
        *outer_region,
        *outer_offset,
        *outer_size,
        *outer_stride,
        *inner_region,
        *inner_offset,
        *inner_size,
        *inner_stride,
        field_byte_offset,
        target_offset,
    ))
}

#[allow(clippy::type_complexity)]
pub(super) fn compiler_place_copy_to_machine_double_indexed_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let source_offset = source.const_offset().ok_or_else(|| {
        Diagnostic::error("final to-machine-double-indexed source is not direct storage")
    })?;
    if target.region != omega_target_operations::RuntimeStorageRegion::Machine {
        return Err(Diagnostic::error(
            "final to-machine-double-indexed target is not machine storage",
        ));
    }
    let mut base_byte_offset = 0usize;
    let mut field_byte_offset = 0usize;
    let mut indices = Vec::new();
    for step in target.steps() {
        match step {
            omega_target_operations::PlaceStep::ConstOffset(offset) if indices.is_empty() => {
                base_byte_offset += *offset;
            }
            omega_target_operations::PlaceStep::ConstOffset(offset) => {
                field_byte_offset += *offset;
            }
            omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            } if indices.len() < 2 => indices.push((
                *index_region,
                *index_offset,
                *index_byte_size,
                *element_byte_size,
            )),
            _ => {
                return Err(Diagnostic::error(
                    "final to-machine-double-indexed target is not doubly indexed inline storage",
                ));
            }
        }
    }
    let [
        (outer_region, outer_offset, outer_size, outer_stride),
        (inner_region, inner_offset, inner_size, inner_stride),
    ] = indices.as_slice()
    else {
        return Err(Diagnostic::error(
            "final to-machine-double-indexed target does not have two indices",
        ));
    };
    Ok((
        source_offset,
        base_byte_offset,
        *outer_region,
        *outer_offset,
        *outer_size,
        *outer_stride,
        *inner_region,
        *inner_offset,
        *inner_size,
        *inner_stride,
        field_byte_offset,
    ))
}

#[allow(clippy::type_complexity)]
pub(super) fn compiler_place_copy_machine_indexed_pair_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let machine = omega_target_operations::RuntimeStorageRegion::Machine;
    if source.region != machine || target.region != machine {
        return Err(Diagnostic::error(
            "final machine-indexed pair is not rooted entirely in machine storage",
        ));
    }
    let (
        source_base_byte_offset,
        source_index_region,
        source_index_offset,
        source_index_byte_size,
        source_element_byte_size,
        source_field_byte_offset,
    ) = compiler_single_direct_indexed_place_offsets(source)?;
    let (
        target_base_byte_offset,
        target_index_region,
        target_index_offset,
        target_index_byte_size,
        target_element_byte_size,
        target_field_byte_offset,
    ) = compiler_single_direct_indexed_place_offsets(target)?;
    Ok((
        source_base_byte_offset,
        source_index_region,
        source_index_offset,
        source_index_byte_size,
        source_element_byte_size,
        source_field_byte_offset,
        target_base_byte_offset,
        target_index_region,
        target_index_offset,
        target_index_byte_size,
        target_element_byte_size,
        target_field_byte_offset,
    ))
}

pub(super) fn compiler_single_direct_indexed_place_offsets(
    place: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let mut base_byte_offset = 0usize;
    let mut indexed = None;
    let mut field_byte_offset = 0usize;
    for step in place.steps() {
        match step {
            omega_target_operations::PlaceStep::ConstOffset(offset) if indexed.is_none() => {
                base_byte_offset = base_byte_offset.checked_add(*offset).ok_or_else(|| {
                    Diagnostic::error("final direct-indexed place base offset overflows")
                })?;
            }
            omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            } if indexed.is_none() => {
                indexed = Some((
                    *index_region,
                    *index_offset,
                    *index_byte_size,
                    *element_byte_size,
                ));
            }
            omega_target_operations::PlaceStep::ConstOffset(offset) => {
                field_byte_offset = field_byte_offset.checked_add(*offset).ok_or_else(|| {
                    Diagnostic::error("final direct-indexed place field offset overflows")
                })?;
            }
            _ => {
                return Err(Diagnostic::error(
                    "final place-copy operand is not singly indexed inline storage",
                ));
            }
        }
    }
    let Some((index_region, index_offset, index_byte_size, element_byte_size)) = indexed else {
        return Err(Diagnostic::error(
            "final direct-indexed place has no runtime index",
        ));
    };
    Ok((
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ))
}

#[allow(clippy::type_complexity)]
pub(super) fn compiler_double_indexed_place_offsets(
    place: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let mut base_byte_offset = 0usize;
    let mut field_byte_offset = 0usize;
    let mut indices = Vec::new();
    for step in place.steps() {
        match step {
            omega_target_operations::PlaceStep::ConstOffset(offset) if indices.is_empty() => {
                base_byte_offset = base_byte_offset.checked_add(*offset).ok_or_else(|| {
                    Diagnostic::error("final double-indexed place base offset overflows")
                })?;
            }
            omega_target_operations::PlaceStep::ConstOffset(offset) => {
                field_byte_offset = field_byte_offset.checked_add(*offset).ok_or_else(|| {
                    Diagnostic::error("final double-indexed place field offset overflows")
                })?;
            }
            omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            } if indices.len() < 2 => indices.push((
                *index_region,
                *index_offset,
                *index_byte_size,
                *element_byte_size,
            )),
            _ => {
                return Err(Diagnostic::error(
                    "final integer-write target is not doubly indexed inline storage",
                ));
            }
        }
    }
    let [
        (outer_region, outer_offset, outer_size, outer_stride),
        (inner_region, inner_offset, inner_size, inner_stride),
    ] = indices.as_slice()
    else {
        return Err(Diagnostic::error(
            "final double-indexed integer-write target does not have two indices",
        ));
    };
    Ok((
        base_byte_offset,
        *outer_region,
        *outer_offset,
        *outer_size,
        *outer_stride,
        *inner_region,
        *inner_offset,
        *inner_size,
        *inner_stride,
        field_byte_offset,
    ))
}

pub(super) fn compiler_place_copy_indexed_to_pointee_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<(usize, usize, usize, usize, usize, usize, usize), Diagnostic> {
    if source.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        || target.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        return Err(Diagnostic::error(
            "final indexed-to-pointee copy does not use one shared runtime-frame base",
        ));
    }
    let (
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        source_field_byte_offset,
    ) = compiler_single_indexed_place_offsets(source)?;
    if index_region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final indexed-to-pointee copy index is not captured in the runtime frame",
        ));
    }
    let (pointer_byte_offset, target_field_byte_offset) = compiler_frame_pointee_offsets(target)?;
    Ok((
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        source_field_byte_offset,
        pointer_byte_offset,
        target_field_byte_offset,
    ))
}

pub(super) fn compiler_place_copy_indexed_to_pointee_by_region_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    if source.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        || target.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        return Err(Diagnostic::error(
            "final cross-region indexed-to-pointee copy is not frame-rooted",
        ));
    }
    let (
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        source_field_byte_offset,
    ) = compiler_single_indexed_place_offsets(source)?;
    if index_region != omega_target_operations::RuntimeStorageRegion::Machine {
        return Err(Diagnostic::error(
            "final cross-region indexed-to-pointee copy has no machine index",
        ));
    }
    let (pointer_byte_offset, target_field_byte_offset) = compiler_frame_pointee_offsets(target)?;
    Ok((
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        source_field_byte_offset,
        pointer_byte_offset,
        target_field_byte_offset,
    ))
}

pub(super) fn compiler_place_copy_to_indexed_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<(usize, usize, usize, usize, usize, usize), Diagnostic> {
    let source_offset = source.const_offset().ok_or_else(|| {
        Diagnostic::error("final to-indexed copy source is not direct runtime storage")
    })?;
    if source.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        || target.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        return Err(Diagnostic::error(
            "final to-indexed copy does not use one shared runtime-frame base",
        ));
    }
    let (
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) = compiler_single_indexed_place_offsets(target)?;
    if index_region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final to-indexed copy index is not captured in the runtime frame",
        ));
    }
    Ok((
        source_offset,
        descriptor_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ))
}

pub(super) fn compiler_place_copy_to_indexed_by_region_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let source_offset = source.const_offset().ok_or_else(|| {
        Diagnostic::error("final cross-region to-indexed source is not direct storage")
    })?;
    if target.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final cross-region to-indexed descriptor is not captured in the runtime frame",
        ));
    }
    let (
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) = compiler_single_indexed_place_offsets(target)?;
    if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        && index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        return Err(Diagnostic::error(
            "final to-indexed copy uses the shared runtime-frame recipe",
        ));
    }
    Ok((
        source_offset,
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ))
}

pub(super) fn compiler_single_indexed_place_offsets(
    place: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    match place.steps() {
        [
            omega_target_operations::PlaceStep::ConstOffset(descriptor_offset),
            omega_target_operations::PlaceStep::Deref,
            omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            },
        ] => Ok((
            *descriptor_offset,
            *index_region,
            *index_offset,
            *index_byte_size,
            *element_byte_size,
            0,
        )),
        [
            omega_target_operations::PlaceStep::ConstOffset(descriptor_offset),
            omega_target_operations::PlaceStep::Deref,
            omega_target_operations::PlaceStep::ScaledIndex {
                index_region,
                index_offset,
                index_byte_size,
                element_byte_size,
            },
            omega_target_operations::PlaceStep::ConstOffset(field_byte_offset),
        ] => Ok((
            *descriptor_offset,
            *index_region,
            *index_offset,
            *index_byte_size,
            *element_byte_size,
            *field_byte_offset,
        )),
        _ => Err(Diagnostic::error(
            "final place-copy operand is not a single indexed place",
        )),
    }
}

pub(super) fn compiler_place_copy_from_indexed_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<
    (
        usize,
        omega_target_operations::RuntimeStorageRegion,
        usize,
        usize,
        usize,
        usize,
        usize,
    ),
    Diagnostic,
> {
    let target_offset = target.const_offset().ok_or_else(|| {
        Diagnostic::error("final from-indexed copy target is not direct runtime storage")
    })?;
    if source.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final from-indexed copy descriptor is not captured in the runtime frame",
        ));
    }
    let (
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    ) = compiler_single_indexed_place_offsets(source)?;
    Ok((
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
        target_offset,
    ))
}

pub(super) fn compiler_place_copy_pointee_pair_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<(usize, usize, usize, usize), Diagnostic> {
    let (source_pointer_byte_offset, source_field_byte_offset) =
        compiler_frame_pointee_offsets(source)?;
    let (target_pointer_byte_offset, target_field_byte_offset) =
        compiler_frame_pointee_offsets(target)?;
    Ok((
        source_pointer_byte_offset,
        source_field_byte_offset,
        target_pointer_byte_offset,
        target_field_byte_offset,
    ))
}

pub(super) fn compiler_frame_pointee_offsets(
    place: &omega_target_operations::Place,
) -> Result<(usize, usize), Diagnostic> {
    if place.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final place-copy pointer is not captured in the runtime frame",
        ));
    }
    match place.steps() {
        [
            omega_target_operations::PlaceStep::ConstOffset(pointer_byte_offset),
            omega_target_operations::PlaceStep::Deref,
        ] => Ok((*pointer_byte_offset, 0)),
        [
            omega_target_operations::PlaceStep::ConstOffset(pointer_byte_offset),
            omega_target_operations::PlaceStep::Deref,
            omega_target_operations::PlaceStep::ConstOffset(field_byte_offset),
        ] => Ok((*pointer_byte_offset, *field_byte_offset)),
        _ => Err(Diagnostic::error(
            "final place-copy operand is not a frame-held pointee",
        )),
    }
}

pub(super) fn compiler_place_copy_from_pointee_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<(usize, usize, usize), Diagnostic> {
    let target_offset = target.const_offset().ok_or_else(|| {
        Diagnostic::error("final from-pointee copy target is not direct runtime storage")
    })?;
    if source.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final from-pointee copy pointer is not captured in the runtime frame",
        ));
    }
    let (pointer_byte_offset, field_byte_offset) = match source.steps() {
        [
            omega_target_operations::PlaceStep::ConstOffset(pointer_byte_offset),
            omega_target_operations::PlaceStep::Deref,
        ] => (*pointer_byte_offset, 0),
        [
            omega_target_operations::PlaceStep::ConstOffset(pointer_byte_offset),
            omega_target_operations::PlaceStep::Deref,
            omega_target_operations::PlaceStep::ConstOffset(field_byte_offset),
        ] => (*pointer_byte_offset, *field_byte_offset),
        _ => {
            return Err(Diagnostic::error(
                "final from-pointee copy source is not a frame-held pointee",
            ));
        }
    };
    Ok((pointer_byte_offset, field_byte_offset, target_offset))
}

pub(super) fn compiler_place_copy_to_pointee_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<(usize, usize, usize), Diagnostic> {
    let source_offset = source.const_offset().ok_or_else(|| {
        Diagnostic::error("final pointee-copy source is not direct runtime storage")
    })?;
    if target.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Err(Diagnostic::error(
            "final pointee-copy pointer is not captured in the runtime frame",
        ));
    }
    let (pointer_byte_offset, field_byte_offset) = match target.steps() {
        [
            omega_target_operations::PlaceStep::ConstOffset(pointer_byte_offset),
            omega_target_operations::PlaceStep::Deref,
        ] => (*pointer_byte_offset, 0),
        [
            omega_target_operations::PlaceStep::ConstOffset(pointer_byte_offset),
            omega_target_operations::PlaceStep::Deref,
            omega_target_operations::PlaceStep::ConstOffset(field_byte_offset),
        ] => (*pointer_byte_offset, *field_byte_offset),
        _ => {
            return Err(Diagnostic::error(
                "final pointee-copy target is not a frame-held pointee",
            ));
        }
    };
    Ok((source_offset, pointer_byte_offset, field_byte_offset))
}

pub(super) fn compiler_exit_indirect_result_copy_offsets(
    source: &omega_target_operations::Place,
    target: &omega_target_operations::Place,
) -> Result<(usize, usize), Diagnostic> {
    let (source_offset, pointer_byte_offset, field_byte_offset) =
        compiler_place_copy_to_pointee_offsets(source, target)?;
    if field_byte_offset != 0 {
        return Err(Diagnostic::error(
            "final indirect-result copy does not begin at the result destination",
        ));
    }
    Ok((source_offset, pointer_byte_offset))
}
