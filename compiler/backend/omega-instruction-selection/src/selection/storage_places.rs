mod expressions;
mod machine_owned;
mod model;
mod nested_fields;
mod static_values;

pub(super) use expressions::indexed_expression_path;
pub(super) use machine_owned::{resolve_machine_owned_place, resolve_machine_owned_place_in_table};
pub(super) use model::{IndexedTargetPath, RuntimeFrameIndexedTarget, RuntimeStoragePlace};
use omega_target_operations::RuntimeStorageRegion;
pub(super) use static_values::{enum_variant_value, static_integer_value};

use crate::InstructionSelectionInput;
use expressions::{
    StorageNamePath, normalized_storage_expression, normalized_storage_name_path_in_table,
};
use nested_fields::resolve_nested_field_layout;
use omega_checked_trees::expression::{
    CallExpression, Expression, ExpressionHandle, ExpressionNode, ExpressionTable, NamePath,
};
use omega_checked_trees::name::ProgramName;
use omega_checked_trees::types::PrimitiveType;
use omega_control_flow::StateKey;
use omega_core::symbols::SymbolHandle;
use omega_layout::{FieldLayout, TypeLayout};
use omega_state_calls::StateCallRole;

pub(super) fn resolve_runtime_storage_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    _source_machine: &str,
    _source_state: &str,
    expression: &Expression,
) -> Option<RuntimeStoragePlace> {
    if let Some(place) =
        resolve_runtime_fixed_indexed_place(input, dispatch_index, source_key, expression)
    {
        return Some(place);
    }

    if let Some((byte_offset, byte_count)) = resolve_machine_owned_place(
        &input.layouts,
        input.entry_key.machine,
        source_key.machine,
        expression,
    ) {
        return Some(RuntimeStoragePlace {
            region: RuntimeStorageRegion::Machine,
            byte_offset,
            byte_count,
        });
    }

    let normalized_expression = normalized_storage_expression(expression)?;
    let Expression::Name(path) = &normalized_expression else {
        return None;
    };
    if path.is_empty() {
        return None;
    }
    let suffix = &path.members()[1..];
    let slot = input
        .runtime_storage
        .frame_slots
        .iter()
        .find(|(_, slot)| {
            slot.dispatch_index == dispatch_index
                && slot.source_key == source_key
                && slot_matches_path(slot.symbol, path, slot.name.as_str())
        })
        .or_else(|| {
            input.runtime_storage.frame_slots.iter().find(|(_, slot)| {
                slot.dispatch_index == dispatch_index
                    && slot_matches_path(slot.symbol, path, slot.name.as_str())
            })
        })
        .map(|(_, slot)| slot)?;
    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: slot.byte_offset,
        type_symbol: slot.type_symbol,
        type_name: slot.type_name.clone(),
        layout: TypeLayout {
            size: slot.byte_size,
            alignment: slot.alignment,
        },
    };
    let (byte_offset, layout) = resolve_nested_field_layout(&input.layouts, &root_field, suffix)?;

    Some(RuntimeStoragePlace {
        region: RuntimeStorageRegion::RuntimeFrame,
        byte_offset,
        byte_count: layout.size,
    })
}

pub(super) fn resolve_runtime_assignment_value_call_result_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    _expression: &CallExpression,
) -> Option<RuntimeStoragePlace> {
    resolve_runtime_call_result_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        StateCallRole::AssignmentValue,
        None,
    )
}

pub(super) fn resolve_runtime_transition_guard_call_result_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    _expression: &CallExpression,
) -> Option<RuntimeStoragePlace> {
    resolve_runtime_call_result_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        StateCallRole::TransitionGuard,
        None,
    )
    .or_else(|| {
        input
            .runtime_storage
            .transition_guard_result_slot(dispatch_index, source_key, statement_index)
            .map(|slot| RuntimeStoragePlace {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: slot.byte_offset,
                byte_count: slot.byte_size,
            })
    })
}

pub(super) fn resolve_runtime_transition_argument_call_result_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    _expression: &CallExpression,
) -> Option<RuntimeStoragePlace> {
    resolve_runtime_call_result_place(
        input,
        dispatch_index,
        source_key,
        statement_index,
        StateCallRole::TransitionArgument,
        None,
    )
}

