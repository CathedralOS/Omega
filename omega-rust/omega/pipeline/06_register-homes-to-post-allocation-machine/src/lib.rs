#![forbid(unsafe_code)]

//! Register homes to the post-allocation machine plan.
//!
//! The stage operation is `stage_optimized_post_allocation_machine_plan`
//! (`post_allocation_machine.rs`). It takes the replayed current-program view
//! the allocation phase supplies, analyzes current machine facts only, and
//! seals a plan that keeps allocation evidence separately.
//! `validate_optimized_post_allocation_machine_plan_custody` replays that
//! custody join. The `plan` folder owns deterministic plan construction
//! (`plan::analyze_post_allocation_machine_plan`) and its independent replay
//! (`validate_post_allocation_machine_plan`). This stage never inspects
//! rewrite history or selects a different construction route.

pub mod physical_instructions;
mod plan;
mod post_allocation_machine;

pub use plan::{
    PostAllocationMachineError, PostAllocationMachineReceipt, ValidatedPostAllocationMachinePlan,
    validate_post_allocation_machine_plan,
};
pub use post_allocation_machine::{
    OptimizedPostAllocationMachinePipelineError,
    StagedOptimizedPostAllocationMachineCustodyReceipt, StagedOptimizedPostAllocationMachinePlan,
    stage_optimized_post_allocation_machine_plan,
    validate_optimized_post_allocation_machine_plan_custody,
};

pub use crate::physical_instructions::{
    Aarch64CbnzFusionIdentity, Aarch64MovnMaterializationIdentity, MachineAlternativeChoiceRule,
    NonAuthoritativeLatencyCost, NonAuthoritativeMachineCost, NonAuthoritativeMachineSizeCost,
    PhysicalAddressOperation, PhysicalOperandFootprint, PostAllocationMachineBlock,
    PostAllocationMachineDecodeError, PostAllocationMachineFunction, PostAllocationMachineIdentity,
    PostAllocationMachineInstruction, PostAllocationMachineOptimizationCustody,
    PostAllocationMachinePlan, QualifiedPhysicalRead, TargetCostModel, TargetCostModelIdentity,
    TargetCostModelVersion, codec, control_flow, costs, evidence, identity, instructions, operands,
    post_allocation_machine_identity, target_cost_model,
};
