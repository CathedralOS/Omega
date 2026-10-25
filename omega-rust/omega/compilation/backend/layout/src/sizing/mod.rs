use crate::TypeLayout;
use target::NativeTarget;
use typed_trees::types::PrimitiveType;

/// Canonical fat-descriptor layout (`{ptr, len}`) for slices and text windows.
///
/// Single source of truth inside layout: derives size/alignment from
/// `runtime-abi` rather than re-deriving `2 * pointer_size` locally.
pub(super) fn fat_descriptor_layout(target: NativeTarget) -> TypeLayout {
    let descriptor = runtime_abi::fat_descriptor_layout(target);
    TypeLayout {
        size: descriptor.byte_size,
        alignment: descriptor.alignment,
    }
}

/// The canonical `TypeLayout` of one target pointer word.
///
/// Addresses, selectors, thin references and each word of a descriptor all
/// read the target's pointer geometry here instead of re-spelling
/// `{ target.pointer_size, target.pointer_alignment }` at the carrier arms.
pub(super) fn pointer_layout(target: NativeTarget) -> TypeLayout {
    TypeLayout {
        size: target.pointer_size,
        alignment: target.pointer_alignment,
    }
}

/// The canonical `TypeLayout` of a scalar primitive, given the target's pointer
/// geometry.
///
/// SINGLE SOURCE OF TRUTH for the primitive -> layout mapping shared by the
/// backend crates that resolve storage sizes (layout, omega-instruction-
/// selection, omega-runtime-storage). Each caller supplies the pointer geometry
/// from its own ABI context, so this helper stays free of any ABI-plan dependency
/// while collapsing the byte-width match that previously lived in three crates.
pub(crate) fn primitive_layout(
    pointer_size: usize,
    pointer_alignment: usize,
    primitive_type: PrimitiveType,
) -> TypeLayout {
    fixed_primitive_layout(primitive_type).unwrap_or(TypeLayout {
        size: pointer_size,
        alignment: pointer_alignment,
    })
}

/// `None` names precisely the primitives whose geometry is a target input.
fn fixed_primitive_layout(primitive_type: PrimitiveType) -> Option<TypeLayout> {
    Some(match primitive_type {
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
        PrimitiveType::Addr | PrimitiveType::Selector => return None,
    })
}

pub(super) fn primitive_type_layout(
    target: Option<NativeTarget>,
    primitive_type: PrimitiveType,
) -> Option<TypeLayout> {
    fixed_primitive_layout(primitive_type).or_else(|| target.map(pointer_layout))
}