fn resolve_runtime_call_result_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    _call_ordinal: Option<usize>,
) -> Option<RuntimeStoragePlace> {
    let slot = input.runtime_storage.call_result_slot(
        dispatch_index,
        source_key,
        statement_index,
        role,
    )?;
    let target_key = match slot.kind {
        omega_runtime_storage::RuntimeFrameSlotKind::StateCallResult { target_key, .. } => {
            target_key
        }
        _ => return None,
    };
    let state_call = input
        .state_calls
        .call_for_role(source_key, statement_index, role)?;
    if state_call.target_key != target_key {
        return None;
    }
    Some(RuntimeStoragePlace {
        region: RuntimeStorageRegion::RuntimeFrame,
        byte_offset: slot.byte_offset,
        byte_count: slot.byte_size,
    })
}

pub(super) fn resolve_runtime_storage_place_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeStoragePlace> {
    if let Some(place) = resolve_runtime_fixed_indexed_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) {
        return Some(place);
    }

    if let Some((byte_offset, byte_count)) = resolve_machine_owned_place_in_table(
        &input.layouts,
        input.entry_key.machine,
        source_key.machine,
        expressions,
        expression,
    ) {
        return Some(RuntimeStoragePlace {
            region: RuntimeStorageRegion::Machine,
            byte_offset,
            byte_count,
        });
    }

    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    if path.is_empty() {
        return None;
    }
    let suffix = &path.members()[1..];
    let slot = input
        .runtime_storage
        .frame_slots
        .iter()
        .find(|(_, slot)| {
            slot.dispatch_index == dispatch_index
                && slot.source_key == source_key
                && slot_matches_table_path(slot.symbol, &path, slot.name.as_str())
        })
        .or_else(|| {
            input.runtime_storage.frame_slots.iter().find(|(_, slot)| {
                slot.dispatch_index == dispatch_index
                    && slot_matches_table_path(slot.symbol, &path, slot.name.as_str())
            })
        })
        .map(|(_, slot)| slot)?;
    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: slot.byte_offset,
        type_symbol: slot.type_symbol,
        type_name: slot.type_name.clone(),
        layout: TypeLayout {
            size: slot.byte_size,
            alignment: slot.alignment,
        },
    };
    let (byte_offset, layout) = resolve_nested_field_layout(&input.layouts, &root_field, suffix)?;

    Some(RuntimeStoragePlace {
        region: RuntimeStorageRegion::RuntimeFrame,
        byte_offset,
        byte_count: layout.size,
    })
}

pub(super) fn resolve_runtime_frame_indexed_target(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimeFrameIndexedTarget> {
    let indexed = indexed_target_path(expression)?;
    let collection_slot =
        runtime_frame_slot_for_expression(input, dispatch_index, source_key, &indexed.collection)?;
    let descriptor_place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        "",
        "",
        &indexed.collection,
    )?;
    let index_place =
        resolve_runtime_storage_place(input, dispatch_index, source_key, "", "", &indexed.index)?;
    if descriptor_place.region != RuntimeStorageRegion::RuntimeFrame
        || index_place.region != RuntimeStorageRegion::RuntimeFrame
    {
        return None;
    }

    let element_layout = indexed_element_layout(input, collection_slot.type_symbol)?;
    let element_type_name = indexed_element_type_name(&collection_slot.type_name)?;
    let (field_byte_offset, field_layout) = if indexed.suffix.is_empty() {
        (0, element_layout)
    } else {
        let root_field = FieldLayout {
            symbol: collection_slot.symbol,
            name: collection_slot.name.clone(),
            offset: 0,
            type_symbol: collection_slot.type_symbol,
            type_name: element_type_name.to_owned(),
            layout: element_layout,
        };
        resolve_nested_field_layout(&input.layouts, &root_field, &indexed.suffix)?
    };

    Some(RuntimeFrameIndexedTarget {
        descriptor_offset: descriptor_place.byte_offset,
        index_offset: index_place.byte_offset,
        element_byte_size: element_layout.size,
        field_byte_offset,
        byte_count: field_layout.size,
    })
}

