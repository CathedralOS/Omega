mod dispatch;
mod runtime_writes;

pub(super) use dispatch::{
    byte_distance_to_case_end, byte_distance_to_case_leave, byte_distance_to_dispatch_loop_start,
    byte_distance_to_next_state_write_end,
};
pub(super) use runtime_writes::{
    byte_distance_to_next_runtime_write_end,
    byte_distance_to_next_runtime_write_end_from_branch_offset,
    byte_distances_to_next_runtime_machine_write_end,
};
