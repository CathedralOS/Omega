#![forbid(unsafe_code)]

//! Physical instruction programs after register assignment and before encoding.
//!
//! Start at [`physical_instructions::PostAllocationMachinePlan`]. This crate
//! owns data and canonical encoding, not construction or admission authority.

pub(crate) use optimization_core::codec_bytes;

pub(crate) use optimization_core::digest_encoder;
pub mod physical_instructions;
pub(crate) use physical_instructions::PostAllocationMachineDecodeError;
pub use physical_instructions::{
    MachineAlternativeChoiceRule, PhysicalAddressOperation, PhysicalOperandFootprint,
    PostAllocationMachineBlock, PostAllocationMachineFunction, PostAllocationMachineIdentity,
    PostAllocationMachineInstruction, PostAllocationMachineOptimizationCustody,
    PostAllocationMachinePlan, QualifiedPhysicalRead, TargetCostModelIdentity, codec, control_flow,
    costs, evidence, identity, instructions, operands, post_allocation_machine_identity,
    target_cost_model,
};
