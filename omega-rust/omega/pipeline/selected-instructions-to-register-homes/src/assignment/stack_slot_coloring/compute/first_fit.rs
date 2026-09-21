use semantic_vocabulary::MachineId;

use crate::{FunctionStackSlotColoring, StackSlotAssignment, StackSlotColoringError};

use super::StackSlotInterval;

const SLOT_BYTES: u64 = 8;

pub(in crate::assignment::stack_slot_coloring) fn color_intervals_first_fit(
    function: usize,
    machine: MachineId,
    mut intervals: Vec<StackSlotInterval>,
) -> Result<FunctionStackSlotColoring, StackSlotColoringError> {
    intervals.sort_by_key(|interval| {
        (
            interval.block.0,
            interval.live_from.0,
            interval.live_through.0,
            interval.storage.0,
        )
    });
    let mut storage = intervals
        .iter()
        .map(|interval| interval.storage)
        .collect::<Vec<_>>();
    storage.sort();
    if let Some(pair) = storage.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(StackSlotColoringError::DuplicateStorage {
            function,
            storage: pair[0],
        });
    }
    let mut assignments: Vec<StackSlotAssignment> = Vec::with_capacity(intervals.len());
    for interval in intervals {
        let mut offset = 0_u64;
        loop {
            let conflicts = assignments.iter().any(|assigned| {
                assigned.spill_area_offset == offset
                    && assigned.block == interval.block
                    && assigned.live_from <= interval.live_through
                    && interval.live_from <= assigned.live_through
            });
            if !conflicts {
                break;
            }
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
        machine,
        assignments,
        spill_area_bytes,
    })
}