pub(super) fn resolve_runtime_pointee_slot_offset(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimePointeeTarget> {
    let target = match expression {
        Expression::Mutable(target) => target.as_ref(),
        _ => expression,
    };
    let normalized_expression = normalized_storage_expression(target)?;
    let Expression::Name(path) = &normalized_expression else {
        return None;
    };
    let [root_name, suffix @ ..] = path.members() else {
        return None;
    };
    let place = resolve_runtime_frame_root_place(
        input,
        dispatch_index,
        source_key,
        path.head_symbol(),
        root_name,
    )?;
    if place.region != RuntimeStorageRegion::RuntimeFrame
        || place.byte_count != input.target.pointer_size
    {
        return None;
    }

    let slot = runtime_frame_slot_for_expression(input, dispatch_index, source_key, target)?;
    let pointee_type_name = slot
        .type_name
        .strip_prefix("&mut ")
        .or_else(|| slot.type_name.strip_prefix('&'))?;
    let pointee_layout = pointee_type_layout(input, slot.type_symbol, pointee_type_name);
    let (field_byte_offset, field_layout) = if suffix.is_empty() {
        (0, pointee_layout)
    } else {
        let root_field = FieldLayout {
            symbol: slot.symbol,
            name: slot.name.clone(),
            offset: 0,
            type_symbol: slot.type_symbol,
            type_name: pointee_type_name.to_owned(),
            layout: pointee_layout,
        };
        resolve_nested_field_layout(&input.layouts, &root_field, suffix)?
    };
    (field_layout.size > 0).then_some(RuntimePointeeTarget {
        pointer_byte_offset: place.byte_offset,
        field_byte_offset,
        pointee_byte_size: field_layout.size,
    })
}

fn resolve_runtime_frame_root_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    root_symbol: SymbolHandle,
    root_name: &ProgramName,
) -> Option<RuntimeStoragePlace> {
    input
        .runtime_storage
        .frame_slots
        .iter()
        .find(|(_, slot)| {
            slot.dispatch_index == dispatch_index
                && slot.source_key == source_key
                && slot_matches_root(slot.symbol, root_symbol, root_name, slot.name.as_str())
        })
        .or_else(|| {
            input.runtime_storage.frame_slots.iter().find(|(_, slot)| {
                slot.dispatch_index == dispatch_index
                    && slot_matches_root(slot.symbol, root_symbol, root_name, slot.name.as_str())
            })
        })
        .map(|(_, slot)| RuntimeStoragePlace {
            region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset: slot.byte_offset,
            byte_count: slot.byte_size,
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimePointeeTarget {
    pub(super) pointer_byte_offset: usize,
    pub(super) field_byte_offset: usize,
    pub(super) pointee_byte_size: usize,
}

fn slot_matches_path(slot_symbol: SymbolHandle, path: &NamePath, slot_name: &str) -> bool {
    let Some(root_name) = path.first() else {
        return false;
    };

    slot_matches_root(slot_symbol, path.head_symbol(), root_name, slot_name)
}

fn slot_matches_table_path(
    slot_symbol: SymbolHandle,
    path: &StorageNamePath<'_>,
    slot_name: &str,
) -> bool {
    let Some(root_name) = path.first() else {
        return false;
    };

    slot_matches_root(slot_symbol, path.head_symbol(), root_name, slot_name)
}

fn slot_matches_root(
    slot_symbol: SymbolHandle,
    root_symbol: SymbolHandle,
    root_name: &ProgramName,
    slot_name: &str,
) -> bool {
    if slot_symbol.is_valid() && root_symbol.is_valid() {
        return slot_symbol == root_symbol;
    }

    root_name.as_str() == slot_name
}

fn runtime_frame_slot_for_expression<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<&'plan omega_runtime_storage::RuntimeFrameSlot> {
    let normalized_expression = normalized_storage_expression(expression)?;
    let Expression::Name(path) = &normalized_expression else {
        return None;
    };

    input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index
                && slot.source_key == source_key
                && slot_matches_path(slot.symbol, path, slot.name.as_str()))
            .then_some(slot)
        })
}

