#![forbid(unsafe_code)]

//! Register homes to the post-allocation machine plan.
//!
//! The stage operation is `stage_optimized_post_allocation_machine_plan`
//! (`post_allocation_machine.rs`). It takes the current-program view the
//! allocation phase supplies, analyzes current machine facts only, and seals
//! a plan that keeps allocation evidence separately. The `plan` folder owns
//! deterministic plan construction (`plan::analyze_post_allocation_machine_plan`)
//! and its independent checker (`validate_post_allocation_machine_plan`). This
//! stage never inspects rewrite history or selects a different construction
//! route.

mod plan;
mod post_allocation_machine;

pub use plan::{
    PostAllocationMachineError, PostAllocationMachineReceipt, ValidatedPostAllocationMachinePlan,
};
pub use post_allocation_machine::{
    OptimizedPostAllocationMachinePipelineError,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedPostAllocationMachinePlan,
    stage_optimized_post_allocation_machine_plan,
};
