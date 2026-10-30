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

/// The one error surface every stage error shares: its variant list is the
/// report, so `Display` names the stage and `Error` adds no behavior beyond
/// what the variants already carry.
macro_rules! stage_error_display {
    ($error:ty, $stage:literal) => {
        impl std::fmt::Display for $error {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "{}: {self:?}", $stage)
            }
        }

        impl std::error::Error for $error {}
    };
}

#[macro_use]
mod accessors;
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
