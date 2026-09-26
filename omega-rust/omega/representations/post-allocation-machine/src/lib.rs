#![forbid(unsafe_code)]

//! Physical instruction programs after register assignment and before encoding.
//!
//! Start at [`physical_instructions::PostAllocationMachinePlan`]. This crate
//! owns data and canonical encoding, not construction or admission authority.

pub mod physical_instructions;
pub use physical_instructions::{
    Aarch64CbnzFusionIdentity, Aarch64MovnMaterializationIdentity, MachineAlternativeChoiceRule,
    NonAuthoritativeLatencyCost, NonAuthoritativeMachineCost, NonAuthoritativeMachineSizeCost,
    PhysicalAddressOperation, PhysicalOperandFootprint, PostAllocationMachineBlock,
    PostAllocationMachineDecodeError, PostAllocationMachineFunction, PostAllocationMachineIdentity,
    PostAllocationMachineInstruction, PostAllocationMachineOptimizationCustody,
    PostAllocationMachinePlan, QualifiedPhysicalRead, TargetCostModel, TargetCostModelIdentity,
    TargetCostModelVersion, codec, control_flow, costs, evidence, identity, instructions, operands,
    post_allocation_machine_identity, target_cost_model,
};
