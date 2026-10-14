#![forbid(unsafe_code)]

//! Register homes to the post-allocation machine plan.
//!
//! The stage operation is `stage_optimized_post_allocation_machine_plan`
//! (`post_allocation_machine.rs`). It takes the current-program view the
//! allocation phase supplies, analyzes current machine facts only, builds the
//! baseline plan under the legality rule, executes the post-allocation
//! optimization phase over it, and seals a plan that keeps allocation
//! evidence separately. The `plan` folder owns deterministic plan
//! construction (`plan::compute_post_allocation_machine_plan`) and sealing;
//! `optimization` owns the phase: its rule table, the shared declared-form
//! rewrite, the unit-liveness guard, and the run evidence realization binds.
//! This stage never inspects rewrite history or selects a different
//! construction route.

mod optimization;
mod plan;
mod post_allocation_machine;

pub use optimization::{
    PostAllocationMachineAction, PostAllocationMachineOptimizationError,
    PostAllocationMachineOptimizationRun, PostAllocationMachineRuleExecution,
};
pub use plan::{
    PostAllocationMachineError, PostAllocationMachineReceipt, ValidatedPostAllocationMachinePlan,
};
pub use post_allocation_machine::{
    OptimizedPostAllocationMachinePipelineError,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedPostAllocationMachinePlan,
    stage_optimized_post_allocation_machine_plan,
};
