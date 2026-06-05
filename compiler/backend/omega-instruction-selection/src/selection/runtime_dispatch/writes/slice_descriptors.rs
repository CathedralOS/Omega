use crate::InstructionSelectionInput;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    RuntimeStorageRegion, RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind,
    StateGuardOperator,
};
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_control_flow::StateKey;
use omega_core::arena::Arena;

use super::super::super::storage_places::{
    resolve_fixed_array_length, resolve_fixed_array_length_in_table,
    resolve_runtime_frame_base_indexed_target, resolve_runtime_frame_base_indexed_target_in_table,
    resolve_runtime_frame_fixed_indexed_target,
    resolve_runtime_frame_fixed_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place,
    resolve_runtime_storage_place_in_table, resolve_slice_element_byte_size_in_table,
};
use super::fixed_array_slices::{
    literal_subslice_range_bounds, resolved_subslice_descriptor_base_in_table,
};

/// Materialize a subslice of a *runtime* slice descriptor (`entries[start..]` /
/// `entries[start..end]` where `entries` is a `&[T]` whose length is only known
/// at runtime) into the target descriptor `slot`. Unlike the fixed-array path,
/// the length is not a compile-time constant, so the new descriptor is computed
/// from the source one:
///   target.ptr = source.ptr + start * element_byte_size
///   target.len = (end - start)          when `end` is a literal
///              = source.len - start      when the range is open-ended
/// Because the binary write reads `left` from the source descriptor and writes
/// the target, this also handles the self-recursive case where source and target
/// are the same slot (an in-place shrink) — exactly what a `decreases … Length`
/// recursion needs.
#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn emit_runtime_frame_slot_runtime_subslice_descriptor_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let descriptor_size = input.runtime_abi.slice_descriptor_size();
    if slot.byte_size != descriptor_size {
        return false;
    }
    let ExpressionNode::Indexed(indexed) = expressions.expression(value) else {
        return false;
    };
    let ExpressionNode::Range(range) = expressions.expression(indexed.index) else {
        return false;
    };

    // The source must be a runtime slice descriptor living in a frame slot.
    let Some(source_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        indexed.collection,
    ) else {
        return false;
    };
    if source_place.region != RuntimeStorageRegion::RuntimeFrame
        || source_place.byte_count != descriptor_size
    {
        return false;
    }
    let Some(element_byte_size) = resolve_slice_element_byte_size_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        indexed.collection,
    ) else {
        return false;
    };

    // Literal `start` (default 0) and optional literal `end`.
    let start = if range.start.is_valid() {
        let ExpressionNode::Integer(start) = expressions.expression(range.start) else {
            return false;
        };
        match usize::try_from(*start) {
            Ok(start) => start,
            Err(_) => return false,
        }
    } else {
        0
    };
    let end_literal = if range.end.is_valid() {
        let ExpressionNode::Integer(end) = expressions.expression(range.end) else {
            return false;
        };
        match usize::try_from(*end) {
            Ok(end) => Some(end),
            Err(_) => return false,
        }
    } else {
        None
    };
    if end_literal.is_some_and(|end| end < start) {
        return false;
    }

    let descriptor = input.runtime_abi.slice_descriptor();
    let ptr_offset = descriptor.ptr_offset();
    let len_offset = descriptor.len_offset();
    let len_size = descriptor.len_size();
    let ptr_size = len_offset - ptr_offset;
    let Some(ptr_delta) = start.checked_mul(element_byte_size) else {
        return false;
    };

    // target.ptr = source.ptr + start * element_byte_size
    let ptr_left = runtime_value_operands.insert(RuntimeValueOperand::Storage {
        region: RuntimeStorageRegion::RuntimeFrame,
        byte_offset: source_place.byte_offset + ptr_offset,
        byte_size: ptr_size,
    });
    let ptr_right = runtime_value_operands.insert(RuntimeValueOperand::Immediate(ptr_delta as i64));
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageBinary {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            target_offset: slot.byte_offset + ptr_offset,
            byte_size: ptr_size,
            left: ptr_left,
            operator: StateGuardOperator::Add,
            right: ptr_right,
            is_float: false,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });

    // target.len = (end - start) if `end` is a literal, else source.len - start.
    match end_literal {
        Some(end) => selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
                target_region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: slot.byte_offset + len_offset,
                byte_size: len_size,
                value: (end - start) as i64,
            },
            source_key: value_source_key,
            source_statement: statement_index,
        }),
        None => {
            let len_left = runtime_value_operands.insert(RuntimeValueOperand::Storage {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: source_place.byte_offset + len_offset,
                byte_size: len_size,
            });
            let len_right =
                runtime_value_operands.insert(RuntimeValueOperand::Immediate(start as i64));
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeStorageBinary {
                    target_region: RuntimeStorageRegion::RuntimeFrame,
                    target_offset: slot.byte_offset + len_offset,
                    byte_size: len_size,
                    left: len_left,
                    operator: StateGuardOperator::Subtract,
                    right: len_right,
                    is_float: false,
                },
                source_key: value_source_key,
                source_statement: statement_index,
            });
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn emit_runtime_frame_slot_slice_descriptor_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if slot.byte_size != input.runtime_abi.slice_descriptor_size() {
        return false;
    }

    if emit_runtime_frame_slot_literal_subslice_descriptor_write_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        slot,
        value,
        selected_instructions,
        statement_index,
    ) {
        return true;
    }

    let ExpressionNode::Call(call) = expressions.expression(value) else {
        return false;
    };
    if !call.receiver.is_valid()
        || !call.arguments.is_empty()
        || (call.target.as_str() != "as_slice" && call.target.as_str() != "as_mut_slice")
    {
        return false;
    }

    if let Some(indexed_target) = resolve_runtime_frame_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) {
        let Some(length) = resolve_fixed_array_length_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            call.receiver,
        ) else {
            return false;
        };

        emit_fixed_indexed_slice_descriptor(
            input,
            value_source_key,
            statement_index,
            slot,
            indexed_target.descriptor_offset,
            indexed_target.element_index,
            indexed_target.element_byte_size,
            indexed_target.field_byte_offset,
            length,
            selected_instructions,
        );
        return true;
    }

    // `ptr.field.as_[mut_]slice()` where `ptr` is a runtime reference parameter: the
    // slice's data pointer is the referent's ADDRESS, computed from the runtime
    // pointer (the param slot's value) plus the field offset -- NOT a static address
    // (which only coincidentally matched the runtime pointer before the frame layout
    // changed).
    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) {
        let Some(length) = resolve_fixed_array_length_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            call.receiver,
        ) else {
            return false;
        };
        emit_pointee_slice_descriptor(
            input,
            value_source_key,
            statement_index,
            slot,
            pointer_target.pointer_byte_offset,
            pointer_target.field_byte_offset,
            length,
            selected_instructions,
        );
        return true;
    }

    let Some(source_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) else {
        let receiver = expressions.to_tree(call.receiver);
        let simplified_receiver = super::mutation::simplify_runtime_expression_with_state_locals(
            input,
            value_source_key,
            statement_index,
            &receiver,
        );
        if let Some(indexed_target) = resolve_runtime_frame_fixed_indexed_target(
            input,
            dispatch_index,
            value_source_key,
            &simplified_receiver,
        ) {
            let Some(length) = resolve_fixed_array_length(
                input,
                dispatch_index,
                value_source_key,
                &simplified_receiver,
            ) else {
                return false;
            };

            emit_fixed_indexed_slice_descriptor(
                input,
                value_source_key,
                statement_index,
                slot,
                indexed_target.descriptor_offset,
                indexed_target.element_index,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                length,
                selected_instructions,
            );
            return true;
        }
        if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target(
            input,
            dispatch_index,
            value_source_key,
            &simplified_receiver,
        ) {
            let Some(length) = resolve_fixed_array_length(
                input,
                dispatch_index,
                value_source_key,
                &simplified_receiver,
            ) else {
                return false;
            };

            emit_indexed_slice_descriptor(
                input,
                value_source_key,
                statement_index,
                slot,
                indexed_target.base_byte_offset,
                indexed_target.index_offset,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                length,
                selected_instructions,
            );
            return true;
        }

        let Some(source_place) = resolve_runtime_storage_place(
            input,
            dispatch_index,
            value_source_key,
            "",
            "",
            &simplified_receiver,
        ) else {
            return false;
        };
        let Some(length) = resolve_fixed_array_length(
            input,
            dispatch_index,
            value_source_key,
            &simplified_receiver,
        ) else {
            return false;
        };

        emit_static_slice_descriptor(
            input,
            value_source_key,
            statement_index,
            slot,
            source_place.region,
            source_place.byte_offset,
            length,
            selected_instructions,
        );
        return true;
    };

    if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) {
        let Some(length) = resolve_fixed_array_length_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            call.receiver,
        ) else {
            return false;
        };

        emit_indexed_slice_descriptor(
            input,
            value_source_key,
            statement_index,
            slot,
            indexed_target.base_byte_offset,
            indexed_target.index_offset,
            indexed_target.element_byte_size,
            indexed_target.field_byte_offset,
            length,
            selected_instructions,
        );
        return true;
    }

    let Some(length) = resolve_fixed_array_length_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) else {
        return false;
    };

    emit_static_slice_descriptor(
        input,
        value_source_key,
        statement_index,
        slot,
        source_place.region,
        source_place.byte_offset,
        length,
        selected_instructions,
    );
    true
}

