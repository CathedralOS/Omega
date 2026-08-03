use crate::InstructionSelectionInput;
use crate::selection::storage_places::{
    RuntimeStoragePlace, resolve_fixed_array_length_in_table,
    resolve_runtime_storage_place_in_table,
};
use omega_control_flow::StateKey;
use psi_checked_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableRangeExpression,
};

pub(super) struct FixedArraySliceSource {
    pub(super) place: RuntimeStoragePlace,
    pub(super) length: usize,
    pub(super) element_byte_size: usize,
}

/// Resolve the base of a subslice-descriptor write to a `{place, length,
/// element_byte_size}` source.
///
/// This is the widening seam for the fat-descriptor subslice unlock. Today it
/// resolves literal fixed-array-backed views (`x.as_slice()[start..end]`); the
/// returned [`FixedArraySliceSource`] carries the element byte size so callers
/// can offset the descriptor pointer uniformly via
/// `FatDescriptorAbi::subslice`, regardless of how the base length was proven.
pub(super) fn resolved_subslice_descriptor_base_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    value: ExpressionHandle,
) -> Option<FixedArraySliceSource> {
    literal_fixed_array_slice_source_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    )
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
        // A BARE fixed array used directly as a slice base (`self.arr[a..b]` or a
        // local `arr[a..b]` without an explicit `.as_slice()`). Resolves only when
        // the value is an actual fixed array with a literal length, so non-array
        // values still decline. Mirrors the base case in subslice_copy.rs; without
        // it, a subslice-of-machine-field argument (`walk(self.source[0..2])`)
        // declines descriptor construction and falls through to a garbage copy.
        _ => {
            let place = resolve_runtime_storage_place_in_table(
                input,
                dispatch_index,
                value_source_key,
                expressions,
                value,
            )?;
            let length = resolve_fixed_array_length_in_table(
                input,
                dispatch_index,
                value_source_key,
                expressions,
                value,
            )?;
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

/// Resolve a literal subslice range to `(start, end)` against a base of
/// `source_length` elements, validating `start <= end <= source_length`.
///
/// Unlike [`literal_subslice_bounds`], which returns `(start, length)`, this
/// returns the half-open `[start, end)` pair expected by
/// `FatDescriptorAbi::subslice`. Open ends default to the base length.
pub(super) fn literal_subslice_range_bounds(
    expressions: &ExpressionTable,
    range: &TableRangeExpression,
    source_length: usize,
) -> Option<(usize, usize)> {
    let (start, length) = literal_subslice_bounds(expressions, range, source_length)?;
    Some((start, start + length))
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
        usize::try_from(start.value_i64()?).ok()?
    } else {
        0
    };
    let end = if range.end.is_valid() {
        let ExpressionNode::Integer(end) = expressions.expression(range.end) else {
            return None;
        };
        usize::try_from(end.value_i64()?).ok()?
    } else {
        source_length
    };

    if start > end || end > source_length {
        return None;
    }

    Some((start, end - start))
}
