use super::RuntimeStorageContext;
use omega_layout::TypeLayout;
use omega_typed_program::types::PrimitiveType;

pub(super) fn layout_for_type_name(context: &RuntimeStorageContext, type_name: &str) -> TypeLayout {
    if let Some(data_layout) = context
        .layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.name == type_name)
        .map(|(_, data_layout)| data_layout.layout)
    {
        return data_layout;
    }

    if let Some(machine_layout) = context
        .layouts
        .machine_layouts
        .iter()
        .find(|(_, machine_layout)| machine_layout.name == type_name)
        .map(|(_, machine_layout)| machine_layout.layout)
    {
        return machine_layout;
    }

    if let Some(primitive_type) = PrimitiveType::from_name(type_name) {
        return primitive_layout(context, primitive_type);
    }

    TypeLayout::default()
}

fn primitive_layout(context: &RuntimeStorageContext, primitive_type: PrimitiveType) -> TypeLayout {
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
            size: context.target.pointer_size,
            alignment: context.target.pointer_alignment,
        },
        PrimitiveType::String => TypeLayout {
            size: context.target.pointer_size * 2,
            alignment: context.target.pointer_alignment,
        },
    }
}

pub(super) fn align_to(offset: usize, alignment: usize) -> usize {
    let alignment = alignment.max(1);
    let remainder = offset % alignment;
    if remainder == 0 {
        offset
    } else {
        offset + alignment - remainder
    }
}
