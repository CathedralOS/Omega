mod expressions;
mod machine_owned;
mod model;
mod nested_fields;
mod static_values;

pub(super) use expressions::indexed_expression_path;
pub(super) use machine_owned::{
    resolve_machine_owned_collection_in_table, resolve_machine_owned_place,
    resolve_machine_owned_place_in_table,
};
pub(super) use model::{
    RuntimeFrameFixedIndexedTarget, RuntimeFrameIndexedTarget, RuntimeStoragePlace,
};
use omega_abstract_operations::RuntimeStorageRegion;
pub(super) use static_values::{
    enum_variant_value, enum_variant_value_in_table, static_integer_value,
    static_integer_value_in_table,
};

use crate::InstructionSelectionInput;
use expressions::{
    StorageNamePath, normalized_storage_expression, normalized_storage_name_path_in_table,
};
use nested_fields::{
    NestedFieldLayoutCursor, resolve_nested_field_layout_step,
    resolve_nested_field_layout_with_pairs, resolve_nested_field_layout_with_symbols,
};
use omega_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable, NamePath,
};
use omega_checked_trees::types::PrimitiveType;
use omega_control_flow::StateKey;
use omega_core::symbols::{BuiltinType, SymbolHandle};
use omega_layout::{FieldLayout, TypeLayout, TypeLayoutDescriptor};
use omega_state_calls::StateCallRole;

fn state_key_matches_statement_source(expected: StateKey, actual: StateKey) -> bool {
    expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
}

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
    let slot = find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
        slot_matches_path(slot, path)
    })
    .or_else(|| {
        input
            .runtime_storage
            .frame_slots
            .iter()
            .find_map(|(_, slot)| {
                (slot.dispatch_index == dispatch_index && slot_matches_path(slot, path))
                    .then_some(slot)
            })
    })?;
    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: slot.byte_offset,
        type_symbol: slot.type_symbol,
        type_name: slot.type_name.clone(),
        type_descriptor: slot.type_descriptor.clone(),
        layout: TypeLayout {
            size: slot.byte_size,
            alignment: slot.alignment,
        },
    };
    let (byte_offset, layout) =
        resolve_nested_field_layout_with_symbols(&input.layouts, &root_field, suffix, |index| {
            path.member_symbol(index + 1)
        })?;

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
    let suffix = path.suffix(1);
    let slot = find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
        slot_matches_table_path(slot, &path)
    })
    .or_else(|| {
        input
            .runtime_storage
            .frame_slots
            .iter()
            .find_map(|(_, slot)| {
                (slot.dispatch_index == dispatch_index && slot_matches_table_path(slot, &path))
                    .then_some(slot)
            })
    })?;
    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: slot.byte_offset,
        type_symbol: slot.type_symbol,
        type_name: slot.type_name.clone(),
        type_descriptor: slot.type_descriptor.clone(),
        layout: TypeLayout {
            size: slot.byte_size,
            alignment: slot.alignment,
        },
    };
    let (byte_offset, layout) =
        resolve_nested_field_layout_with_pairs(&input.layouts, &root_field, suffix.iter())?;

    Some(RuntimeStoragePlace {
        region: RuntimeStorageRegion::RuntimeFrame,
        byte_offset,
        byte_count: layout.size,
    })
}

pub(super) fn resolve_fixed_array_length_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<usize> {
    if let Some(slot) = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    ) && let Some((_, length)) = slot.type_descriptor.fixed_array()
    {
        return Some(length);
    }

    let collection = resolve_machine_owned_collection_in_table(
        &input.layouts,
        input.entry_key.machine,
        source_key.machine,
        expressions,
        expression,
    )?;
    let (_, length) = collection.type_descriptor.fixed_array()?;
    Some(length)
}

pub(super) fn resolve_runtime_frame_indexed_target(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expression: &Expression,
) -> Option<RuntimeFrameIndexedTarget> {
    let indexed = indexed_target_path(expression)?;
    let collection_slot =
        runtime_frame_slot_for_expression(input, dispatch_index, source_key, indexed.collection)?;
    let descriptor_place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        source_key,
        "",
        "",
        indexed.collection,
    )?;
    let index_place =
        resolve_runtime_storage_place(input, dispatch_index, source_key, "", "", indexed.index)?;
    if descriptor_place.region != RuntimeStorageRegion::RuntimeFrame
        || index_place.region != RuntimeStorageRegion::RuntimeFrame
    {
        return None;
    }

    let element_descriptor = collection_slot.type_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let root_field = FieldLayout {
        symbol: collection_slot.symbol,
        name: collection_slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout) =
        resolve_indexed_target_suffix_layout(input, &root_field, indexed.suffix_root)?;

    Some(RuntimeFrameIndexedTarget {
        descriptor_offset: descriptor_place.byte_offset,
        index_offset: index_place.byte_offset,
        element_byte_size: element_layout.size,
        field_byte_offset,
        byte_count: field_layout.size,
    })
}

