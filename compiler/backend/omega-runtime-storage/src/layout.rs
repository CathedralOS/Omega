use super::RuntimeStorageContext;
use omega_core::symbols::SymbolHandle;
use omega_layout::TypeLayout;
use omega_typed_trees::types::PrimitiveType;

pub(super) fn layout_for_type(
    context: &RuntimeStorageContext,
    type_symbol: SymbolHandle,
    type_name: &str,
) -> TypeLayout {
    if type_name.starts_with('&') {
        return TypeLayout {
            size: context.target.pointer_size,
            alignment: context.target.pointer_alignment,
        };
    }

    if type_symbol.is_valid() {
        if let Some(data_layout) = context
            .layouts
            .data_layouts
            .iter()
            .find(|(_, data_layout)| data_layout.symbol == type_symbol)
            .map(|(_, data_layout)| data_layout.layout)
        {
            return data_layout;
        }

        if let Some(machine_layout) = context
            .layouts
            .machine_layouts
            .iter()
            .find(|(_, machine_layout)| machine_layout.symbol == type_symbol)
            .map(|(_, machine_layout)| machine_layout.layout)
        {
            return machine_layout;
        }
    }

    if let Some(primitive_type) = PrimitiveType::from_name(type_name) {
        return primitive_layout(context, primitive_type);
    }

    if let Some(layout) = builtin_named_layout(context, type_name) {
        return layout;
    }

    if is_slice_descriptor_name(type_name) {
        return TypeLayout {
            size: context.target.pointer_size * 2,
            alignment: context.target.pointer_alignment,
        };
    }

    TypeLayout::default()
}

fn is_slice_descriptor_name(type_name: &str) -> bool {
    type_name.starts_with('[') && type_name.ends_with(']') && !type_name.contains(';')
}

fn builtin_named_layout(context: &RuntimeStorageContext, type_name: &str) -> Option<TypeLayout> {
    match type_name {
        "Uint" => Some(TypeLayout {
            size: context.target.pointer_size,
            alignment: context.target.pointer_alignment,
        }),
        "Real" => Some(TypeLayout {
            size: 8,
            alignment: 8,
        }),
        _ => None,
    }
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
