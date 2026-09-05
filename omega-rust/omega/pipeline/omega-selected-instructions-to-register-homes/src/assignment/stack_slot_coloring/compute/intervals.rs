use crate::{
    FunctionLogicalSpillOperations, LogicalSpillStorageClass, LogicalSpillStorageId,
    StackSlotColoringError,
};
use omega_selected_instructions::SelectedBlockId;

use crate::LiveRangePoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::assignment::stack_slot_coloring) struct StackSlotInterval {
    pub(in crate::assignment::stack_slot_coloring) storage: LogicalSpillStorageId,
    pub(in crate::assignment::stack_slot_coloring) class: LogicalSpillStorageClass,
    pub(in crate::assignment::stack_slot_coloring) block: SelectedBlockId,
    pub(in crate::assignment::stack_slot_coloring) live_from: LiveRangePoint,
    pub(in crate::assignment::stack_slot_coloring) live_through: LiveRangePoint,
}

pub(super) fn intervals_for_function(
    function: usize,
    logical: &FunctionLogicalSpillOperations,
) -> Result<Vec<StackSlotInterval>, StackSlotColoringError> {
    let Some(action) = logical.action.as_ref() else {
        return Ok(Vec::new());
    };
    let storage = action.storage.id;
    if action.storage.class != LogicalSpillStorageClass::NonAddressUnsignedU64V1 {
        return Err(StackSlotColoringError::UnsupportedStorageClass { function, storage });
    }
    let Some(first_rewrite) = action.rewrites.first() else {
        return Err(StackSlotColoringError::InvalidInterval { function, storage });
    };
    if action.store.source != action.victim
        || action.store.storage != storage
        || action.reload.storage != storage
        || action.reload.before_instruction != first_rewrite.instruction
        || first_rewrite.result != action.reload.result
        || action
            .rewrites
            .iter()
            .any(|rewrite| rewrite.block != action.block || rewrite.result != action.reload.result)
    {
        return Err(StackSlotColoringError::InvalidLogicalAction { function, storage });
    }
    if action.pressure_point > first_rewrite.point
        || action
            .rewrites
            .iter()
            .any(|rewrite| rewrite.point < action.pressure_point)
        || action.rewrites.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(StackSlotColoringError::InvalidInterval { function, storage });
    }
    Ok(vec![StackSlotInterval {
        storage,
        class: action.storage.class,
        block: action.block,
        live_from: action.pressure_point,
        live_through: first_rewrite.point,
    }])
}