pub(super) fn resolve_runtime_frame_indexed_target_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeFrameIndexedTarget> {
    let indexed = indexed_target_path_in_table(expressions, expression)?;
    let collection_slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )?;
    let descriptor_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.collection,
    )?;
    let index_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.index,
    )?;
    if descriptor_place.region != RuntimeStorageRegion::RuntimeFrame
        || index_place.region != RuntimeStorageRegion::RuntimeFrame
    {
        return None;
    }

    let element_descriptor = collection_slot.type_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let root_field = FieldLayout {
        symbol: collection_slot.symbol,
        name: collection_slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout) = resolve_indexed_target_suffix_layout_in_table(
        input,
        &root_field,
        expressions,
        indexed.suffix_root,
    )?;

    Some(RuntimeFrameIndexedTarget {
        descriptor_offset: descriptor_place.byte_offset,
        index_offset: index_place.byte_offset,
        element_byte_size: element_layout.size,
        field_byte_offset,
        byte_count: field_layout.size,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeMachineIndexedTarget {
    pub(super) base_byte_offset: usize,
    pub(super) index_offset: usize,
    pub(super) element_byte_size: usize,
    pub(super) field_byte_offset: usize,
    pub(super) byte_count: usize,
}

pub(super) fn resolve_runtime_machine_indexed_target_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeMachineIndexedTarget> {
    let indexed = indexed_target_path_in_table(expressions, expression)?;
    let collection = resolve_machine_owned_collection_in_table(
        &input.layouts,
        input.entry_key.machine,
        source_key.machine,
        expressions,
        indexed.collection,
    )?;
    let index_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        indexed.index,
    )?;
    if index_place.region != RuntimeStorageRegion::RuntimeFrame {
        return None;
    }

    let element_descriptor = collection.type_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let root_field = FieldLayout {
        symbol: SymbolHandle::invalid(),
        name: "".into(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout) = resolve_indexed_target_suffix_layout_in_table(
        input,
        &root_field,
        expressions,
        indexed.suffix_root,
    )?;

    Some(RuntimeMachineIndexedTarget {
        base_byte_offset: collection.byte_offset,
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
    let [_root_name, suffix @ ..] = path.members() else {
        return None;
    };
    let slot = runtime_frame_slot_for_expression(input, dispatch_index, source_key, target)?;
    if slot.byte_size != input.runtime_abi.pointer_size {
        return None;
    }
    let pointee_descriptor = slot.type_descriptor.reference_referee()?;
    let pointee_layout = descriptor_layout(input, pointee_descriptor);
    let (field_byte_offset, field_layout) = if suffix.is_empty() {
        (0, pointee_layout)
    } else {
        let root_field = FieldLayout {
            symbol: slot.symbol,
            name: slot.name.clone(),
            offset: 0,
            type_symbol: pointee_descriptor.storage_symbol(),
            type_name: "".into(),
            type_descriptor: pointee_descriptor.clone(),
            layout: pointee_layout,
        };
        resolve_nested_field_layout_with_symbols(&input.layouts, &root_field, suffix, |index| {
            path.member_symbol(index + 1)
        })?
    };
    (field_layout.size > 0).then_some(RuntimePointeeTarget {
        pointer_byte_offset: slot.byte_offset,
        field_byte_offset,
        pointee_byte_size: field_layout.size,
    })
}

pub(super) fn resolve_runtime_pointee_slot_offset_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimePointeeTarget> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;
    if path.is_empty() {
        return None;
    }
    let slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        expression,
    )?;
    if slot.byte_size != input.runtime_abi.pointer_size {
        return None;
    }
    let pointee_descriptor = slot.type_descriptor.reference_referee()?;
    let pointee_layout = descriptor_layout(input, pointee_descriptor);
    let suffix = path.suffix(1);
    let (field_byte_offset, field_layout) = if path.len() <= 1 {
        (0, pointee_layout)
    } else {
        let root_field = FieldLayout {
            symbol: slot.symbol,
            name: slot.name.clone(),
            offset: 0,
            type_symbol: pointee_descriptor.storage_symbol(),
            type_name: "".into(),
            type_descriptor: pointee_descriptor.clone(),
            layout: pointee_layout,
        };
        resolve_nested_field_layout_with_pairs(&input.layouts, &root_field, suffix.iter())?
    };
    (field_layout.size > 0).then_some(RuntimePointeeTarget {
        pointer_byte_offset: slot.byte_offset,
        field_byte_offset,
        pointee_byte_size: field_layout.size,
    })
}

pub(super) fn resolve_runtime_pointee_fixed_indexed_target_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimePointeeTarget> {
    let fixed = fixed_indexed_target_path_in_table(expressions, expression)?;
    let place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        fixed.collection,
    )?;
    if place.region != RuntimeStorageRegion::RuntimeFrame
        || place.byte_count != input.runtime_abi.pointer_size
    {
        return None;
    }

    let slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        fixed.collection,
    )?;
    let collection_descriptor = slot.type_descriptor.reference_referee()?;
    let element_descriptor = collection_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let element_index = usize::try_from(fixed.index).ok()?;
    let element_offset = element_index.checked_mul(element_layout.size)?;
    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout) = resolve_indexed_target_suffix_layout_in_table(
        input,
        &root_field,
        expressions,
        fixed.suffix_root,
    )?;

    (field_layout.size > 0).then_some(RuntimePointeeTarget {
        pointer_byte_offset: place.byte_offset,
        field_byte_offset: element_offset.checked_add(field_byte_offset)?,
        pointee_byte_size: field_layout.size,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimePointeeTarget {
    pub(super) pointer_byte_offset: usize,
    pub(super) field_byte_offset: usize,
    pub(super) pointee_byte_size: usize,
}

fn slot_matches_path(slot: &omega_runtime_storage::RuntimeFrameSlot, path: &NamePath) -> bool {
    slot_matches_root(slot.symbol, path.head_symbol())
        || path.first().is_some_and(|name| *name == slot.name)
}

fn slot_matches_table_path(
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    path: &StorageNamePath<'_>,
) -> bool {
    slot_matches_root(slot.symbol, path.head_symbol())
        || path.member(0).is_some_and(|name| *name == slot.name)
}

fn slot_matches_root(slot_symbol: SymbolHandle, root_symbol: SymbolHandle) -> bool {
    slot_symbol.is_valid() && root_symbol.is_valid() && slot_symbol == root_symbol
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

    find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
        slot_matches_path(slot, path)
    })
}

