//! Classifies exact compiler place-write shapes before encoding and relocation replay.

use super::*;

#[derive(Clone, Copy)]
pub(super) enum CompilerBodyPlaceIntegerWriteShape {
    Direct {
        byte_offset: usize,
    },
    Pointee {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
    },
    FrameIndexed {
        descriptor_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    FrameBaseIndexed {
        base_byte_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    FrameBaseDoubleIndexed {
        base_byte_offset: usize,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        field_byte_offset: usize,
    },
    MachineIndexed {
        base_byte_offset: usize,
        index_region: omega_target_operations::RuntimeStorageRegion,
        index_offset: usize,
        index_byte_size: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    MachineDoubleIndexed {
        base_byte_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        field_byte_offset: usize,
    },
    PointeeDoubleIndexed {
        descriptor_offset: usize,
        outer_index_region: omega_target_operations::RuntimeStorageRegion,
        outer_index_offset: usize,
        outer_index_byte_size: usize,
        outer_stride: usize,
        inner_index_region: omega_target_operations::RuntimeStorageRegion,
        inner_index_offset: usize,
        inner_index_byte_size: usize,
        inner_stride: usize,
        field_byte_offset: usize,
    },
    General,
}

pub(super) fn compiler_body_place_integer_write_shape(
    target: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceIntegerWriteShape, Diagnostic> {
    if let Some(byte_offset) = target.const_offset() {
        return Ok(CompilerBodyPlaceIntegerWriteShape::Direct { byte_offset });
    }
    if target.region == omega_target_operations::RuntimeStorageRegion::Machine
        && let Ok((
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        )) = compiler_single_direct_indexed_place_offsets(target)
    {
        return Ok(CompilerBodyPlaceIntegerWriteShape::MachineIndexed {
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        });
    }
    if target.region == omega_target_operations::RuntimeStorageRegion::Machine
        && let Ok((
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        )) = compiler_double_indexed_place_offsets(target)
    {
        return Ok(CompilerBodyPlaceIntegerWriteShape::MachineDoubleIndexed {
            base_byte_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        });
    }
    if target.region != omega_target_operations::RuntimeStorageRegion::RuntimeFrame {
        return Ok(CompilerBodyPlaceIntegerWriteShape::General);
    }
    if let Ok((
        descriptor_offset,
        outer_index_region,
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        inner_index_region,
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        field_byte_offset,
    )) = compiler_pointee_double_indexed_place_offsets(target)
    {
        return Ok(CompilerBodyPlaceIntegerWriteShape::PointeeDoubleIndexed {
            descriptor_offset,
            outer_index_region,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_region,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        });
    }
    if let Ok((
        base_byte_offset,
        outer_index_region,
        outer_index_offset,
        outer_index_byte_size,
        outer_stride,
        inner_index_region,
        inner_index_offset,
        inner_index_byte_size,
        inner_stride,
        field_byte_offset,
    )) = compiler_double_indexed_place_offsets(target)
        && outer_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        && inner_index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        return Ok(CompilerBodyPlaceIntegerWriteShape::FrameBaseDoubleIndexed {
            base_byte_offset,
            outer_index_offset,
            outer_index_byte_size,
            outer_stride,
            inner_index_offset,
            inner_index_byte_size,
            inner_stride,
            field_byte_offset,
        });
    }
    if let Ok((
        base_byte_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )) = compiler_single_direct_indexed_place_offsets(target)
        && index_region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
    {
        return Ok(CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        });
    }
    if let Ok((
        descriptor_offset,
        index_region,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )) = compiler_single_indexed_place_offsets(target)
    {
        return Ok(CompilerBodyPlaceIntegerWriteShape::FrameIndexed {
            descriptor_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        });
    }
    match target.steps() {
        [
            omega_target_operations::PlaceStep::ConstOffset(pointer_byte_offset),
            omega_target_operations::PlaceStep::Deref,
        ] => Ok(CompilerBodyPlaceIntegerWriteShape::Pointee {
            pointer_byte_offset: *pointer_byte_offset,
            field_byte_offset: 0,
        }),
        [
            omega_target_operations::PlaceStep::ConstOffset(pointer_byte_offset),
            omega_target_operations::PlaceStep::Deref,
            omega_target_operations::PlaceStep::ConstOffset(field_byte_offset),
        ] => Ok(CompilerBodyPlaceIntegerWriteShape::Pointee {
            pointer_byte_offset: *pointer_byte_offset,
            field_byte_offset: *field_byte_offset,
        }),
        _ => Ok(CompilerBodyPlaceIntegerWriteShape::General),
    }
}

pub(super) fn compiler_body_place_write_shape_with_cross_region_frame_base(
    target: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceIntegerWriteShape, Diagnostic> {
    if target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
        && let Ok((
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        )) = compiler_single_direct_indexed_place_offsets(target)
    {
        return Ok(CompilerBodyPlaceIntegerWriteShape::FrameBaseIndexed {
            base_byte_offset,
            index_region,
            index_offset,
            index_byte_size,
            element_byte_size,
            field_byte_offset,
        });
    }
    compiler_body_place_integer_write_shape(target)
}

pub(super) fn compiler_body_place_binary_write_shape(
    target: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceIntegerWriteShape, Diagnostic> {
    compiler_body_place_write_shape_with_cross_region_frame_base(target)
}

pub(super) fn compiler_body_place_convert_write_shape(
    target: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceIntegerWriteShape, Diagnostic> {
    compiler_body_place_write_shape_with_cross_region_frame_base(target)
}

pub(super) fn compiler_body_place_string_write_shape(
    target: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceIntegerWriteShape, Diagnostic> {
    compiler_body_place_write_shape_with_cross_region_frame_base(target)
}

pub(super) fn compiler_body_place_bounded_buffer_write_shape(
    target: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceIntegerWriteShape, Diagnostic> {
    compiler_body_place_write_shape_with_cross_region_frame_base(target)
}

pub(super) fn compiler_body_place_bounded_buffer_literal_append_shape(
    target: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceIntegerWriteShape, Diagnostic> {
    compiler_body_place_write_shape_with_cross_region_frame_base(target)
}

pub(super) fn compiler_body_place_bounded_buffer_source_append_shape(
    target: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceIntegerWriteShape, Diagnostic> {
    compiler_body_place_write_shape_with_cross_region_frame_base(target)
}

pub(super) fn compiler_body_place_address_write_shape(
    source: &omega_target_operations::Place,
) -> Result<CompilerBodyPlaceIntegerWriteShape, Diagnostic> {
    compiler_body_place_write_shape_with_cross_region_frame_base(source)
}
