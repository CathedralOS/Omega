use crate::TypeLayout;
use omega_checked_trees::types::PrimitiveType;
use omega_runtime_abi::build_runtime_abi_plan;
use omega_target::NativeTarget;

/// Canonical fat-descriptor layout (`{ptr, len}`) for slices and text windows.
///
/// Single source of truth inside omega-layout: derives size/alignment from
/// `omega-runtime-abi` rather than re-deriving `2 * pointer_size` locally.
pub(super) fn fat_descriptor_layout(target: NativeTarget) -> TypeLayout {
    let descriptor = build_runtime_abi_plan(target).slice_descriptor();
    TypeLayout {
        size: descriptor.total_size(),
        alignment: descriptor.align(),
    }
}

pub(super) fn primitive_type_layout(
    target: NativeTarget,
    primitive_type: PrimitiveType,
) -> TypeLayout {
    match primitive_type {
        PrimitiveType::Bool => TypeLayout {
            size: 1,
            alignment: 1,
        },
        PrimitiveType::F32 | PrimitiveType::I32 | PrimitiveType::U32 => TypeLayout {
            size: 4,
            alignment: 4,
        },
        PrimitiveType::F64 | PrimitiveType::I64 | PrimitiveType::U64 => TypeLayout {
            size: 8,
            alignment: 8,
        },
        PrimitiveType::Usize => TypeLayout {
            size: target.pointer_size,
            alignment: target.pointer_alignment,
        },
        PrimitiveType::String => fat_descriptor_layout(target),
    }
}