fn runtime_frame_slot_for_expression_in_table<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<&'plan omega_runtime_storage::RuntimeFrameSlot> {
    let path = normalized_storage_name_path_in_table(expressions, expression)?;

    find_runtime_frame_slot_for_path(input, dispatch_index, source_key, |slot| {
        slot_matches_table_path(slot, &path)
    })
}

fn find_runtime_frame_slot_for_path<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    dispatch_index: u32,
    source_key: StateKey,
    matches_path: impl Fn(&omega_runtime_storage::RuntimeFrameSlot) -> bool,
) -> Option<&'plan omega_runtime_storage::RuntimeFrameSlot> {
    input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index
                && state_key_matches_statement_source(slot.source_key, source_key)
                && matches_path(slot))
            .then_some(slot)
        })
        .or_else(|| {
            input
                .runtime_storage
                .frame_slots
                .iter()
                .find_map(|(_, slot)| {
                    (state_key_matches_statement_source(slot.source_key, source_key)
                        && matches_path(slot))
                    .then_some(slot)
                })
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
        runtime_frame_slot_for_expression(input, dispatch_index, source_key, fixed.collection)?;
    let element_descriptor = inline_fixed_array_element_type(&slot.type_descriptor)?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let index = usize::try_from(fixed.index).ok()?;
    let element_offset = index.checked_mul(element_layout.size)?;
    let root_field = FieldLayout {
        symbol: slot.symbol,
        name: slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout) =
        resolve_indexed_target_suffix_layout(input, &root_field, fixed.suffix_root)?;

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
    let index = usize::try_from(fixed.index).ok()?;
    if let Some(slot) = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        fixed.collection,
    ) {
        let element_descriptor = inline_fixed_array_element_type(&slot.type_descriptor)?;
        let element_layout = descriptor_layout(input, element_descriptor);
        let element_offset = index.checked_mul(element_layout.size)?;
        let root_field = FieldLayout {
            symbol: slot.symbol,
            name: slot.name.clone(),
            offset: 0,
            type_symbol: element_descriptor.storage_symbol(),
            type_name: "".into(),
            type_descriptor: element_descriptor.clone(),
            layout: element_layout,
        };
        let (field_byte_offset, field_layout) = resolve_indexed_target_suffix_layout_in_table(
            input,
            &root_field,
            expressions,
            fixed.suffix_root,
        )?;

        return Some(RuntimeStoragePlace {
            region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset: slot
                .byte_offset
                .checked_add(element_offset)?
                .checked_add(field_byte_offset)?,
            byte_count: field_layout.size,
        });
    }

    let collection = resolve_machine_owned_collection_in_table(
        &input.layouts,
        input.entry_key.machine,
        source_key.machine,
        expressions,
        fixed.collection,
    )?;
    let element_descriptor = inline_fixed_array_element_type(&collection.type_descriptor)?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let element_offset = index.checked_mul(element_layout.size)?;
    let root_field = FieldLayout {
        symbol: SymbolHandle::invalid(),
        name: "".into(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout) = resolve_indexed_target_suffix_layout_in_table(
        input,
        &root_field,
        expressions,
        fixed.suffix_root,
    )?;

    Some(RuntimeStoragePlace {
        region: RuntimeStorageRegion::Machine,
        byte_offset: collection
            .byte_offset
            .checked_add(element_offset)?
            .checked_add(field_byte_offset)?,
        byte_count: field_layout.size,
    })
}

pub(super) fn resolve_runtime_frame_fixed_indexed_target_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<RuntimeFrameFixedIndexedTarget> {
    let fixed = fixed_indexed_target_path_in_table(expressions, expression)?;
    let collection_slot = runtime_frame_slot_for_expression_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        fixed.collection,
    )?;
    if inline_fixed_array_element_type(&collection_slot.type_descriptor).is_some() {
        return None;
    }
    let descriptor_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        fixed.collection,
    )?;
    if descriptor_place.region != RuntimeStorageRegion::RuntimeFrame {
        return None;
    }

    let element_descriptor = collection_slot.type_descriptor.element_type()?;
    let element_layout = descriptor_layout(input, element_descriptor);
    let element_index = usize::try_from(fixed.index).ok()?;
    let root_field = FieldLayout {
        symbol: collection_slot.symbol,
        name: collection_slot.name.clone(),
        offset: 0,
        type_symbol: element_descriptor.storage_symbol(),
        type_name: "".into(),
        type_descriptor: element_descriptor.clone(),
        layout: element_layout,
    };
    let (field_byte_offset, field_layout) = resolve_indexed_target_suffix_layout_in_table(
        input,
        &root_field,
        expressions,
        fixed.suffix_root,
    )?;

    Some(RuntimeFrameFixedIndexedTarget {
        descriptor_offset: descriptor_place.byte_offset,
        element_index,
        element_byte_size: element_layout.size,
        field_byte_offset,
        byte_count: field_layout.size,
    })
}

