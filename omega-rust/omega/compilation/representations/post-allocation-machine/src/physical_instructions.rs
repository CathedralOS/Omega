//! The current physical instruction program and its exact input identities.
//!
//! Control flow owns functions and blocks, instructions own chosen machine
//! alternatives, and operands own physical register footprints. The codec
//! authenticates these data; it does not establish that they implement the inputs.

pub mod codec;
pub mod control_flow;
pub mod costs;
pub mod evidence;
pub mod identity;
pub mod instructions;
pub mod operands;

pub(crate) use codec::PostAllocationMachineDecodeError;
pub use control_flow::{PostAllocationMachineBlock, PostAllocationMachineFunction};
pub use costs::{TargetCostModelIdentity, target_cost_model};
pub use evidence::PostAllocationMachineOptimizationCustody;
pub use identity::{PostAllocationMachineIdentity, post_allocation_machine_identity};
pub use instructions::{
    MachineAlternativeChoiceRule, PhysicalAddressOperation, PostAllocationMachineInstruction,
};
pub use operands::{PhysicalOperandFootprint, QualifiedPhysicalRead};

use optimization_core::PostAllocationOptimizationManifestIdentity;
use register_homes::{AllocationLegalityIdentity, RegisterHomeIdentity};
use register_model::{
    PhysicalRegisterModelIdentity, RegisterConstraintCatalogIdentity,
    TargetRegisterEnvironmentIdentity,
};
use selected_instructions::LiveRangeIdentity;
use selected_instructions::{
    MachineEffectCatalogIdentity, PreAllocationMachineEffectIdentity,
    SelectedInstructionPlanIdentity,
};
use target::NativeTarget;

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
}

impl PostAllocationMachinePlan {
    /// The machine function for `machine`, when the plan carries it.
    pub fn function(
        &self,
        machine: semantic_vocabulary::MachineId,
    ) -> Option<&PostAllocationMachineFunction> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
    }

    /// Every function's blocks, in function order.
    pub fn blocks(&self) -> impl Iterator<Item = &PostAllocationMachineBlock> + '_ {
        self.functions.iter().flat_map(|function| &function.blocks)
    }

    /// Every function's physical instructions, in function then block order.
    pub fn instructions(&self) -> impl Iterator<Item = &PostAllocationMachineInstruction> + '_ {
        self.functions
            .iter()
            .flat_map(|function| function.instructions())
    }

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
