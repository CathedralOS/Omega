//! Source primitive geometry over the backend's pointer and descriptor words.

use crate::data_layout_queries::{TypeLayout, type_layout};
use checked_trees::typed_trees::types::PrimitiveType;
pub(crate) use layout::{fat_descriptor_layout, pointer_layout};
use target::NativeTarget;

/// The canonical `TypeLayout` of a scalar primitive, given the target's pointer
/// geometry.
fn primitive_layout(
    pointer_size: usize,
    pointer_alignment: usize,
    primitive_type: PrimitiveType,
) -> TypeLayout {
    match primitive_type {
        PrimitiveType::Bool | PrimitiveType::I8 | PrimitiveType::U8 => type_layout(1, 1),
        PrimitiveType::I16 | PrimitiveType::U16 => type_layout(2, 2),
        PrimitiveType::F32 | PrimitiveType::I32 | PrimitiveType::U32 => type_layout(4, 4),
        PrimitiveType::F64 | PrimitiveType::I64 | PrimitiveType::U64 => type_layout(8, 8),
        PrimitiveType::Addr | PrimitiveType::Selector => {
            type_layout(pointer_size, pointer_alignment)
        }
    }
}

pub(super) fn primitive_type_layout(
    target: NativeTarget,
    primitive_type: PrimitiveType,
) -> TypeLayout {
    primitive_layout(
        target.pointer_size,
        target.pointer_alignment,
        primitive_type,
    )
}
