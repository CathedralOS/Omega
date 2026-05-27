use crate::InstructionSelectionInput;
use crate::selection::storage_places::{
    RuntimeStoragePlace, resolve_fixed_array_length_in_table,
    resolve_runtime_storage_place_in_table,
};
use omega_checked_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableRangeExpression,
};
use omega_control_flow::StateKey;

pub(super) struct FixedArraySliceSource {
    pub(super) place: RuntimeStoragePlace,
    pub(super) length: usize,
    pub(super) element_byte_size: usize,
}

pub(super) fn literal_fixed_array_slice_source_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    value: ExpressionHandle,
) -> Option<FixedArraySliceSource> {
    match expressions.expression(value) {
        ExpressionNode::Call(call)
            if call.receiver.is_valid()
                && call.arguments.is_empty()
                && (call.target.as_str() == "as_slice"
                    || call.target.as_str() == "as_mut_slice") =>
        {
            let place = resolve_runtime_storage_place_in_table(
                input,
                dispatch_index,
                value_source_key,
                expressions,
                call.receiver,
            )?;
            let length = resolve_fixed_array_length_in_table(
                input,
                dispatch_index,
                value_source_key,
                expressions,
                call.receiver,
            )?;
            fixed_array_slice_source(place, length)
        }
        ExpressionNode::Indexed(subslice) => {
            let ExpressionNode::Range(range) = expressions.expression(subslice.index) else {
                return None;
            };
            let source = literal_fixed_array_slice_source_in_table(
                input,
                dispatch_index,
                value_source_key,
                expressions,
                subslice.collection,
            )?;
            fixed_array_subslice_source(source, expressions, range)
        }
        _ => None,
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
    expressions: &ExpressionTable,
    range: &TableRangeExpression,
) -> Option<FixedArraySliceSource> {
    let (start, length) = literal_subslice_bounds(expressions, range, source.length)?;
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

pub(super) fn literal_subslice_bounds(
    expressions: &ExpressionTable,
    range: &TableRangeExpression,
    source_length: usize,
) -> Option<(usize, usize)> {
    let start = if range.start.is_valid() {
        let ExpressionNode::Integer(start) = expressions.expression(range.start) else {
            return None;
        };
        usize::try_from(*start).ok()?
    } else {
        0
    };
    let end = if range.end.is_valid() {
        let ExpressionNode::Integer(end) = expressions.expression(range.end) else {
            return None;
        };
        usize::try_from(*end).ok()?
    } else {
        source_length
    };

    if start > end || end > source_length {
        return None;
    }

    Some((start, end - start))
}
