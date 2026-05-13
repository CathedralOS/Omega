use crate::TypeLayout;
use omega_checked_trees::types::PrimitiveType;
use omega_target::NativeTarget;

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
        PrimitiveType::F64 | PrimitiveType::U64 => TypeLayout {
            size: 8,
            alignment: 8,
        },
        PrimitiveType::Usize => TypeLayout {
            size: target.pointer_size,
            alignment: target.pointer_alignment,
        },
        PrimitiveType::String => TypeLayout {
            size: target.pointer_size * 2,
            alignment: target.pointer_alignment,
        },
    }
}
