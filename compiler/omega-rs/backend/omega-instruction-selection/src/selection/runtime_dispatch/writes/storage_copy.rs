use crate::InstructionSelectionInput;
use omega_control_flow::StateKey;
use psi_checked_trees::expression::{Expression, ExpressionHandle, ExpressionTable};

use super::super::super::storage_places::{
    resolve_runtime_frame_base_double_indexed_source_with_index_regions,
    resolve_runtime_frame_base_double_indexed_source_with_index_regions_in_table,
    resolve_runtime_frame_base_indexed_target_with_index_region,
    resolve_runtime_frame_base_indexed_target_with_index_region_in_table,
    resolve_runtime_frame_fixed_indexed_target,
    resolve_runtime_frame_fixed_indexed_target_in_table, resolve_runtime_frame_indexed_target,
    resolve_runtime_frame_indexed_target_in_table,
    resolve_runtime_frame_indexed_target_near_slot_in_table,
    resolve_runtime_machine_double_indexed_source,
    resolve_runtime_machine_double_indexed_source_in_table, resolve_runtime_machine_indexed_target,
    resolve_runtime_machine_indexed_target_in_table, resolve_runtime_pointee_fixed_indexed_target,
    resolve_runtime_pointee_fixed_indexed_target_in_table, resolve_runtime_pointee_slot_offset,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place,
    resolve_runtime_storage_place_in_table,
};
use omega_abstract_operations::RuntimeStorageRegion;
use omega_abstract_operations::{RuntimeValueOperand, SelectedInstructionKind};
use psi_arena::Arena;

