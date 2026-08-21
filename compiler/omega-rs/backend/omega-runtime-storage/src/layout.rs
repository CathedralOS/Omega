use super::RuntimeStorageContext;
use omega_layout::TypeLayout;
use psi_arena::HandleSpan;
use psi_checked_trees::types::{
    FixedArrayLength, PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
    TypeReferenceTable,
};
use psi_symbols::{BuiltinType, SymbolHandle};

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
    let TypeReferenceNode::Reference { referee, .. } = table.type_reference(type_reference) else {
        return None;
    };
    let mut referee = *referee;
    loop {
        match table.type_reference(referee) {
            TypeReferenceNode::Constrained { base_type, .. } => referee = *base_type,
            TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Named { .. } => {
                let layout = layout_for_type_reference(context, table, referee);
                return (layout.size > 0).then_some(layout);
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
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let base = layout_for_type_reference(context, table, *base_type);
            if bounded_byte_buffer_shape(table, *base_type, *constraints).is_some() {
                TypeLayout {
                    size: context.target.pointer_size.saturating_add(base.size),
                    alignment: context.target.pointer_alignment,
                }
            } else {
                base
            }
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
        TypeReferenceNode::Slice { .. } => slice_layout(context),
        TypeReferenceNode::DynamicTrait { .. } => dynamic_trait_layout(context),
        TypeReferenceNode::Generic {
            base_symbol: symbol,
            base_name: name,
            ..
        }
        | TypeReferenceNode::Named { symbol, name } => {
            layout_for_named_type(context, *symbol, name.as_str())
        }
        TypeReferenceNode::ConstExpression(_) | TypeReferenceNode::Unit => TypeLayout::default(),
    }
}

/// Recognize the exact semantic carrier shape used by `omega-layout` for an
/// owned fixed array qualified by a named non-layout domain. Runtime-frame
/// planning must not peel this constraint: the carrier is `{len, bytes}`, not
/// the plain always-full array.
pub(super) fn bounded_byte_buffer_shape(
    table: &TypeReferenceTable,
    base_type: TypeReferenceHandle,
    constraints: HandleSpan<TypeConstraintNode>,
) -> Option<(TypeReferenceHandle, usize)> {
    let has_named_domain = table.constraints(constraints).iter().any(|constraint| {
        matches!(
            constraint,
            TypeConstraintNode::Domain(name)
                if !psi_checked_trees::wire::is_layout_domain_constraint(name)
                    && psi_language_semantics::CarryPermission::from_name(name.as_str()).is_none()
        )
    });
    if !has_named_domain {
        return None;
    }
    let TypeReferenceNode::FixedArray {
        element_type,
        length: FixedArrayLength::Literal(capacity),
    } = table.type_reference(base_type)
    else {
        return None;
    };
    Some((*element_type, *capacity))
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
        TypeReferenceNode::Slice { .. } => slice_layout(context),
        TypeReferenceNode::DynamicTrait { .. } => dynamic_trait_layout(context),
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

fn dynamic_trait_layout(context: &RuntimeStorageContext) -> TypeLayout {
    let descriptor =
        omega_runtime_abi::build_runtime_abi_plan(context.target).dynamic_trait_descriptor();
    TypeLayout {
        size: descriptor.total_size(),
        alignment: descriptor.align(),
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

    None
}

/// Thin wrapper over the shared `omega_layout::primitive_layout`; supplies this
/// crate's pointer geometry and delegates the byte-width match.
fn primitive_layout(context: &RuntimeStorageContext, primitive_type: PrimitiveType) -> TypeLayout {
    omega_layout::primitive_layout(
        context.target.pointer_size,
        context.target.pointer_alignment,
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