#[derive(Debug, Clone, Copy)]
struct FixedIndexedTargetPath<'expression> {
    collection: &'expression Expression,
    index: i64,
    suffix_root: &'expression Expression,
}

#[derive(Debug, Clone, Copy)]
struct TableFixedIndexedTargetPath {
    collection: ExpressionHandle,
    index: i64,
    suffix_root: ExpressionHandle,
}

#[derive(Debug, Clone, Copy)]
struct IndexedTargetPath<'expression> {
    collection: &'expression Expression,
    index: &'expression Expression,
    suffix_root: &'expression Expression,
}

#[derive(Debug, Clone, Copy)]
struct TableIndexedTargetPath {
    collection: ExpressionHandle,
    index: ExpressionHandle,
    suffix_root: ExpressionHandle,
}

fn fixed_indexed_target_path(expression: &Expression) -> Option<FixedIndexedTargetPath<'_>> {
    match expression {
        Expression::Mutable(target) => fixed_indexed_target_path(target),
        Expression::Member(member) => {
            let path = fixed_indexed_target_path(&member.receiver)?;
            Some(FixedIndexedTargetPath {
                collection: path.collection,
                index: path.index,
                suffix_root: expression,
            })
        }
        Expression::Indexed(indexed) => {
            let Expression::Integer(index) = &indexed.index else {
                return None;
            };
            Some(FixedIndexedTargetPath {
                collection: &indexed.collection,
                index: *index,
                suffix_root: expression,
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
            let path = fixed_indexed_target_path_in_table(table, member.receiver)?;
            Some(TableFixedIndexedTargetPath {
                collection: path.collection,
                index: path.index,
                suffix_root: expression,
            })
        }
        ExpressionNode::Indexed(indexed) => {
            if let Some(path) = fixed_indexed_target_path_in_table(table, indexed.collection) {
                return Some(TableFixedIndexedTargetPath {
                    collection: path.collection,
                    index: path.index,
                    suffix_root: expression,
                });
            }
            let ExpressionNode::Integer(index) = table.expression(indexed.index) else {
                return None;
            };
            Some(TableFixedIndexedTargetPath {
                collection: indexed.collection,
                index: *index,
                suffix_root: expression,
            })
        }
        _ => None,
    }
}

fn indexed_target_path(expression: &Expression) -> Option<IndexedTargetPath<'_>> {
    match expression {
        Expression::Mutable(target) => indexed_target_path(target),
        Expression::Member(member) => {
            let path = indexed_target_path(&member.receiver)?;
            Some(IndexedTargetPath {
                collection: path.collection,
                index: path.index,
                suffix_root: expression,
            })
        }
        Expression::Indexed(indexed) => Some(IndexedTargetPath {
            collection: &indexed.collection,
            index: &indexed.index,
            suffix_root: expression,
        }),
        _ => None,
    }
}

