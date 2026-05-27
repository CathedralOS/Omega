use crate::InstructionSelectionInput;
use omega_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_control_flow::StateKey;

use super::super::super::storage_places::{
    resolve_fixed_array_length, resolve_fixed_array_length_in_table,
    resolve_runtime_frame_fixed_indexed_target_in_table,
    resolve_runtime_frame_indexed_target_in_table, resolve_runtime_machine_indexed_target_in_table,
    resolve_runtime_pointee_fixed_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place,
    resolve_runtime_storage_place_in_table,
};
use omega_abstract_operations::RuntimeStorageRegion;
use omega_abstract_operations::SelectedInstructionKind;

pub(in crate::selection::runtime_dispatch) fn runtime_storage_copy(
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
    let source_place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        value_source_key,
        source_machine,
        source_state,
        value,
    )?;
    if target_place.byte_count != source_place.byte_count || target_place.byte_count == 0 {
        return None;
    }

    Some(SelectedInstructionKind::CopyRuntimeStorage {
        source_region: source_place.region,
        source_offset: source_place.byte_offset,
        target_region: target_place.region,
        target_offset: target_place.byte_offset,
        byte_count: target_place.byte_count,
    })
}

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
fn literal_fixed_array_subslice_index_source(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    value: &Expression,
) -> Option<super::super::super::storage_places::RuntimeStoragePlace> {
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
    Some(super::super::super::storage_places::RuntimeStoragePlace {
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

fn literal_usize(expression: &Expression) -> Option<usize> {
    let Expression::Integer(value) = expression else {
        return None;
    };
    usize::try_from(*value).ok()
}

fn literal_fixed_array_subslice_index_source_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    value: ExpressionHandle,
) -> Option<super::super::super::storage_places::RuntimeStoragePlace> {
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
    Some(super::super::super::storage_places::RuntimeStoragePlace {
        region: source_place.region,
        byte_offset,
        byte_count: element_byte_size,
    })
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

fn literal_usize_in_table(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<usize> {
    let ExpressionNode::Integer(value) = expressions.expression(expression) else {
        return None;
    };
    usize::try_from(*value).ok()
}

pub(in crate::selection::runtime_dispatch) fn runtime_storage_copy_in_table(
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
    let source_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    )?;
    if target_place.byte_count != source_place.byte_count || target_place.byte_count == 0 {
        return None;
    }

    Some(SelectedInstructionKind::CopyRuntimeStorage {
        source_region: source_place.region,
        source_offset: source_place.byte_offset,
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

pub(in crate::selection::runtime_dispatch) fn runtime_storage_indirect_copy_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
) -> Option<SelectedInstructionKind> {
    let source_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    )?;

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && source_place.byte_count > 0
    {
        return Some(
            SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee {
                source_region: source_place.region,
                source_offset: source_place.byte_offset,
                pointer_byte_offset: pointer_target.pointer_byte_offset,
                field_byte_offset: pointer_target.field_byte_offset,
                byte_count: source_place.byte_count,
            },
        );
    }

    if let Some(pointer_target) = resolve_runtime_pointee_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && source_place.byte_count > 0
    {
        return Some(
            SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee {
                source_region: source_place.region,
                source_offset: source_place.byte_offset,
                pointer_byte_offset: pointer_target.pointer_byte_offset,
                field_byte_offset: pointer_target.field_byte_offset,
                byte_count: source_place.byte_count,
            },
        );
    }

    let indexed_target = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    )?;
    if source_place.region != RuntimeStorageRegion::RuntimeFrame
        || source_place.byte_count != indexed_target.byte_count
    {
        return None;
    }

    Some(
        SelectedInstructionKind::CopyRuntimeStorageToRuntimeFrameIndexed {
            source_region: source_place.region,
            source_offset: source_place.byte_offset,
            descriptor_offset: indexed_target.descriptor_offset,
            index_offset: indexed_target.index_offset,
            element_byte_size: indexed_target.element_byte_size,
            field_byte_offset: indexed_target.field_byte_offset,
            byte_count: indexed_target.byte_count,
        },
    )
}

pub(in crate::selection::runtime_dispatch) fn runtime_storage_indexed_source_copy_in_table(
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

    let indexed_source = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    );
    if let Some(indexed_source) = indexed_source {
        if target_place.byte_count != indexed_source.byte_count {
            return None;
        }

        let kind = if target_place.region == RuntimeStorageRegion::RuntimeFrame {
            SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeFrame {
                descriptor_offset: indexed_source.descriptor_offset,
                index_offset: indexed_source.index_offset,
                element_byte_size: indexed_source.element_byte_size,
                field_byte_offset: indexed_source.field_byte_offset,
                target_offset: target_place.byte_offset,
                byte_count: target_place.byte_count,
            }
        } else {
            SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeStorage {
                descriptor_offset: indexed_source.descriptor_offset,
                index_offset: indexed_source.index_offset,
                element_byte_size: indexed_source.element_byte_size,
                field_byte_offset: indexed_source.field_byte_offset,
                target_region: target_place.region,
                target_offset: target_place.byte_offset,
                byte_count: target_place.byte_count,
            }
        };

        return Some(kind);
    }

    let indexed_source = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    )?;
    if target_place.byte_count != indexed_source.byte_count {
        return None;
    }

    Some(
        SelectedInstructionKind::CopyRuntimeMachineIndexedToRuntimeStorage {
            base_byte_offset: indexed_source.base_byte_offset,
            index_offset: indexed_source.index_offset,
            element_byte_size: indexed_source.element_byte_size,
            field_byte_offset: indexed_source.field_byte_offset,
            target_region: target_place.region,
            target_offset: target_place.byte_offset,
            byte_count: target_place.byte_count,
        },
    )
}

pub(in crate::selection::runtime_dispatch) fn runtime_storage_fixed_indexed_source_copy_in_table(
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

    let fixed_source = resolve_runtime_frame_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    )?;
    if target_place.byte_count != fixed_source.byte_count {
        return None;
    }

    let kind = if target_place.region == RuntimeStorageRegion::RuntimeFrame {
        SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeFrame {
            descriptor_offset: fixed_source.descriptor_offset,
            element_index: fixed_source.element_index,
            element_byte_size: fixed_source.element_byte_size,
            field_byte_offset: fixed_source.field_byte_offset,
            target_offset: target_place.byte_offset,
            byte_count: target_place.byte_count,
        }
    } else {
        SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeStorage {
            descriptor_offset: fixed_source.descriptor_offset,
            element_index: fixed_source.element_index,
            element_byte_size: fixed_source.element_byte_size,
            field_byte_offset: fixed_source.field_byte_offset,
            target_region: target_place.region,
            target_offset: target_place.byte_offset,
            byte_count: target_place.byte_count,
        }
    };

    Some(kind)
}
