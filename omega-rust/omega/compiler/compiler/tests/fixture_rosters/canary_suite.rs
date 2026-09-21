//! Checked-tree fixture inputs owned by the root canary-suite tests.

pub const BOUNDARY_EQUALITY_RECAST_WITNESS_COMPILE: &str =
    "dependent/boundary_equality_recast_witness_compile";
pub const TASK_RUNTIME_MACHINE_SELECTION_COMPILE: &str =
    "tasks/task_runtime_machine_selection_compile";
pub const ALLOC_BUMP_ALLOCATOR_CANARY: &str = "memory/bump_allocator_canary";

pub const PASS_CANARIES: &[&str] = &[
    BOUNDARY_EQUALITY_RECAST_WITNESS_COMPILE,
    TASK_RUNTIME_MACHINE_SELECTION_COMPILE,
    ALLOC_BUMP_ALLOCATOR_CANARY,
];
