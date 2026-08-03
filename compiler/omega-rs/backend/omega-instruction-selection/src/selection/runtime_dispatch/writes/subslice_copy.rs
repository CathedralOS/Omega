use crate::InstructionSelectionInput;
use omega_control_flow::StateKey;
use psi_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};

use super::super::super::storage_places::{
    RuntimeStoragePlace, resolve_fixed_array_length, resolve_runtime_storage_place,
    resolve_runtime_storage_place_in_table,
};
use super::fixed_array_slices::{FixedArraySliceSource, literal_fixed_array_slice_source_in_table};
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    RuntimeStorageRegion, SelectedInstruction, SelectedInstructionKind,
};

/// Materialize a RANGE subslice of a literal fixed array (`arr[a..b]` /
/// `arr.as_slice()[a..b]`) as a fat `{ptr, len}` descriptor written into a
/// frame-slot target -- the missing case in the body-mutation value path,
/// which otherwise byte-COPIES into the target (wrong for a `&[u8]` view, whose
/// 16-byte slot must hold a descriptor, not the bytes). Fires only when the
/// value resolves to a fixed-array subslice AND the target is a descriptor-
/// sized frame slot; declines otherwise (element copies and array-into-array
/// copies keep their existing lowering). Mirrors `emit_static_slice_descriptor`.
#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch) fn runtime_fixed_array_subslice_descriptor_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    value_source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    operation_source_key: StateKey,
    statement_index: usize,
    target: &Expression,
    value: &Expression,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let Some(source) = literal_fixed_array_slice_source(
        input,
        dispatch_index,
        value_source_key,
        source_machine,
        source_state,
        value,
    ) else {
        return false;
    };
    let Some(target_place) = resolve_runtime_storage_place(
        input,
        dispatch_index,
        target_source_key,
        source_machine,
        source_state,
        target,
    ) else {
        return false;
    };
    let descriptor = input.runtime_abi.slice_descriptor();
    if target_place.region != RuntimeStorageRegion::RuntimeFrame
        || target_place.byte_count != descriptor.total_size()
    {
        return false;
    }
    // ptr = address of the (windowed) source array start, into the descriptor's
    // pointer slot; len = the subslice element count, into its length slot.
    selected_instructions.push(SelectedInstruction {
        kind: crate::selection::runtime_dispatch::write_place_address_direct(
            source.place.region,
            source.place.byte_offset,
            target_place.byte_offset,
        ),
        source_key: operation_source_key,
        source_statement: statement_index,
    });
    selected_instructions.push(SelectedInstruction {
        kind: crate::selection::runtime_dispatch::write_place_integer_direct(
            RuntimeStorageRegion::RuntimeFrame,
            target_place.byte_offset + descriptor.len_offset(),
            source.length as i64,
            descriptor.len_size(),
        ),
        source_key: operation_source_key,
        source_statement: statement_index,
    });
    true
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

    Some(crate::selection::runtime_dispatch::copy_places_direct(
        source.region,
        source.byte_offset,
        target_place.region,
        target_place.byte_offset,
        target_place.byte_count,
    ))
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

    Some(crate::selection::runtime_dispatch::copy_places_direct(
        source.region,
        source.byte_offset,
        target_place.region,
        target_place.byte_offset,
        target_place.byte_count,
    ))
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
    let source = literal_fixed_array_slice_source(
        input,
        dispatch_index,
        value_source_key,
        source_machine,
        source_state,
        &indexed_element.collection,
    )?;
    fixed_array_index_source_place(source, element_index)
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
    let source = literal_fixed_array_slice_source_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        indexed_element.collection,
    )?;
    fixed_array_index_source_place(source, element_index)
}

/// The element count of a literal-bounded fixed-array subslice expression
/// (`arr[a..b]` / `arr.as_slice()[a..b]` / a bare fixed array) -- i.e. the
/// compile-time value of its `.len`. `None` when the value is not such a
/// subslice (runtime bounds, a non-array source, etc.).
#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch) fn fixed_array_subslice_length(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    value: &Expression,
) -> Option<usize> {
    literal_fixed_array_slice_source(
        input,
        dispatch_index,
        value_source_key,
        source_machine,
        source_state,
        value,
    )
    .map(|source| source.length)
}

#[allow(clippy::too_many_arguments)]
fn literal_fixed_array_slice_source(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    value: &Expression,
) -> Option<FixedArraySliceSource> {
    match value {
        Expression::Call(call)
            if call.receiver.is_some()
                && call.arguments.is_empty()
                && (call.target.as_str() == "as_slice"
                    || call.target.as_str() == "as_mut_slice") =>
        {
            let receiver = call.receiver.as_deref()?;
            let place = resolve_runtime_storage_place(
                input,
                dispatch_index,
                value_source_key,
                source_machine,
                source_state,
                receiver,
            )?;
            let length =
                resolve_fixed_array_length(input, dispatch_index, value_source_key, receiver)?;
            fixed_array_slice_source(place, length)
        }
        Expression::Indexed(subslice) => {
            let (start, end) = literal_range_bounds(&subslice.index)?;
            let source = literal_fixed_array_slice_source(
                input,
                dispatch_index,
                value_source_key,
                source_machine,
                source_state,
                &subslice.collection,
            )?;
            fixed_array_subslice_source(source, start, end)
        }
        // A BARE fixed array used directly as a slice base (`arr[a..b]` without
        // an explicit `.as_slice()`). Resolves only when the value is an actual
        // fixed array with a literal length, so non-array values still decline.
        _ => {
            let place = resolve_runtime_storage_place(
                input,
                dispatch_index,
                value_source_key,
                source_machine,
                source_state,
                value,
            )?;
            let length =
                resolve_fixed_array_length(input, dispatch_index, value_source_key, value)?;
            fixed_array_slice_source(place, length)
        }
    }
}

fn fixed_array_slice_source(
    place: RuntimeStoragePlace,
    length: usize,
) -> Option<FixedArraySliceSource> {
    if length == 0 || place.byte_count % length != 0 {
        return None;
    }
    let element_byte_size = place.byte_count / length;
    Some(FixedArraySliceSource {
        place,
        length,
        element_byte_size,
    })
}

fn fixed_array_subslice_source(
    source: FixedArraySliceSource,
    start: usize,
    end: Option<usize>,
) -> Option<FixedArraySliceSource> {
    let end = end.unwrap_or(source.length);
    if start > end || end > source.length {
        return None;
    }
    let length = end - start;
    let byte_offset = source
        .place
        .byte_offset
        .checked_add(start.checked_mul(source.element_byte_size)?)?;
    Some(FixedArraySliceSource {
        place: RuntimeStoragePlace {
            region: source.place.region,
            byte_offset,
            byte_count: length.checked_mul(source.element_byte_size)?,
        },
        length,
        element_byte_size: source.element_byte_size,
    })
}

fn fixed_array_index_source_place(
    source: FixedArraySliceSource,
    element_index: usize,
) -> Option<RuntimeStoragePlace> {
    if element_index >= source.length {
        return None;
    }
    let byte_offset = source
        .place
        .byte_offset
        .checked_add(element_index.checked_mul(source.element_byte_size)?)?;
    Some(RuntimeStoragePlace {
        region: source.place.region,
        byte_offset,
        byte_count: source.element_byte_size,
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
    usize::try_from(value.value_i64()?).ok()
}

fn literal_usize_in_table(
    expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> Option<usize> {
    let ExpressionNode::Integer(value) = expressions.expression(expression) else {
        return None;
    };
    usize::try_from(value.value_i64()?).ok()
}