fn indexed_target_path_in_table(
    table: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<TableIndexedTargetPath> {
    match table.expression(expression) {
        ExpressionNode::Mutable(target) => indexed_target_path_in_table(table, *target),
        ExpressionNode::Member(member) => {
            let path = indexed_target_path_in_table(table, member.receiver)?;
            Some(TableIndexedTargetPath {
                collection: path.collection,
                index: path.index,
                suffix_root: expression,
            })
        }
        ExpressionNode::Indexed(indexed) => {
            if let Some(path) = indexed_target_path_in_table(table, indexed.collection) {
                return Some(TableIndexedTargetPath {
                    collection: path.collection,
                    index: path.index,
                    suffix_root: expression,
                });
            }
            Some(TableIndexedTargetPath {
                collection: indexed.collection,
                index: indexed.index,
                suffix_root: expression,
            })
        }
        _ => None,
    }
}

fn resolve_indexed_target_suffix_layout(
    input: &InstructionSelectionInput<'_>,
    root_field: &FieldLayout,
    expression: &Expression,
) -> Option<(usize, TypeLayout)> {
    let cursor = NestedFieldLayoutCursor::from_root(root_field);
    let cursor = resolve_indexed_target_suffix_cursor(&input.layouts, cursor, expression)?;
    Some((cursor.byte_offset(), cursor.layout()))
}

fn resolve_indexed_target_suffix_cursor<'layout>(
    layouts: &'layout omega_layout::LayoutPlan,
    cursor: NestedFieldLayoutCursor<'layout>,
    expression: &Expression,
) -> Option<NestedFieldLayoutCursor<'layout>> {
    match expression {
        Expression::Mutable(target) => {
            resolve_indexed_target_suffix_cursor(layouts, cursor, target)
        }
        Expression::Indexed(_) => Some(cursor),
        Expression::Member(member) => {
            let cursor = resolve_indexed_target_suffix_cursor(layouts, cursor, &member.receiver)?;
            resolve_nested_field_layout_step(
                layouts,
                cursor,
                &member.member,
                member.member_symbol,
                None,
            )
        }
        _ => None,
    }
}

fn resolve_indexed_target_suffix_layout_in_table(
    input: &InstructionSelectionInput<'_>,
    root_field: &FieldLayout,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<(usize, TypeLayout)> {
    let cursor = NestedFieldLayoutCursor::from_root(root_field);
    let cursor = resolve_indexed_target_suffix_cursor_in_table(
        &input.layouts,
        cursor,
        expressions,
        expression,
    )?;
    Some((cursor.byte_offset(), cursor.layout()))
}

fn resolve_indexed_target_suffix_cursor_in_table<'layout>(
    layouts: &'layout omega_layout::LayoutPlan,
    cursor: NestedFieldLayoutCursor<'layout>,
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<NestedFieldLayoutCursor<'layout>> {
    match expressions.expression(expression) {
        ExpressionNode::Mutable(target) => {
            resolve_indexed_target_suffix_cursor_in_table(layouts, cursor, expressions, *target)
        }
        ExpressionNode::Indexed(indexed) => {
            let Some(collection_cursor) = resolve_indexed_target_suffix_cursor_in_table(
                layouts,
                cursor,
                expressions,
                indexed.collection,
            ) else {
                return Some(cursor);
            };

            let ExpressionNode::Integer(index) = expressions.expression(indexed.index) else {
                return None;
            };
            apply_fixed_array_index_to_cursor(collection_cursor, usize::try_from(*index).ok()?)
        }
        ExpressionNode::Member(member) => {
            let cursor = resolve_indexed_target_suffix_cursor_in_table(
                layouts,
                cursor,
                expressions,
                member.receiver,
            )?;
            resolve_nested_field_layout_step(
                layouts,
                cursor,
                &member.member,
                member.member_symbol,
                None,
            )
        }
        _ => None,
    }
}

