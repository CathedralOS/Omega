use crate::assignment::stack_slot_coloring::compute::{
    StackSlotInterval, color_intervals_first_fit,
};
use register_homes::{LogicalSpillStorageClass, LogicalSpillStorageId};
use selected_instructions::{LiveRangePoint, SelectedBlockId};
use semantic_vocabulary::MachineId;

fn interval(storage: u32, block: u32, from: u32, through: u32) -> StackSlotInterval {
    StackSlotInterval {
        storage: LogicalSpillStorageId(storage),
        class: LogicalSpillStorageClass::NonAddressUnsignedU64V1,
        block: SelectedBlockId(block),
        live_from: LiveRangePoint(from),
        live_through: LiveRangePoint(through),
    }
}

fn offsets(intervals: Vec<StackSlotInterval>) -> Vec<u64> {
    color_intervals_first_fit(0, MachineId::new(1).unwrap(), intervals)
        .unwrap()
        .assignments
        .iter()
        .map(|assignment| assignment.spill_area_offset)
        .collect()
}

#[test]
fn overlapping_closed_intervals_use_distinct_slots() {
    assert_eq!(
        offsets(vec![interval(0, 0, 1, 4), interval(1, 0, 3, 6)]),
        [0, 8]
    );
}
