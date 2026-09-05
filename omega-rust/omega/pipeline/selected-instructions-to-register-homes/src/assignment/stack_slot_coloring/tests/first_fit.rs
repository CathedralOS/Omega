use crate::assignment::stack_slot_coloring::compute::{
    StackSlotInterval, color_intervals_first_fit,
};
use crate::*;
use selected_instructions::SelectedBlockId;
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

#[test]
fn disjoint_intervals_reuse_the_first_slot() {
    assert_eq!(
        offsets(vec![interval(0, 0, 1, 2), interval(1, 0, 3, 4)]),
        [0, 0]
    );
}

#[test]
fn touching_closed_interval_endpoints_conflict() {
    assert_eq!(
        offsets(vec![interval(0, 0, 1, 3), interval(1, 0, 3, 5)]),
        [0, 8]
    );
}

#[test]
fn distinct_blocks_may_reuse_the_first_slot() {
    assert_eq!(
        offsets(vec![interval(0, 0, 1, 5), interval(1, 1, 1, 5)]),
        [0, 0]
    );
}