fn apply_fixed_array_index_to_cursor<'layout>(
    cursor: NestedFieldLayoutCursor<'layout>,
    index: usize,
) -> Option<NestedFieldLayoutCursor<'layout>> {
    let (element_type, length) = cursor.type_descriptor().fixed_array()?;
    if index >= length {
        return None;
    }

    let element_layout = TypeLayout {
        size: cursor.layout().size / length,
        alignment: cursor.layout().alignment,
    };

    Some(NestedFieldLayoutCursor::from_indexed_fixed_array_element(
        cursor,
        index,
        element_type,
        element_layout,
    ))
}

fn descriptor_layout(
    input: &InstructionSelectionInput<'_>,
    descriptor: &TypeLayoutDescriptor,
) -> TypeLayout {
    match descriptor {
        TypeLayoutDescriptor::Reference { .. } => {
            return TypeLayout {
                size: input.runtime_abi.pointer_size,
                alignment: input.runtime_abi.pointer_alignment,
            };
        }
        TypeLayoutDescriptor::Constrained { base_type } => {
            return descriptor_layout(input, base_type);
        }
        TypeLayoutDescriptor::FixedArray {
            element_type,
            length,
        } => {
            let element = descriptor_layout(input, element_type);
            return TypeLayout {
                size: element.size.saturating_mul(*length),
                alignment: element.alignment,
            };
        }
        TypeLayoutDescriptor::Slice { .. } => {
            return TypeLayout {
                size: input.runtime_abi.string_descriptor_size(),
                alignment: input.runtime_abi.pointer_alignment,
            };
        }
        TypeLayoutDescriptor::Named { symbol, name } => {
            let type_symbol = *symbol;
            if let Some(primitive_type) = PrimitiveType::from_name(name) {
                return primitive_layout(input, primitive_type);
            }

            if let Some(layout) = builtin_type_layout(input, type_symbol) {
                return layout;
            }

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
        }
        TypeLayoutDescriptor::Unit => {}
    }

    TypeLayout::default()
}

fn inline_fixed_array_element_type(
    descriptor: &TypeLayoutDescriptor,
) -> Option<&TypeLayoutDescriptor> {
    match descriptor {
        TypeLayoutDescriptor::Constrained { base_type } => {
            inline_fixed_array_element_type(base_type)
        }
        TypeLayoutDescriptor::FixedArray { element_type, .. } => Some(element_type),
        _ => None,
    }
}

fn builtin_type_layout(
    input: &InstructionSelectionInput<'_>,
    type_symbol: SymbolHandle,
) -> Option<TypeLayout> {
    if Some(type_symbol) == input.program.symbols.builtin_type_symbol(BuiltinType::UInt) {
        return Some(TypeLayout {
            size: input.runtime_abi.pointer_size,
            alignment: input.runtime_abi.pointer_alignment,
        });
    }

    if Some(type_symbol) == input.program.symbols.builtin_type_symbol(BuiltinType::Int) {
        return Some(TypeLayout {
            size: input.runtime_abi.pointer_size,
            alignment: input.runtime_abi.pointer_alignment,
        });
    }

    if Some(type_symbol) == input.program.symbols.builtin_type_symbol(BuiltinType::Real) {
        return Some(TypeLayout {
            size: 8,
            alignment: 8,
        });
    }

    None
}

fn primitive_layout(
    input: &InstructionSelectionInput<'_>,
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
            size: input.runtime_abi.pointer_size,
            alignment: input.runtime_abi.pointer_alignment,
        },
        PrimitiveType::String => TypeLayout {
            size: input.runtime_abi.string_descriptor_size(),
            alignment: input.runtime_abi.pointer_alignment,
        },
    }
}
