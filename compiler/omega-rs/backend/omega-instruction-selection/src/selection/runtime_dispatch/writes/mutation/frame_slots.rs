use crate::InstructionSelectionInput;
use crate::selection::runtime_dispatch::text_writes::string_literal_data_handle;
use crate::selection::runtime_dispatch::writes::mutation::operators::supports_scalar_integer_write;
use crate::selection::storage_places::{
    resolve_runtime_frame_base_indexed_target_in_table,
    resolve_runtime_frame_fixed_indexed_target_in_table,
    resolve_runtime_frame_indexed_target_in_table,
    resolve_runtime_frame_indexed_target_near_slot_in_table,
    resolve_runtime_pointee_fixed_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place_in_table,
};
use omega_abstract_operations::{
    RuntimeStorageRegion, RuntimeValueOperand, SelectedInstructionKind,
};
use omega_checked_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableNamePath,
};
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};

use super::super::static_values::{
    RuntimeStaticValues, resolve_runtime_static_integer_value_in_table,
};

pub(in crate::selection) fn runtime_frame_slot_target_expression(
    expressions: &mut ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
) -> ExpressionHandle {
    let mut members = HandleSpan::empty();
    expressions.push_name_path_member(&mut members, slot.name.clone());

    let mut member_symbols = HandleSpan::empty();
    expressions.push_name_path_member_symbol(&mut member_symbols, slot.symbol);

    expressions.insert(ExpressionNode::Name(TableNamePath {
        members,
        member_symbols,
        head_symbol: slot.symbol,
        symbol: slot.symbol,
    }))
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn select_runtime_frame_slot_value_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    select_runtime_frame_slot_value_write_in_table_with_source_anchor(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        slot,
        value,
        static_values,
        runtime_value_operands,
        slot.byte_offset,
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::selection) fn select_runtime_frame_slot_value_write_in_table_with_source_anchor(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    statement_index: usize,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
    static_values: &RuntimeStaticValues,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    source_anchor_byte_offset: usize,
) -> Option<SelectedInstructionKind> {
    if slot.byte_size == input.runtime_abi.pointer_size
        && let Some(kind) = select_runtime_frame_slot_address_write_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            slot,
            value,
        )
    {
        return Some(kind);
    }

    if supports_scalar_integer_write(slot.byte_size)
        && let Some(value) =
            resolve_runtime_static_integer_value_in_table(input, expressions, value, static_values)
    {
        return Some(SelectedInstructionKind::WriteRuntimeStorageInteger {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset: slot.byte_offset,
            byte_size: slot.byte_size,
            value,
        });
    }

    if slot.byte_size == input.runtime_abi.string_descriptor_size()
        && let Some(value) = expressions.string_literal_value(value)
    {
        let data = string_literal_data_handle(input, value_source_key, statement_index, &value);
        if data.is_valid() {
            return Some(SelectedInstructionKind::WriteRuntimeFrameString {
                byte_offset: slot.byte_offset,
                data,
                byte_length: value.len(),
            });
        }
    }

    if let Some(pointee) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && pointee.pointee_byte_size == slot.byte_size
        && pointee.pointee_byte_size > 0
        && !matches!(expressions.expression(value), ExpressionNode::Mutable(_))
    {
        return Some(SelectedInstructionKind::CopyRuntimePointeeToRuntimeFrame {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            pointer_byte_offset: pointee.pointer_byte_offset,
            field_byte_offset: pointee.field_byte_offset,
            target_offset: slot.byte_offset,
            byte_count: slot.byte_size,
        });
    }

    // `let d = rooms[2].depth` where `rooms: &[Room]` is a slice DESCRIPTOR local:
    // read the element field through the descriptor's pointer. The fixed-indexed
    // pointee target folds the CONSTANT index into its field offset (deref ptr +
    // index*element_size + field_offset). Without this, the flat storage-place
    // fallback below treats the descriptor as a struct and reads its bytes at the
    // bare field offset -- dropping both the index and the deref, so every constant
    // index aliased element 0's garbage. (A dynamic index lowers via the
    // FrameIndexed path; this mirrors the guard-side constant-index fix 8e775fbd.)
    if let Some(pointee) = resolve_runtime_pointee_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && pointee.pointee_byte_size == slot.byte_size
        && pointee.pointee_byte_size > 0
    {
        return Some(SelectedInstructionKind::CopyRuntimePointeeToRuntimeFrame {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            pointer_byte_offset: pointee.pointer_byte_offset,
            field_byte_offset: pointee.field_byte_offset,
            target_offset: slot.byte_offset,
            byte_count: slot.byte_size,
        });
    }

    if let Some(source_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && source_place.byte_count == slot.byte_size
        && source_place.byte_count > 0
    {
        return Some(SelectedInstructionKind::CopyRuntimeStorage {
            source_region: source_place.region,
            source_offset: source_place.byte_offset,
            target_region: RuntimeStorageRegion::RuntimeFrame,
            target_offset: slot.byte_offset,
            byte_count: slot.byte_size,
        });
    }

    if let Some(indexed_source) = resolve_runtime_frame_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && indexed_source.byte_count == slot.byte_size
        && indexed_source.byte_count > 0
    {
        return Some(
            SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeFrame {
                descriptor_offset: indexed_source.descriptor_offset,
                element_index: indexed_source.element_index,
                element_byte_size: indexed_source.element_byte_size,
                field_byte_offset: indexed_source.field_byte_offset,
                target_offset: slot.byte_offset,
                byte_count: slot.byte_size,
            },
        );
    }

    if let Some(indexed_source) = resolve_runtime_frame_indexed_target_near_slot_in_table(
        input,
        dispatch_index,
        expressions,
        value,
        source_anchor_byte_offset,
    )
    .or_else(|| {
        resolve_runtime_frame_indexed_target_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            value,
        )
    }) && indexed_source.byte_count == slot.byte_size
        && indexed_source.byte_count > 0
    {
        return Some(
            SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeFrame {
                descriptor_offset: indexed_source.descriptor_offset,
                index_offset: indexed_source.index_offset,
                element_byte_size: indexed_source.element_byte_size,
                field_byte_offset: indexed_source.field_byte_offset,
                target_offset: slot.byte_offset,
                byte_count: slot.byte_size,
            },
        );
    }

    if let Some(kind) = super::select_runtime_frame_slot_convert_write_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        slot,
        value,
        static_values,
        runtime_value_operands,
    ) {
        return Some(kind);
    }

    // Decision 17 (operand-driven): the write resolves its arithmetic domain from
    // the OPERANDS' types internally, not from the destination slot.
    super::select_runtime_storage_binary_write_in_table(
        input,
        dispatch_index,
        value_source_key,
        statement_index,
        expressions,
        RuntimeStorageRegion::RuntimeFrame,
        slot.byte_offset,
        slot.byte_size,
        value,
        static_values,
        runtime_value_operands,
    )
}

