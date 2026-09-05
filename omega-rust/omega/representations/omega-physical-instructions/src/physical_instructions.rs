//! The current physical instruction program and its exact input identities.
//!
//! Control flow owns functions and blocks, instructions own chosen machine
//! alternatives, and operands own physical register footprints. The codec
//! authenticates these data; it does not establish that they implement the inputs.

pub mod codec;
pub mod control_flow;
pub mod evidence;
pub mod identity;
pub mod instructions;
pub mod operands;

pub use codec::PostAllocationMachineDecodeError;
pub use control_flow::*;
pub use evidence::*;
pub use identity::{PostAllocationMachineIdentity, post_allocation_machine_identity};
pub use instructions::*;
pub use operands::*;

use omega_optimization_core::PostAllocationOptimizationManifestIdentity;
use omega_register_homes::{AllocationLegalityIdentity, LiveRangeIdentity, RegisterHomeIdentity};
use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterConstraintCatalogIdentity,
    TargetRegisterEnvironmentIdentity,
};
use omega_selected_instructions::{
    MachineEffectCatalogIdentity, PreAllocationMachineEffectIdentity,
    SelectedInstructionPlanIdentity,
};
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostAllocationMachinePlan {
    pub identity: PostAllocationMachineIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub effects: PreAllocationMachineEffectIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub homes: RegisterHomeIdentity,
    pub post_allocation_manifest: PostAllocationOptimizationManifestIdentity,
    pub target: NativeTarget,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub physical_register_model: PhysicalRegisterModelIdentity,
    pub register_constraints: RegisterConstraintCatalogIdentity,
    pub machine_effect_catalog: MachineEffectCatalogIdentity,
    pub choice_rule: MachineAlternativeChoiceRule,
    pub functions: Vec<PostAllocationMachineFunction>,
    /// Structural-signature Unit functions remain parallel to the ordinary
    /// scalar/VReg roster. Their optional call is one atomic machine effect;
    /// only the ordinary `ReturnUnit` row selects an encoded alternative.
    pub structural_unit_functions: Vec<PostAllocationStructuralUnitFunction>,
}

impl PostAllocationMachinePlan {
    /// Encodes this unchecked plan in the strict, self-authenticating artifact
    /// envelope. This does not grant validation or emission authority.
    pub fn encode(&self) -> Vec<u8> {
        codec::encode_terminal_post_allocation_machine_plan(self)
    }

    /// Decodes and content-authenticates an unchecked plan. Call
    /// an independent post-allocation validator before use.
    pub fn decode(encoded: &[u8]) -> Result<Self, crate::PostAllocationMachineDecodeError> {
        codec::decode_terminal_post_allocation_machine_plan(encoded)
    }
}