#[allow(clippy::too_many_arguments)]
fn emit_fixed_indexed_slice_descriptor(
    input: &InstructionSelectionInput<'_>,
    value_source_key: StateKey,
    statement_index: usize,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    descriptor_offset: usize,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    length: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeFrameFixedIndexedAddressToRuntimeFrame {
            descriptor_offset,
            element_index,
            element_byte_size,
            field_byte_offset,
            target_offset: slot.byte_offset,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });
    emit_slice_descriptor_length(
        input,
        value_source_key,
        statement_index,
        slot,
        length,
        selected_instructions,
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_indexed_slice_descriptor(
    input: &InstructionSelectionInput<'_>,
    value_source_key: StateKey,
    statement_index: usize,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    base_byte_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    length: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame {
            base_byte_offset,
            index_offset,
            element_byte_size,
            field_byte_offset,
            target_offset: slot.byte_offset,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });
    emit_slice_descriptor_length(
        input,
        value_source_key,
        statement_index,
        slot,
        length,
        selected_instructions,
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_pointee_slice_descriptor(
    input: &InstructionSelectionInput<'_>,
    value_source_key: StateKey,
    statement_index: usize,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    length: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    // ptr = *(runtime pointer at pointer_byte_offset) + field_byte_offset.
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimePointeeAddressToRuntimeFrame {
            pointer_byte_offset,
            field_byte_offset,
            target_offset: slot.byte_offset,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });
    emit_slice_descriptor_length(
        input,
        value_source_key,
        statement_index,
        slot,
        length,
        selected_instructions,
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_static_slice_descriptor(
    input: &InstructionSelectionInput<'_>,
    value_source_key: StateKey,
    statement_index: usize,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    source_region: RuntimeStorageRegion,
    source_offset: usize,
    length: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            source_region,
            source_offset,
            target_offset: slot.byte_offset,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });
    emit_slice_descriptor_length(
        input,
        value_source_key,
        statement_index,
        slot,
        length,
        selected_instructions,
    );
}

fn emit_slice_descriptor_length(
    input: &InstructionSelectionInput<'_>,
    value_source_key: StateKey,
    statement_index: usize,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    length: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let descriptor = input.runtime_abi.slice_descriptor();
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset: slot.byte_offset + descriptor.len_offset(),
            byte_size: descriptor.len_size(),
            value: length as i64,
        },
        source_key: value_source_key,
        source_statement: statement_index,
    });
}

#[allow(clippy::too_many_arguments)]
fn emit_runtime_frame_slot_literal_subslice_descriptor_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
    selected_instructions: &mut SelectedInstructionSink,
    statement_index: usize,
) -> bool {
    let ExpressionNode::Indexed(indexed) = expressions.expression(value) else {
        return false;
    };
    let ExpressionNode::Range(range) = expressions.expression(indexed.index) else {
        return false;
    };
    let Some(source) = resolved_subslice_descriptor_base_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        indexed.collection,
    ) else {
        return false;
    };
    let Some((start, end)) = literal_subslice_range_bounds(expressions, range, source.length)
    else {
        return false;
    };
    // Uniform subslice: new.ptr = base.ptr + start * element_byte_size,
    // new.len = end - start. The shape is owned by omega-runtime-abi.
    let Some(subslice) = input.runtime_abi.slice_descriptor().subslice(
        source.place.byte_offset,
        source.element_byte_size,
        start,
        end,
    ) else {
        return false;
    };

    emit_static_slice_descriptor(
        input,
        value_source_key,
        statement_index,
        slot,
        source.place.region,
        subslice.ptr_delta,
        subslice.len,
        selected_instructions,
    );
    true
}
