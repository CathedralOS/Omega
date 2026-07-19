use super::RuntimeStorageContext;
use omega_checked_trees::types::{
    FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode, TypeReferenceTable,
};
use omega_core::symbols::{BuiltinType, SymbolHandle};
use omega_layout::TypeLayout;

/// Rung C2's record view: a reference-typed local whose initializer is a
/// judged RECAST over a byte region reserves the REFEREE RECORD's width. For
/// wide records the slot's first word carries the backing address; the wider
/// reservation is the read-side discriminator between this view and an
/// ordinary pointer slot. Every other reference local keeps the pointer model
/// -- gating on the recast initializer keeps boundary pointer locals intact.
pub(super) fn recast_view_layout(
    context: &RuntimeStorageContext,
    table: &TypeReferenceTable,
    type_reference: TypeReferenceHandle,
) -> Option<TypeLayout> {
    let TypeReferenceNode::Reference { referee, .. } = table.type_reference(type_reference)
    else {
        return None;
    };
    let mut referee = *referee;
    loop {
        match table.type_reference(referee) {
            TypeReferenceNode::Constrained { base_type, .. } => referee = *base_type,
            TypeReferenceNode::Named { symbol, name } => {
                let record = layout_for_named_type(context, *symbol, name.as_str());
                return (record.size > 0).then_some(record);
            }
            _ => return None,
        }
    }
}

pub(super) fn layout_for_type_reference(
    context: &RuntimeStorageContext,
    table: &TypeReferenceTable,
    type_reference: TypeReferenceHandle,
) -> TypeLayout {
    match table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            layout_for_reference_type(context, table, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            layout_for_type_reference(context, table, *base_type)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            let element = layout_for_type_reference(context, table, *element_type);
            let FixedArrayLength::Literal(length) = length else {
                return TypeLayout::default();
            };
            TypeLayout {
                size: element.size.saturating_mul(*length),
                alignment: element.alignment,
            }
        }
        TypeReferenceNode::Slice { .. } | TypeReferenceNode::DynamicTrait { .. } => {
            slice_layout(context)
        }
        TypeReferenceNode::Generic {
            base_symbol: symbol,
            base_name: name,
            ..
        }
        | TypeReferenceNode::Named { symbol, name } => {
            layout_for_named_type(context, *symbol, name.as_str())
        }
        TypeReferenceNode::Unit => TypeLayout::default(),
    }
}

fn layout_for_reference_type(
    context: &RuntimeStorageContext,
    table: &TypeReferenceTable,
    referee: TypeReferenceHandle,
) -> TypeLayout {
    match table.type_reference(referee) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            layout_for_reference_type(context, table, *base_type)
        }
        TypeReferenceNode::Slice { .. } | TypeReferenceNode::DynamicTrait { .. } => {
            slice_layout(context)
        }
        _ => pointer_layout(context),
    }
}

fn layout_for_named_type(
    context: &RuntimeStorageContext,
    type_symbol: SymbolHandle,
    type_name: &str,
) -> TypeLayout {
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

    if let Some(layout) = builtin_type_layout(context, type_symbol) {
        return layout;
    }

    TypeLayout::default()
}

fn pointer_layout(context: &RuntimeStorageContext) -> TypeLayout {
    TypeLayout {
        size: context.target.pointer_size,
        alignment: context.target.pointer_alignment,
    }
}

fn slice_layout(context: &RuntimeStorageContext) -> TypeLayout {
    TypeLayout {
        size: context.target.pointer_size * 2,
        alignment: context.target.pointer_alignment,
    }
}

fn builtin_type_layout(
    context: &RuntimeStorageContext,
    type_symbol: SymbolHandle,
) -> Option<TypeLayout> {
    if Some(type_symbol)
        == context
            .program
            .symbols
            .builtin_type_symbol(BuiltinType::UInt)
    {
        return Some(TypeLayout {
            size: context.target.pointer_size,
            alignment: context.target.pointer_alignment,
        });
    }

    if Some(type_symbol)
        == context
            .program
            .symbols
            .builtin_type_symbol(BuiltinType::Int)
    {
        return Some(TypeLayout {
            size: context.target.pointer_size,
            alignment: context.target.pointer_alignment,
        });
    }

    if Some(type_symbol)
        == context
            .program
            .symbols
            .builtin_type_symbol(BuiltinType::Real)
    {
        return Some(TypeLayout {
            size: 8,
            alignment: 8,
        });
    }

    None
}

/// Thin wrapper over the shared `omega_layout::primitive_layout` -- supplies this
/// crate's pointer geometry and the fat `{ptr, len}` String descriptor (two
/// pointers, pointer-aligned) and delegates, so the byte-width match is
/// single-sourced in omega-layout.
fn primitive_layout(context: &RuntimeStorageContext, primitive_type: PrimitiveType) -> TypeLayout {
    omega_layout::primitive_layout(
        context.target.pointer_size,
        context.target.pointer_alignment,
        TypeLayout {
            size: context.target.pointer_size * 2,
            alignment: context.target.pointer_alignment,
        },
        primitive_type,
    )
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
