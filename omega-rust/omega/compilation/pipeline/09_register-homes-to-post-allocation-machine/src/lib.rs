#![forbid(unsafe_code)]

mod machine;

pub use machine::{
    OptimizedPostAllocationMachinePipelineError, StagedOptimizedPostAllocationMachinePlan,
    stage_optimized_post_allocation_machine_plan,
};
