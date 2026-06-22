mod dispatch;
mod runtime_writes;

pub(super) use dispatch::{
    byte_distance_to_case_end, byte_distance_to_case_leave, byte_distance_to_dispatch_loop_leave,
    byte_distance_to_dispatch_loop_start, byte_distance_to_next_dispatch_action_end,
};
pub(super) use runtime_writes::{
    byte_distance_to_branch_arms_end, byte_distance_to_next_guarded_effect_end,
    byte_distance_to_next_runtime_write_end, byte_distances_to_next_runtime_machine_write_end,
};
