//! Optimizer module role: fixture. Explicit predecessor stage for realization-only tests.

use register_homes_to_post_allocation_machine::{
    StagedOptimizedPostAllocationMachinePlan, stage_optimized_post_allocation_machine_plan,
};
use selected_instructions_to_register_homes::RetainedAllocation;

/// Production emission accepts the preceding machine stage's output and never
/// rebuilds it. Realization-only fixtures construct that input here explicitly.
pub(crate) fn with_allocated_machine<Output>(
    allocation: RetainedAllocation,
    realize: impl FnOnce(RetainedAllocation, StagedOptimizedPostAllocationMachinePlan) -> Output,
) -> Output {
    let machine = stage_optimized_post_allocation_machine_plan(&allocation.current())
        .expect("fixture allocation produces a checked machine");
    realize(allocation, machine)
}
