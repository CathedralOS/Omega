use crate::{
    FunctionLogicalSpillOperations, FunctionStackSlotColoring, LogicalSpillStorageClass,
    LogicalSpillStorageId, StackSlotAssignment, StackSlotColoringError,
    ValidatedLogicalSpillOperations,
};
use selected_instructions::SelectedBlockId;

use crate::LiveRangePoint;

const SLOT_BYTES: u64 = 8;

#[derive(Debug, Clone, Copy)]
struct Interval {
    storage: LogicalSpillStorageId,
    class: LogicalSpillStorageClass,
    block: SelectedBlockId,
    live_from: LiveRangePoint,
    live_through: LiveRangePoint,
}

pub(super) fn replay(
    source: &ValidatedLogicalSpillOperations,
) -> Result<Vec<FunctionStackSlotColoring>, StackSlotColoringError> {
    source
        .plan()
        .functions
        .iter()
        .enumerate()
        .map(|(function, logical)| replay_function(function, logical))
        .collect()
}

fn replay_function(
    function: usize,
    logical: &FunctionLogicalSpillOperations,
) -> Result<FunctionStackSlotColoring, StackSlotColoringError> {
    let mut intervals = intervals(function, logical)?;
    intervals.sort_by_key(|interval| {
        (
            interval.block.0,
            interval.live_from.0,
            interval.live_through.0,
            interval.storage.0,
        )
    });
    let mut seen = intervals
        .iter()
        .map(|interval| interval.storage)
        .collect::<Vec<_>>();
    seen.sort();
    if let Some(pair) = seen.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(StackSlotColoringError::DuplicateStorage {
            function,
            storage: pair[0],
        });
    }
    let mut assignments: Vec<StackSlotAssignment> = Vec::with_capacity(intervals.len());
    for interval in intervals {
        let mut offset = 0_u64;
        while assignments.iter().any(|assigned| {
            assigned.spill_area_offset == offset
                && assigned.block == interval.block
                && assigned.live_from <= interval.live_through
                && interval.live_from <= assigned.live_through
        }) {
            offset = offset
                .checked_add(SLOT_BYTES)
                .ok_or(StackSlotColoringError::OffsetOverflow { function })?;
        }
        assignments.push(StackSlotAssignment {
            storage: interval.storage,
            class: interval.class,
            block: interval.block,
            live_from: interval.live_from,
            live_through: interval.live_through,
            size_bytes: SLOT_BYTES,
            alignment_bytes: SLOT_BYTES,
            spill_area_offset: offset,
        });
    }
    let spill_area_bytes = assignments.iter().try_fold(0_u64, |size, assignment| {
        assignment
            .spill_area_offset
            .checked_add(SLOT_BYTES)
            .map(|end| size.max(end))
            .ok_or(StackSlotColoringError::OffsetOverflow { function })
    })?;
    Ok(FunctionStackSlotColoring {
        machine: logical.machine,
        assignments,
        spill_area_bytes,
    })
}

fn intervals(
    function: usize,
    logical: &FunctionLogicalSpillOperations,
) -> Result<Vec<Interval>, StackSlotColoringError> {
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
    Ok(vec![Interval {
        storage,
        class: action.storage.class,
        block: action.block,
        live_from: action.pressure_point,
        live_through: first_rewrite.point,
    }])
}