fn resolve_runtime_fixed_indexed_place(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimeStoragePlace> {
    let fixed = fixed_indexed_target_path(expression)?;
    let slot =
        runtime_frame_slot_for_expression(input, dispatch_index, source_key, &fixed.collection)?;
    let element_layout = indexed_element_layout(input, slot.type_symbol)?;
    let element_type_name = indexed_element_type_name(&slot.type_name)?;
    let index = usize::try_from(fixed.index).ok()?;
    let element_offset = index.checked_mul(element_layout.size)?;
    let (field_byte_offset, field_layout) = if fixed.suffix.is_empty() {
        (0, element_layout)
    } else {
        let root_field = FieldLayout {
            symbol: slot.symbol,
            name: slot.name.clone(),
            offset: 0,
            type_symbol: slot.type_symbol,
            type_name: element_type_name.to_owned(),
            layout: element_layout,
        };
        resolve_nested_field_layout(&input.layouts, &root_field, &fixed.suffix)?
    };

    Some(RuntimeStoragePlace {
        region: RuntimeStorageRegion::RuntimeFrame,
        byte_offset: slot
            .byte_offset
            .checked_add(element_offset)?
            .checked_add(field_byte_offset)?,
        byte_count: field_layout.size,
    })
}

fn resolve_runtime_fixed_indexed_place_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeStoragePlace> {
    let fixed = fixed_indexed_target_path_in_table(expressions, expression)?;
    let collection = expressions.to_tree(fixed.collection);
    let slot = runtime_frame_slot_for_expression(input, dispatch_index, source_key, &collection)?;
    let element_layout = indexed_element_layout(input, slot.type_symbol)?;
    let element_type_name = indexed_element_type_name(&slot.type_name)?;
    let index = usize::try_from(fixed.index).ok()?;
    let element_offset = index.checked_mul(element_layout.size)?;
    let (field_byte_offset, field_layout) = if fixed.suffix.is_empty() {
        (0, element_layout)
    } else {
        let root_field = FieldLayout {
            symbol: slot.symbol,
            name: slot.name.clone(),
            offset: 0,
            type_symbol: slot.type_symbol,
            type_name: element_type_name.to_owned(),
            layout: element_layout,
        };
        resolve_nested_field_layout(&input.layouts, &root_field, &fixed.suffix)?
    };

    Some(RuntimeStoragePlace {
        region: RuntimeStorageRegion::RuntimeFrame,
        byte_offset: slot
            .byte_offset
            .checked_add(element_offset)?
            .checked_add(field_byte_offset)?,
        byte_count: field_layout.size,
    })
}

#[derive(Debug, Clone)]
struct FixedIndexedTargetPath {
    collection: Expression,
    index: i64,
    suffix: Vec<ProgramName>,
}

#[derive(Debug, Clone)]
struct TableFixedIndexedTargetPath {
    collection: ExpressionHandle,
    index: i64,
    suffix: Vec<ProgramName>,
}

fn fixed_indexed_target_path(expression: &Expression) -> Option<FixedIndexedTargetPath> {
    match expression {
        Expression::Mutable(target) => fixed_indexed_target_path(target),
        Expression::Member(member) => {
            let mut path = fixed_indexed_target_path(&member.receiver)?;
            path.suffix.push(member.member.clone());
            Some(path)
        }
        Expression::Indexed(indexed) => {
            let Expression::Integer(index) = &indexed.index else {
                return None;
            };
            Some(FixedIndexedTargetPath {
                collection: indexed.collection.clone(),
                index: *index,
                suffix: Vec::new(),
            })
        }
        _ => None,
    }
}

fn fixed_indexed_target_path_in_table(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<TableFixedIndexedTargetPath> {
    match table.expression(expression) {
        ExpressionNode::Mutable(target) => fixed_indexed_target_path_in_table(table, *target),
        ExpressionNode::Member(member) => {
            let mut path = fixed_indexed_target_path_in_table(table, member.receiver)?;
            path.suffix.push(member.member.clone());
            Some(path)
        }
        ExpressionNode::Indexed(indexed) => {
            let ExpressionNode::Integer(index) = table.expression(indexed.index) else {
                return None;
            };
            Some(TableFixedIndexedTargetPath {
                collection: indexed.collection,
                index: *index,
                suffix: Vec::new(),
            })
        }
        _ => None,
    }
}

fn indexed_target_path(expression: &Expression) -> Option<IndexedTargetPath> {
    match expression {
        Expression::Mutable(target) => indexed_target_path(target),
        Expression::Member(member) => {
            let mut path = indexed_target_path(&member.receiver)?;
            path.suffix.push(member.member.clone());
            Some(path)
        }
        Expression::Indexed(indexed) => Some(IndexedTargetPath {
            collection: indexed.collection.clone(),
            index: indexed.index.clone(),
            suffix: Vec::new(),
        }),
        _ => None,
    }
}

fn indexed_element_layout(
    input: &InstructionSelectionInput<'_>,
    type_symbol: SymbolHandle,
) -> Option<TypeLayout> {
    if type_symbol.is_valid() {
        if let Some(layout) = input
            .layouts
            .data_layouts
            .iter()
            .find(|(_, layout)| layout.symbol == type_symbol)
            .map(|(_, layout)| layout.layout)
        {
            return Some(layout);
        }

        if let Some(layout) = input
            .layouts
            .machine_layouts
            .iter()
            .find(|(_, layout)| layout.symbol == type_symbol)
            .map(|(_, layout)| layout.layout)
        {
            return Some(layout);
        }
    }

    let _ = PrimitiveType::Bool;
    None
}

fn indexed_element_type_name(type_name: &str) -> Option<&str> {
    type_name
        .strip_prefix('[')
        .and_then(|inner| inner.split_once(';').map(|(element, _)| element.trim()))
        .or_else(|| {
            type_name
                .strip_prefix("&mut [")
                .and_then(|inner| inner.strip_suffix(']'))
        })
        .or_else(|| {
            type_name
                .strip_prefix("&[")
                .and_then(|inner| inner.strip_suffix(']'))
        })
        .or_else(|| {
            type_name.strip_prefix("&mut [").and_then(|inner| {
                inner
                    .split_once(';')
                    .map(|(element, _)| element.trim_end_matches(']').trim())
            })
        })
        .or_else(|| {
            type_name.strip_prefix("&[").and_then(|inner| {
                inner
                    .split_once(';')
                    .map(|(element, _)| element.trim_end_matches(']').trim())
            })
        })
}

fn pointee_type_layout(
    input: &InstructionSelectionInput<'_>,
    type_symbol: SymbolHandle,
    type_name: &str,
) -> TypeLayout {
    if type_symbol.is_valid() {
        if let Some(layout) = input
            .layouts
            .data_layouts
            .iter()
            .find(|(_, layout)| layout.symbol == type_symbol)
            .map(|(_, layout)| layout.layout)
        {
            return layout;
        }

        if let Some(layout) = input
            .layouts
            .machine_layouts
            .iter()
            .find(|(_, layout)| layout.symbol == type_symbol)
            .map(|(_, layout)| layout.layout)
        {
            return layout;
        }
    }

    if let Some(primitive_type) = PrimitiveType::from_name(type_name) {
        return match primitive_type {
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
                size: input.target.pointer_size,
                alignment: input.target.pointer_alignment,
            },
            PrimitiveType::String => TypeLayout {
                size: input.target.pointer_size * 2,
                alignment: input.target.pointer_alignment,
            },
        };
    }

    if type_name == "Option" || type_name.starts_with("Option<") {
        return TypeLayout {
            size: input.target.pointer_size * 2,
            alignment: input.target.pointer_alignment,
        };
    }

    if type_name == "Uint" {
        return TypeLayout {
            size: input.target.pointer_size,
            alignment: input.target.pointer_alignment,
        };
    }

    if type_name == "Real" {
        return TypeLayout {
            size: 8,
            alignment: 8,
        };
    }

    TypeLayout::default()
}
