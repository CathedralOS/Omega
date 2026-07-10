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

/// The canonical `TypeLayout` of a scalar primitive, given the target's pointer
/// geometry and the fat-descriptor layout to use for `String`.
///
/// SINGLE SOURCE OF TRUTH for the primitive -> layout mapping shared by the
/// backend crates that resolve storage sizes (omega-layout, omega-instruction-
/// selection, omega-runtime-storage). Each caller supplies the pointer geometry
/// from its own ABI context and the `String` descriptor layout it derives from
/// `omega-runtime-abi` (a two-pointer, pointer-aligned fat `{ptr, len}`), so this
/// helper stays free of any ABI-plan dependency while collapsing the byte-width
/// match that previously lived, identically, in three crates.
pub fn primitive_layout(
    pointer_size: usize,
    pointer_alignment: usize,
    string_layout: TypeLayout,
    primitive_type: PrimitiveType,
) -> TypeLayout {
    match primitive_type {
        PrimitiveType::Bool | PrimitiveType::I8 | PrimitiveType::U8 => TypeLayout {
            size: 1,
            alignment: 1,
        },
        PrimitiveType::I16 | PrimitiveType::U16 => TypeLayout {
            size: 2,
            alignment: 2,
        },
        PrimitiveType::F32 | PrimitiveType::I32 | PrimitiveType::U32 => TypeLayout {
            size: 4,
            alignment: 4,
        },
        PrimitiveType::F64 | PrimitiveType::I64 | PrimitiveType::U64 => TypeLayout {
            size: 8,
            alignment: 8,
        },
        PrimitiveType::Addr => TypeLayout {
            size: pointer_size,
            alignment: pointer_alignment,
        },
        PrimitiveType::String => string_layout,
    }
}

pub(super) fn primitive_type_layout(
    target: NativeTarget,
    primitive_type: PrimitiveType,
) -> TypeLayout {
    primitive_layout(
        target.pointer_size,
        target.pointer_alignment,
        fat_descriptor_layout(target),
        primitive_type,
    )
}