#[allow(clippy::too_many_arguments)]
pub(in crate::selection::runtime_dispatch) fn runtime_stored_integer_projection_copy_in_table(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    target_source_key: StateKey,
    value_source_key: StateKey,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
) -> Option<SelectedInstructionKind> {
    let target = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    )?;
    super::mutation::select_runtime_stored_integer_projection_write_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
        target.region,
        target.byte_offset,
        target.byte_count,
        runtime_value_operands,
    )
}

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

    // A member read through a REFERENCE-typed slot is a DEREF, never a flat
    // offset: `table.con_out` with `table: &EfiSystemTable` reads
    // [*(frame+slot) + 64]. Without this arm the flat resolver below folded
    // slot_base + field_offset and read frame garbage silently
    // (shared-ref-param-field-read gap). The generalized pointee copy lands
    // in either region.
    if matches!(
        target_place.region,
        RuntimeStorageRegion::RuntimeFrame | RuntimeStorageRegion::Machine
    ) && let Some(pointee) =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, value_source_key, value)
        && pointee.pointee_byte_size == target_place.byte_count
        && pointee.pointee_byte_size > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_from_pointee(
                pointee.pointer_byte_offset,
                pointee.field_byte_offset,
                target_place.region,
                target_place.byte_offset,
                target_place.byte_count,
            ),
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
    if target_place.byte_count != source_place.byte_count || target_place.byte_count == 0 {
        return None;
    }

    // The first migrated CopyPlaces site (Phase 6 rung 2): addressing rides
    // the Place operands; the materializer emits byte-for-byte what the
    // CopyRuntimeStorage encoder emitted for this direct pair, and the
    // relocation walker patches each base from the place's own region.
    Some(crate::selection::runtime_dispatch::copy_places_direct(
        source_place.region,
        source_place.byte_offset,
        target_place.region,
        target_place.byte_offset,
        target_place.byte_count,
    ))
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

    // The retired ToFrame/ToStorage split collapses: the target region rides
    // the place (rung 2c-iv).
    Some(
        crate::selection::runtime_dispatch::copy_places_from_fixed_indexed(
            fixed_source.descriptor_offset,
            fixed_source.element_index,
            fixed_source.element_byte_size,
            fixed_source.field_byte_offset,
            target_place.region,
            target_place.byte_offset,
            target_place.byte_count,
        ),
    )
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

        // The retired ToFrame/ToStorage split collapses: the target region
        // rides the place (rung 2c-v).
        let kind = crate::selection::runtime_dispatch::copy_places_from_indexed(
            indexed_source.descriptor_offset,
            indexed_source.index_region,
            indexed_source.index_offset,
            indexed_source.index_byte_size,
            indexed_source.element_byte_size,
            indexed_source.field_byte_offset,
            target_place.region,
            target_place.byte_offset,
            target_place.byte_count,
        );

        return Some(kind);
    }

    if let Some(indexed_source) =
        resolve_runtime_machine_indexed_target(input, dispatch_index, value_source_key, value)
    {
        if target_place.byte_count != indexed_source.byte_count {
            return None;
        }

        return Some(
            crate::selection::runtime_dispatch::copy_places_from_machine_indexed(
                indexed_source.base_byte_offset,
                indexed_source.index_region,
                indexed_source.index_offset,
                indexed_source.index_byte_size,
                indexed_source.element_byte_size,
                indexed_source.field_byte_offset,
                target_place.region,
                target_place.byte_offset,
                target_place.byte_count,
            ),
        );
    }

    // KEEP IN SYNC with `runtime_storage_indexed_source_copy_in_table` (the
    // parallel-cascade rule): the double-indexed arms, frame flavor first.
    if let Some(double_source) = resolve_runtime_frame_base_double_indexed_source_with_index_regions(
        input,
        dispatch_index,
        value_source_key,
        value,
    ) {
        if target_place.byte_count != double_source.byte_count {
            return None;
        }
        return Some(
            crate::selection::runtime_dispatch::copy_places_from_frame_base_double_indexed(
                double_source.base_byte_offset,
                double_source.outer_index_region,
                double_source.outer_index_offset,
                double_source.outer_index_byte_size,
                double_source.outer_stride,
                double_source.inner_index_region,
                double_source.inner_index_offset,
                double_source.inner_index_byte_size,
                double_source.inner_stride,
                double_source.field_byte_offset,
                target_place.region,
                target_place.byte_offset,
                target_place.byte_count,
            ),
        );
    }

    let double_source = resolve_runtime_machine_double_indexed_source(
        input,
        dispatch_index,
        value_source_key,
        value,
    )?;
    if target_place.byte_count != double_source.byte_count {
        return None;
    }
    Some(
        crate::selection::runtime_dispatch::copy_places_from_machine_double_indexed(
            double_source.base_byte_offset,
            double_source.outer_index_region,
            double_source.outer_index_offset,
            double_source.outer_index_byte_size,
            double_source.outer_stride,
            double_source.inner_index_region,
            double_source.inner_index_offset,
            double_source.inner_index_byte_size,
            double_source.inner_stride,
            double_source.field_byte_offset,
            target_place.region,
            target_place.byte_offset,
            target_place.byte_count,
        ),
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

    // Keep the table path semantically identical to `runtime_storage_copy`:
    // a member of an address-bearing reference parameter is read through the
    // pointer. Resolving it as a flat place first turns `r.value` into
    // `frame_slot + field_offset`, which reads bytes beyond the pointer slot.
    if matches!(
        target_place.region,
        RuntimeStorageRegion::RuntimeFrame | RuntimeStorageRegion::Machine
    ) && let Some(pointee) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && pointee.pointee_byte_size == target_place.byte_count
        && pointee.pointee_byte_size > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_from_pointee(
                pointee.pointer_byte_offset,
                pointee.field_byte_offset,
                target_place.region,
                target_place.byte_offset,
                target_place.byte_count,
            ),
        );
    }

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

    // The in-table twin of the migrated CopyPlaces site above (rung 2):
    // identical direct-pair shape, same byte-for-byte materialization.
    Some(crate::selection::runtime_dispatch::copy_places_direct(
        source_place.region,
        source_place.byte_offset,
        target_place.region,
        target_place.byte_offset,
        target_place.byte_count,
    ))
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
    if let Some(pointer_source) =
        resolve_runtime_pointee_slot_offset(input, dispatch_index, value_source_key, value)
        && let Some(pointer_target) =
            resolve_runtime_pointee_slot_offset(input, dispatch_index, target_source_key, target)
        && pointer_source.pointee_byte_size == pointer_target.pointee_byte_size
        && pointer_source.pointee_byte_size > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_pointee_pair(
                pointer_source.pointer_byte_offset,
                pointer_source.field_byte_offset,
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                pointer_source.pointee_byte_size,
            ),
        );
    }

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
            crate::selection::runtime_dispatch::copy_places_fixed_indexed_to_pointee(
                fixed_source.descriptor_offset,
                fixed_source.element_index,
                fixed_source.element_byte_size,
                fixed_source.field_byte_offset,
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                fixed_source.byte_count,
            ),
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
        return Some(crate::selection::runtime_dispatch::copy_places_to_pointee(
            source_place.region,
            source_place.byte_offset,
            pointer_target.pointer_byte_offset,
            pointer_target.field_byte_offset,
            source_place.byte_count,
        ));
    }

    if let Some(pointer_target) = resolve_runtime_pointee_fixed_indexed_target(
        input,
        dispatch_index,
        target_source_key,
        target,
    ) && source_place.byte_count > 0
    {
        return Some(crate::selection::runtime_dispatch::copy_places_to_pointee(
            source_place.region,
            source_place.byte_offset,
            pointer_target.pointer_byte_offset,
            pointer_target.field_byte_offset,
            source_place.byte_count,
        ));
    }

    if let Some(double_target) = resolve_runtime_frame_base_double_indexed_source_with_index_regions(
        input,
        dispatch_index,
        target_source_key,
        target,
    ) && source_place.byte_count == double_target.byte_count
        && source_place.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_to_frame_base_double_indexed(
                source_place.region,
                source_place.byte_offset,
                double_target.base_byte_offset,
                double_target.outer_index_region,
                double_target.outer_index_offset,
                double_target.outer_index_byte_size,
                double_target.outer_stride,
                double_target.inner_index_region,
                double_target.inner_index_offset,
                double_target.inner_index_byte_size,
                double_target.inner_stride,
                double_target.field_byte_offset,
                double_target.byte_count,
            ),
        );
    }

    if let Some(indexed_target) = resolve_runtime_frame_base_indexed_target_with_index_region(
        input,
        dispatch_index,
        target_source_key,
        target,
    ) && source_place.byte_count == indexed_target.byte_count
        && source_place.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_to_frame_base_indexed(
                source_place.region,
                source_place.byte_offset,
                indexed_target.base_byte_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                indexed_target.byte_count,
            ),
        );
    }

    let indexed_target =
        resolve_runtime_frame_indexed_target(input, dispatch_index, target_source_key, target)?;
    if source_place.byte_count != indexed_target.byte_count {
        return None;
    }

    Some(crate::selection::runtime_dispatch::copy_places_to_indexed(
        source_place.region,
        source_place.byte_offset,
        indexed_target.descriptor_offset,
        indexed_target.index_region,
        indexed_target.index_offset,
        indexed_target.index_byte_size,
        indexed_target.element_byte_size,
        indexed_target.field_byte_offset,
        indexed_target.byte_count,
    ))
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
    if let Some(pointer_source) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && pointer_source.pointee_byte_size == pointer_target.pointee_byte_size
        && pointer_source.pointee_byte_size > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_pointee_pair(
                pointer_source.pointer_byte_offset,
                pointer_source.field_byte_offset,
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                pointer_source.pointee_byte_size,
            ),
        );
    }

    if let Some(pointer_source) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && let Some(double_target) =
        resolve_runtime_frame_base_double_indexed_source_with_index_regions_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
        && pointer_source.pointee_byte_size == double_target.byte_count
        && double_target.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_pointee_to_frame_base_double_indexed(
                pointer_source.pointer_byte_offset,
                pointer_source.field_byte_offset,
                double_target.base_byte_offset,
                double_target.outer_index_region,
                double_target.outer_index_offset,
                double_target.outer_index_byte_size,
                double_target.outer_stride,
                double_target.inner_index_region,
                double_target.inner_index_offset,
                double_target.inner_index_byte_size,
                double_target.inner_stride,
                double_target.field_byte_offset,
                double_target.byte_count,
            ),
        );
    }

    if let Some(pointer_source) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && let Some(indexed_target) =
        resolve_runtime_frame_base_indexed_target_with_index_region_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
        && pointer_source.pointee_byte_size == indexed_target.byte_count
        && indexed_target.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_pointee_to_frame_base_indexed(
                pointer_source.pointer_byte_offset,
                pointer_source.field_byte_offset,
                indexed_target.base_byte_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                indexed_target.byte_count,
            ),
        );
    }

    if let Some(pointer_source) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && let Some(indexed_target) = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && pointer_source.pointee_byte_size == indexed_target.byte_count
        && indexed_target.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_pointee_to_machine_indexed(
                pointer_source.pointer_byte_offset,
                pointer_source.field_byte_offset,
                indexed_target.base_byte_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                indexed_target.byte_count,
            ),
        );
    }

    if let Some(pointer_source) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && let Some(double_target) = resolve_runtime_machine_double_indexed_source_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && pointer_source.pointee_byte_size == double_target.byte_count
        && double_target.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_pointee_to_machine_double_indexed(
                pointer_source.pointer_byte_offset,
                pointer_source.field_byte_offset,
                double_target.base_byte_offset,
                double_target.outer_index_region,
                double_target.outer_index_offset,
                double_target.outer_index_byte_size,
                double_target.outer_stride,
                double_target.inner_index_region,
                double_target.inner_index_offset,
                double_target.inner_index_byte_size,
                double_target.inner_stride,
                double_target.field_byte_offset,
                double_target.byte_count,
            ),
        );
    }

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
        return Some(crate::selection::runtime_dispatch::copy_places_to_pointee(
            source_place.region,
            source_place.byte_offset,
            pointer_target.pointer_byte_offset,
            pointer_target.field_byte_offset,
            source_place.byte_count,
        ));
    }

    if let Some(pointer_target) = resolve_runtime_pointee_fixed_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && source_place.byte_count > 0
    {
        return Some(crate::selection::runtime_dispatch::copy_places_to_pointee(
            source_place.region,
            source_place.byte_offset,
            pointer_target.pointer_byte_offset,
            pointer_target.field_byte_offset,
            source_place.byte_count,
        ));
    }

    if let Some(double_target) =
        resolve_runtime_frame_base_double_indexed_source_with_index_regions_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
        && source_place.byte_count == double_target.byte_count
        && source_place.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_to_frame_base_double_indexed(
                source_place.region,
                source_place.byte_offset,
                double_target.base_byte_offset,
                double_target.outer_index_region,
                double_target.outer_index_offset,
                double_target.outer_index_byte_size,
                double_target.outer_stride,
                double_target.inner_index_region,
                double_target.inner_index_offset,
                double_target.inner_index_byte_size,
                double_target.inner_stride,
                double_target.field_byte_offset,
                double_target.byte_count,
            ),
        );
    }

    if let Some(indexed_target) =
        resolve_runtime_frame_base_indexed_target_with_index_region_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
        && source_place.byte_count == indexed_target.byte_count
        && source_place.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_to_frame_base_indexed(
                source_place.region,
                source_place.byte_offset,
                indexed_target.base_byte_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                indexed_target.byte_count,
            ),
        );
    }

    let indexed_target = resolve_runtime_frame_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    )?;
    if source_place.byte_count != indexed_target.byte_count {
        return None;
    }

    Some(crate::selection::runtime_dispatch::copy_places_to_indexed(
        source_place.region,
        source_place.byte_offset,
        indexed_target.descriptor_offset,
        indexed_target.index_region,
        indexed_target.index_offset,
        indexed_target.index_byte_size,
        indexed_target.element_byte_size,
        indexed_target.field_byte_offset,
        indexed_target.byte_count,
    ))
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
    // `&mut array[i]` constructs a reference value; it is not a value copy
    // from the indexed element into an already-established pointee. Leave the
    // explicit Mutable wrapper for the address-write selector.
    if matches!(
        expressions.expression(value),
        psi_checked_trees::expression::ExpressionNode::Mutable(_)
    ) {
        return None;
    }
    if let Some(double_target) = resolve_runtime_machine_double_indexed_source_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && let Some(double_source) = resolve_runtime_machine_double_indexed_source_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && double_source.byte_count == double_target.byte_count
        && double_target.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_machine_double_indexed_pair(
                double_source.base_byte_offset,
                double_source.outer_index_region,
                double_source.outer_index_offset,
                double_source.outer_index_byte_size,
                double_source.outer_stride,
                double_source.inner_index_region,
                double_source.inner_index_offset,
                double_source.inner_index_byte_size,
                double_source.inner_stride,
                double_source.field_byte_offset,
                double_target.base_byte_offset,
                double_target.outer_index_region,
                double_target.outer_index_offset,
                double_target.outer_index_byte_size,
                double_target.outer_stride,
                double_target.inner_index_region,
                double_target.inner_index_offset,
                double_target.inner_index_byte_size,
                double_target.inner_stride,
                double_target.field_byte_offset,
                double_target.byte_count,
            ),
        );
    }

    if let Some(double_target) =
        resolve_runtime_frame_base_double_indexed_source_with_index_regions_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
        && let Some(double_source) =
            resolve_runtime_frame_base_double_indexed_source_with_index_regions_in_table(
                input,
                dispatch_index,
                value_source_key,
                expressions,
                value,
            )
        && double_source.byte_count == double_target.byte_count
        && double_target.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_frame_base_double_indexed_pair(
                double_source.base_byte_offset,
                double_source.outer_index_region,
                double_source.outer_index_offset,
                double_source.outer_index_byte_size,
                double_source.outer_stride,
                double_source.inner_index_region,
                double_source.inner_index_offset,
                double_source.inner_index_byte_size,
                double_source.inner_stride,
                double_source.field_byte_offset,
                double_target.base_byte_offset,
                double_target.outer_index_region,
                double_target.outer_index_offset,
                double_target.outer_index_byte_size,
                double_target.outer_stride,
                double_target.inner_index_region,
                double_target.inner_index_offset,
                double_target.inner_index_byte_size,
                double_target.inner_stride,
                double_target.field_byte_offset,
                double_target.byte_count,
            ),
        );
    }

    if let Some(double_target) =
        resolve_runtime_frame_base_double_indexed_source_with_index_regions_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
        && let Some(double_source) = resolve_runtime_machine_double_indexed_source_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            value,
        )
        && double_source.byte_count == double_target.byte_count
        && double_target.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_cross_region_double_indexed_pair(
                RuntimeStorageRegion::Machine,
                double_source.base_byte_offset,
                double_source.outer_index_region,
                double_source.outer_index_offset,
                double_source.outer_index_byte_size,
                double_source.outer_stride,
                double_source.inner_index_region,
                double_source.inner_index_offset,
                double_source.inner_index_byte_size,
                double_source.inner_stride,
                double_source.field_byte_offset,
                RuntimeStorageRegion::RuntimeFrame,
                double_target.base_byte_offset,
                double_target.outer_index_region,
                double_target.outer_index_offset,
                double_target.outer_index_byte_size,
                double_target.outer_stride,
                double_target.inner_index_region,
                double_target.inner_index_offset,
                double_target.inner_index_byte_size,
                double_target.inner_stride,
                double_target.field_byte_offset,
                double_target.byte_count,
            ),
        );
    }

    if let Some(double_target) = resolve_runtime_machine_double_indexed_source_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && let Some(double_source) =
        resolve_runtime_frame_base_double_indexed_source_with_index_regions_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            value,
        )
        && double_source.byte_count == double_target.byte_count
        && double_target.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_cross_region_double_indexed_pair(
                RuntimeStorageRegion::RuntimeFrame,
                double_source.base_byte_offset,
                double_source.outer_index_region,
                double_source.outer_index_offset,
                double_source.outer_index_byte_size,
                double_source.outer_stride,
                double_source.inner_index_region,
                double_source.inner_index_offset,
                double_source.inner_index_byte_size,
                double_source.inner_stride,
                double_source.field_byte_offset,
                RuntimeStorageRegion::Machine,
                double_target.base_byte_offset,
                double_target.outer_index_region,
                double_target.outer_index_offset,
                double_target.outer_index_byte_size,
                double_target.outer_stride,
                double_target.inner_index_region,
                double_target.inner_index_offset,
                double_target.inner_index_byte_size,
                double_target.inner_stride,
                double_target.field_byte_offset,
                double_target.byte_count,
            ),
        );
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && let Some(double_source) = resolve_runtime_machine_double_indexed_source_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && pointer_target.pointee_byte_size == double_source.byte_count
        && double_source.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_machine_double_indexed_to_pointee(
                double_source.base_byte_offset,
                double_source.outer_index_region,
                double_source.outer_index_offset,
                double_source.outer_index_byte_size,
                double_source.outer_stride,
                double_source.inner_index_region,
                double_source.inner_index_offset,
                double_source.inner_index_byte_size,
                double_source.inner_stride,
                double_source.field_byte_offset,
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                double_source.byte_count,
            ),
        );
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && let Some(double_source) =
        resolve_runtime_frame_base_double_indexed_source_with_index_regions_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            value,
        )
        && pointer_target.pointee_byte_size == double_source.byte_count
        && double_source.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_frame_base_double_indexed_to_pointee(
                double_source.base_byte_offset,
                double_source.outer_index_region,
                double_source.outer_index_offset,
                double_source.outer_index_byte_size,
                double_source.outer_stride,
                double_source.inner_index_region,
                double_source.inner_index_offset,
                double_source.inner_index_byte_size,
                double_source.inner_stride,
                double_source.field_byte_offset,
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                double_source.byte_count,
            ),
        );
    }

    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && let Some(indexed_source) =
        resolve_runtime_frame_base_indexed_target_with_index_region_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            value,
        )
        && pointer_target.pointee_byte_size == indexed_source.byte_count
        && indexed_source.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_frame_base_indexed_to_pointee(
                indexed_source.base_byte_offset,
                indexed_source.index_region,
                indexed_source.index_offset,
                indexed_source.index_byte_size,
                indexed_source.element_byte_size,
                indexed_source.field_byte_offset,
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                indexed_source.byte_count,
            ),
        );
    }

    if let Some(indexed_target) =
        resolve_runtime_frame_base_indexed_target_with_index_region_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
        && let Some(indexed_source) =
            resolve_runtime_frame_base_indexed_target_with_index_region_in_table(
                input,
                dispatch_index,
                value_source_key,
                expressions,
                value,
            )
        && indexed_source.byte_count == indexed_target.byte_count
        && indexed_target.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_frame_base_indexed_pair(
                indexed_source.base_byte_offset,
                indexed_source.index_region,
                indexed_source.index_offset,
                indexed_source.index_byte_size,
                indexed_source.element_byte_size,
                indexed_source.field_byte_offset,
                indexed_target.base_byte_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                indexed_target.byte_count,
            ),
        );
    }

    if let Some(indexed_target) =
        resolve_runtime_frame_base_indexed_target_with_index_region_in_table(
            input,
            dispatch_index,
            target_source_key,
            expressions,
            target,
        )
        && let Some(indexed_source) = resolve_runtime_machine_indexed_target_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            value,
        )
        && indexed_source.byte_count == indexed_target.byte_count
        && indexed_target.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_cross_region_indexed_pair(
                RuntimeStorageRegion::Machine,
                indexed_source.base_byte_offset,
                indexed_source.index_region,
                indexed_source.index_offset,
                indexed_source.index_byte_size,
                indexed_source.element_byte_size,
                indexed_source.field_byte_offset,
                RuntimeStorageRegion::RuntimeFrame,
                indexed_target.base_byte_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                indexed_target.byte_count,
            ),
        );
    }

    if let Some(indexed_target) = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        target_source_key,
        expressions,
        target,
    ) && let Some(indexed_source) =
        resolve_runtime_frame_base_indexed_target_with_index_region_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            value,
        )
        && indexed_source.byte_count == indexed_target.byte_count
        && indexed_target.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_cross_region_indexed_pair(
                RuntimeStorageRegion::RuntimeFrame,
                indexed_source.base_byte_offset,
                indexed_source.index_region,
                indexed_source.index_offset,
                indexed_source.index_byte_size,
                indexed_source.element_byte_size,
                indexed_source.field_byte_offset,
                RuntimeStorageRegion::Machine,
                indexed_target.base_byte_offset,
                indexed_target.index_region,
                indexed_target.index_offset,
                indexed_target.index_byte_size,
                indexed_target.element_byte_size,
                indexed_target.field_byte_offset,
                indexed_target.byte_count,
            ),
        );
    }

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
    ) && let Some(indexed_source) = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) && pointer_target.pointee_byte_size == indexed_source.byte_count
        && indexed_source.byte_count > 0
    {
        return Some(
            crate::selection::runtime_dispatch::copy_places_machine_indexed_to_pointee(
                indexed_source.base_byte_offset,
                indexed_source.index_region,
                indexed_source.index_offset,
                indexed_source.index_byte_size,
                indexed_source.element_byte_size,
                indexed_source.field_byte_offset,
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                indexed_source.byte_count,
            ),
        );
    }

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
            crate::selection::runtime_dispatch::copy_places_indexed_to_pointee(
                indexed_source.descriptor_offset,
                indexed_source.index_region,
                indexed_source.index_offset,
                indexed_source.index_byte_size,
                indexed_source.element_byte_size,
                indexed_source.field_byte_offset,
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                indexed_source.byte_count,
            ),
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

        // The retired ToFrame/ToStorage split collapses: the target region
        // rides the place (rung 2c-v).
        let kind = crate::selection::runtime_dispatch::copy_places_from_indexed(
            indexed_source.descriptor_offset,
            indexed_source.index_region,
            indexed_source.index_offset,
            indexed_source.index_byte_size,
            indexed_source.element_byte_size,
            indexed_source.field_byte_offset,
            target_place.region,
            target_place.byte_offset,
            target_place.byte_count,
        );

        return Some(kind);
    }

    if let Some(indexed_source) = resolve_runtime_machine_indexed_target_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    ) {
        if target_place.byte_count != indexed_source.byte_count {
            return None;
        }

        return Some(
            crate::selection::runtime_dispatch::copy_places_from_machine_indexed(
                indexed_source.base_byte_offset,
                indexed_source.index_region,
                indexed_source.index_offset,
                indexed_source.index_byte_size,
                indexed_source.element_byte_size,
                indexed_source.field_byte_offset,
                target_place.region,
                target_place.byte_offset,
                target_place.byte_count,
            ),
        );
    }

    // BOTH-RUNTIME nested read of a FRAME-resident 2D array (`g[i][j]`) into
    // a runtime-storage target.
    if let Some(double_source) =
        resolve_runtime_frame_base_double_indexed_source_with_index_regions_in_table(
            input,
            dispatch_index,
            value_source_key,
            expressions,
            value,
        )
    {
        if target_place.byte_count != double_source.byte_count {
            return None;
        }
        return Some(
            crate::selection::runtime_dispatch::copy_places_from_frame_base_double_indexed(
                double_source.base_byte_offset,
                double_source.outer_index_region,
                double_source.outer_index_offset,
                double_source.outer_index_byte_size,
                double_source.outer_stride,
                double_source.inner_index_region,
                double_source.inner_index_offset,
                double_source.inner_index_byte_size,
                double_source.inner_stride,
                double_source.field_byte_offset,
                target_place.region,
                target_place.byte_offset,
                target_place.byte_count,
            ),
        );
    }

    // BOTH-RUNTIME nested read (`grid[i][j]`): the double-indexed op carries
    // the (row, element) index pair.
    let double_source = resolve_runtime_machine_double_indexed_source_in_table(
        input,
        dispatch_index,
        value_source_key,
        expressions,
        value,
    )?;
    if target_place.byte_count != double_source.byte_count {
        return None;
    }

    Some(
        crate::selection::runtime_dispatch::copy_places_from_machine_double_indexed(
            double_source.base_byte_offset,
            double_source.outer_index_region,
            double_source.outer_index_offset,
            double_source.outer_index_byte_size,
            double_source.outer_stride,
            double_source.inner_index_region,
            double_source.inner_index_offset,
            double_source.inner_index_byte_size,
            double_source.inner_stride,
            double_source.field_byte_offset,
            target_place.region,
            target_place.byte_offset,
            target_place.byte_count,
        ),
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

    // The retired ToFrame/ToStorage split collapses: the target region rides
    // the place (rung 2c-iv).
    Some(
        crate::selection::runtime_dispatch::copy_places_from_fixed_indexed(
            fixed_source.descriptor_offset,
            fixed_source.element_index,
            fixed_source.element_byte_size,
            fixed_source.field_byte_offset,
            target_place.region,
            target_place.byte_offset,
            target_place.byte_count,
        ),
    )
}
