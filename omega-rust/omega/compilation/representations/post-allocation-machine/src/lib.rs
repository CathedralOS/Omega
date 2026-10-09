#![forbid(unsafe_code)]

//! Physical instruction programs after register assignment and before encoding.
//!
//! Start at [`physical_instructions::PostAllocationMachinePlan`]. This crate
//! owns data and canonical encoding, not construction or admission authority.

mod codec_bytes;

mod digest_encoder;
pub mod physical_instructions;
pub use physical_instructions::{
    Aarch64CbnzFusionIdentity, Aarch64MovnMaterializationIdentity, MachineAlternativeChoiceRule,
    PhysicalAddressOperation, PhysicalOperandFootprint, PostAllocationMachineBlock, PostAllocationMachineFunction, PostAllocationMachineIdentity,
    PostAllocationMachineInstruction, PostAllocationMachineOptimizationCustody,
    PostAllocationMachinePlan, QualifiedPhysicalRead, TargetCostModelIdentity, codec, control_flow, costs, evidence, identity, instructions, operands,
    post_allocation_machine_identity, target_cost_model,
};
pub(crate) use physical_instructions::{ 
    PostAllocationMachineDecodeError};
