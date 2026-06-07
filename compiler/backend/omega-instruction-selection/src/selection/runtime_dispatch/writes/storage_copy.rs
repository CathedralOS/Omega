use crate::InstructionSelectionInput;
use omega_checked_trees::expression::{Expression, ExpressionHandle, ExpressionTable};
use omega_control_flow::StateKey;

use super::super::super::storage_places::{
    resolve_runtime_frame_fixed_indexed_target,
    resolve_runtime_frame_fixed_indexed_target_in_table, resolve_runtime_frame_indexed_target,
    resolve_runtime_frame_indexed_target_in_table,
    resolve_runtime_frame_indexed_target_near_slot_in_table,
    resolve_runtime_machine_indexed_target, resolve_runtime_machine_indexed_target_in_table,
    resolve_runtime_pointee_fixed_indexed_target,
    resolve_runtime_pointee_fixed_indexed_target_in_table, resolve_runtime_pointee_slot_offset,
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

pub(in crate::selection::runtime_dispatch) fn runtime_storage_fixed_indexed_source_copy(
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

    let fixed_source =
        resolve_runtime_frame_fixed_indexed_target(input, dispatch_index, value_source_key, value)?;
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

pub(in crate::selection::runtime_dispatch) fn runtime_storage_indexed_source_copy(
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

    if let Some(indexed_source) =
        resolve_runtime_frame_indexed_target(input, dispatch_index, value_source_key, value)
    {
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

    let indexed_source =
        resolve_runtime_machine_indexed_target(input, dispatch_index, value_source_key, value)?;
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

pub(in crate::selection::runtime_dispatch) fn runtime_storage_indirect_copy(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    value_source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    target: &Expression,
    value: &Expression,
) -> Option<SelectedInstructionKind> {
    if let Some(fixed_source) =
        resolve_runtime_frame_fixed_indexed_target(input, dispatch_index, value_source_key, value)
        && let Some(pointer_target) = resolve_runtime_pointee_fixed_indexed_target(
            input,
            dispatch_index,
            target_source_key,
            target,
        )
        && fixed_source.byte_count == pointer_target.pointee_byte_size
    {
        return Some(
            SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimePointee {
                descriptor_offset: fixed_source.descriptor_offset,
                element_index: fixed_source.element_index,
                element_byte_size: fixed_source.element_byte_size,
                source_field_byte_offset: fixed_source.field_byte_offset,
                pointer_byte_offset: pointer_target.pointer_byte_offset,
                target_field_byte_offset: pointer_target.field_byte_offset,
                byte_count: fixed_source.byte_count,
            },
        );
    }

    let source_place = resolve_runtime_storage_place(
        input,
        dispatch_index,
        value_source_key,
        source_machine,
        source_state,
        value,
    )?;

    if let Some(pointer_target) =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, target_source_key, target)
        && source_place.byte_count > 0
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

    if let Some(pointer_target) = resolve_runtime_pointee_fixed_indexed_target(
        input,
        dispatch_index,
        target_source_key,
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

    let indexed_target =
        resolve_runtime_frame_indexed_target(input, dispatch_index, target_source_key, target)?;
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
    // `out.f = items[i].f` where `out` is a `&mut` parameter: the target is a
    // pointee (deref `out`) and the source a runtime-frame slice element. Resolve
    // the pointee target BEFORE the plain-place path below -- otherwise the
    // reference slot is treated as inline data (`out_slot + field`) and the value
    // is written INTO the pointer instead of through it. (The dungeon `out.field =
    // rooms[index].field` shape.)
    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && let Some(indexed_source) = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && pointer_target.pointee_byte_size == indexed_source.byte_count
        && indexed_source.byte_count > 0
    {
        return Some(
            SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimePointee {
                descriptor_offset: indexed_source.descriptor_offset,
                index_offset: indexed_source.index_offset,
                element_byte_size: indexed_source.element_byte_size,
                source_field_byte_offset: indexed_source.field_byte_offset,
                pointer_byte_offset: pointer_target.pointer_byte_offset,
                target_field_byte_offset: pointer_target.field_byte_offset,
                byte_count: indexed_source.byte_count,
            },
        );
    }

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

    let indexed_source = resolve_runtime_frame_indexed_target_near_slot_in_table(
        input,
        dispatch_index,
        expressions,
        value,
        target_place.byte_offset,
    )
    .or_else(|| {
        resolve_runtime_frame_indexed_target_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            value,
        )
    });
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
