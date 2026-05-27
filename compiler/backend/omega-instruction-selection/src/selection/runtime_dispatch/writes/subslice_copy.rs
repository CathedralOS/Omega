use crate::InstructionSelectionInput;
use omega_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_control_flow::StateKey;

use super::super::super::storage_places::{
    RuntimeStoragePlace, resolve_fixed_array_length, resolve_fixed_array_length_in_table,
    resolve_runtime_storage_place, resolve_runtime_storage_place_in_table,
};
use omega_abstract_operations::SelectedInstructionKind;

#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch) fn runtime_fixed_array_subslice_indexed_source_copy(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    value_source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    target: &Expression,
    value: &Expression,
) -> Option<SelectedInstructionKind> {
    let target_place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        target_source_key,
        source_machine,
        source_state,
        target,
    )?;
    if target_place.byte_count == 0 {
        return None;
    }

    let source = literal_fixed_array_subslice_index_source(
        input,
        dispatch_index,
        value_source_key,
        source_machine,
        source_state,
        value,
    )?;
    if target_place.byte_count != source.byte_count {
        return None;
    }

    Some(SelectedInstructionKind::CopyRuntimeStorage {
        source_region: source.region,
        source_offset: source.byte_offset,
        target_region: target_place.region,
        target_offset: target_place.byte_offset,
        byte_count: target_place.byte_count,
    })
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch) fn runtime_fixed_array_subslice_indexed_source_copy_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
) -> Option<SelectedInstructionKind> {
    let target_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    )?;
    if target_place.byte_count == 0 {
        return None;
    }

    let source = literal_fixed_array_subslice_index_source_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    )?;
    if target_place.byte_count != source.byte_count {
        return None;
    }

    Some(SelectedInstructionKind::CopyRuntimeStorage {
        source_region: source.region,
        source_offset: source.byte_offset,
        target_region: target_place.region,
        target_offset: target_place.byte_offset,
        byte_count: target_place.byte_count,
    })
}

#[allow(clippy::too_many_arguments)]
fn literal_fixed_array_subslice_index_source(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    value: &Expression,
) -> Option<RuntimeStoragePlace> {
    let Expression::Indexed(indexed_element) = value else {
        return None;
    };
    let element_index = literal_usize(&indexed_element.index)?;
    let Expression::Indexed(subslice) = &indexed_element.collection else {
        return None;
    };
    let (subslice_start, subslice_end) = literal_range_bounds(&subslice.index)?;
    let Expression::Call(call) = &subslice.collection else {
        return None;
    };
    if !call.receiver.is_some()
        || !call.arguments.is_empty()
        || (call.target.as_str() != "as_slice" && call.target.as_str() != "as_mut_slice")
    {
        return None;
    }

    let receiver = call.receiver.as_deref()?;
    let source_place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        value_source_key,
        source_machine,
        source_state,
        receiver,
    )?;
    let source_length =
        resolve_fixed_array_length(input, dispatch_index, value_source_key, receiver)?;
    fixed_array_subslice_index_source_place(
        source_place,
        source_length,
        subslice_start,
        subslice_end,
        element_index,
    )
}

fn literal_fixed_array_subslice_index_source_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    value: ExpressionHandle,
) -> Option<RuntimeStoragePlace> {
    let ExpressionNode::Indexed(indexed_element) = expressions.expression(value) else {
        return None;
    };
    let element_index = literal_usize_in_table(expressions, indexed_element.index)?;
    let ExpressionNode::Indexed(subslice) = expressions.expression(indexed_element.collection)
    else {
        return None;
    };
    let (subslice_start, subslice_end) =
        literal_range_bounds_in_table(expressions, subslice.index)?;
    let ExpressionNode::Call(call) = expressions.expression(subslice.collection) else {
        return None;
    };
    if !call.receiver.is_valid()
        || !call.arguments.is_empty()
        || (call.target.as_str() != "as_slice" && call.target.as_str() != "as_mut_slice")
    {
        return None;
    }

    let source_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    )?;
    let source_length = resolve_fixed_array_length_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    )?;
    fixed_array_subslice_index_source_place(
        source_place,
        source_length,
        subslice_start,
        subslice_end,
        element_index,
    )
}

fn fixed_array_subslice_index_source_place(
    source_place: RuntimeStoragePlace,
    source_length: usize,
    subslice_start: usize,
    subslice_end: Option<usize>,
    element_index: usize,
) -> Option<RuntimeStoragePlace> {
    if source_length == 0 || source_place.byte_count % source_length != 0 {
        return None;
    }

    let end = subslice_end.unwrap_or(source_length);
    let absolute_index = subslice_start.checked_add(element_index)?;
    if subslice_start > end || end > source_length || absolute_index >= end {
        return None;
    }

    let element_byte_size = source_place.byte_count / source_length;
    let byte_offset = source_place
        .byte_offset
        .checked_add(absolute_index.checked_mul(element_byte_size)?)?;
    Some(RuntimeStoragePlace {
        region: source_place.region,
        byte_offset,
        byte_count: element_byte_size,
    })
}

fn literal_range_bounds(range: &Expression) -> Option<(usize, Option<usize>)> {
    let Expression::Range(range) = range else {
        return None;
    };
    let start = match range.start.as_deref() {
        Some(start) => literal_usize(start)?,
        None => 0,
    };
    let end = match range.end.as_deref() {
        Some(end) => Some(literal_usize(end)?),
        None => None,
    };
    Some((start, end))
}

fn literal_range_bounds_in_table(
    expressions: &ExpressionTable,
    range: ExpressionHandle,
) -> Option<(usize, Option<usize>)> {
    let ExpressionNode::Range(range) = expressions.expression(range) else {
        return None;
    };
    let start = if range.start.is_valid() {
        literal_usize_in_table(expressions, range.start)?
    } else {
        0
    };
    let end = if range.end.is_valid() {
        Some(literal_usize_in_table(expressions, range.end)?)
    } else {
        None
    };
    Some((start, end))
}

fn literal_usize(expression: &Expression) -> Option<usize> {
    let Expression::Integer(value) = expression else {
        return None;
    };
    usize::try_from(*value).ok()
}

fn literal_usize_in_table(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<usize> {
    let ExpressionNode::Integer(value) = expressions.expression(expression) else {
        return None;
    };
    usize::try_from(*value).ok()
}