fn select_runtime_frame_slot_address_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    value: ExpressionHandle,
) -> Option<SelectedInstructionKind> {
    // `&mut <place>` / `&<place>` (e.g. `&mut cells[index]`): bind the reference
    // slot to the *address* of the referenced place rather than copying the
    // referent. Without this the place is mis-lowered as a copy through the
    // still-uninitialized reference slot.
    if let ExpressionNode::Mutable(inner) = expressions.expression(value) {
        return select_runtime_frame_slot_place_address_write_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            slot,
            *inner,
        );
    }

    let ExpressionNode::Call(call) = expressions.expression(value) else {
        return None;
    };
    if !call.receiver.is_valid()
        || !call.arguments.is_empty()
        || (call.target.as_str() != "as_slice" && call.target.as_str() != "as_mut_slice")
    {
        return None;
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) {
        return Some(
            SelectedInstructionKind::WriteRuntimePointeeAddressToRuntimeFrame {
                pointer_byte_offset: pointer_target.pointer_byte_offset,
                field_byte_offset: pointer_target.field_byte_offset,
                target_offset: slot.byte_offset,
            },
        );
    }

    if let Some(indexed_target) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) {
        return Some(
            SelectedInstructionKind::WriteRuntimeFrameIndexedAddressToRuntimeFrame {
                descriptor_offset: indexed_target.descriptor_offset,
                index_offset: indexed_target.index_offset,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                target_offset: slot.byte_offset,
            },
        );
    }

    if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    ) {
        return Some(
            SelectedInstructionKind::WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame {
                base_byte_offset: indexed_target.base_byte_offset,
                index_offset: indexed_target.index_offset,
                element_byte_size: indexed_target.element_byte_size,
                field_byte_offset: indexed_target.field_byte_offset,
                target_offset: slot.byte_offset,
            },
        );
    }

    let source_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        call.receiver,
    )?;
    Some(
        SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            source_region: source_place.region,
            source_offset: source_place.byte_offset,
            target_offset: slot.byte_offset,
        },
    )
}

/// Writes the address of the referenced place `referent` (the target of a `&` /
/// `&mut`) into the reference slot. Resolves the place through the same target
/// shapes as a read — fixed/runtime slice index, inline-array index, pointee
/// field, or a plain storage place — and emits the matching address-to-frame
/// write so the slot ends up holding the element address, not a copy of it.
fn select_runtime_frame_slot_place_address_write_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    referent: ExpressionHandle,
) -> Option<SelectedInstructionKind> {
    if let Some(target) = resolve_runtime_frame_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        referent,
    ) {
        return Some(
            SelectedInstructionKind::WriteRuntimeFrameFixedIndexedAddressToRuntimeFrame {
                descriptor_offset: target.descriptor_offset,
                element_index: target.element_index,
                element_byte_size: target.element_byte_size,
                field_byte_offset: target.field_byte_offset,
                target_offset: slot.byte_offset,
            },
        );
    }

    if let Some(target) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        referent,
    ) {
        return Some(
            SelectedInstructionKind::WriteRuntimeFrameIndexedAddressToRuntimeFrame {
                descriptor_offset: target.descriptor_offset,
                index_offset: target.index_offset,
                element_byte_size: target.element_byte_size,
                field_byte_offset: target.field_byte_offset,
                target_offset: slot.byte_offset,
            },
        );
    }

    if let Some(target) = resolve_runtime_frame_base_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        referent,
    ) {
        return Some(
            SelectedInstructionKind::WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame {
                base_byte_offset: target.base_byte_offset,
                index_offset: target.index_offset,
                element_byte_size: target.element_byte_size,
                field_byte_offset: target.field_byte_offset,
                target_offset: slot.byte_offset,
            },
        );
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        referent,
    ) {
        return Some(
            SelectedInstructionKind::WriteRuntimePointeeAddressToRuntimeFrame {
                pointer_byte_offset: pointer_target.pointer_byte_offset,
                field_byte_offset: pointer_target.field_byte_offset,
                target_offset: slot.byte_offset,
            },
        );
    }

    let source_place = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        referent,
    )?;
    Some(
        SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            source_region: source_place.region,
            source_offset: source_place.byte_offset,
            target_offset: slot.byte_offset,
        },
    )
}
