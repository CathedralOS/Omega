#![forbid(unsafe_code)]

/*
We define the physical instruction representation (`PostAllocationMachinePlan`)
that stands between register home assignment and concrete machine-code encoding.

Once register allocation has assigned every virtual register a physical register
home or frame spill slot, instruction selection must resolve each selected
instruction to its unique applicable `MachineAlternative` and materialize
explicit physical register-unit footprints (`storage_units`, `read_units`,
`write_units`, `implicit_unit_uses`, `implicit_unit_defs`, and `implicit_unit_clobbers`).
We store that post-allocation machine program, its domain-separated content
identity (`omega.terminal-postallocation-machine.v27`), and its descriptive
target cost model (`TargetCostModel`).
*/

pub(crate) use ::wire::codec_bytes;
pub(crate) use ::wire::digest_encoder;

pub mod control_flow;
pub mod costs;
pub mod evidence;
pub mod identity;
pub mod instructions;
pub mod operands;
pub mod program;
pub mod program_validation;
pub use program::{PhysicalFunction, PostAllocationMachineProgram};

pub use control_flow::{PostAllocationMachineBlock, PostAllocationMachineFunction};
pub use costs::{TargetCostModelIdentity, target_cost_model};
pub use evidence::{PostAllocationMachineOptimizationCustody, PostAllocationMachinePhaseReceipt};
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
use selected_instructions::{
    LiveRangeIdentity, MachineEffectCatalogIdentity, PreAllocationMachineEffectIdentity,
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
    pub fn function(
        &self,
        machine: semantic_vocabulary::MachineId,
    ) -> Option<&PostAllocationMachineFunction> {
        self.functions
            .iter()
            .find(|function| function.machine == machine)
    }

    pub fn blocks(&self) -> impl Iterator<Item = &PostAllocationMachineBlock> + '_ {
        self.functions.iter().flat_map(|function| &function.blocks)
    }

    pub fn instructions(&self) -> impl Iterator<Item = &PostAllocationMachineInstruction> + '_ {
        self.functions
            .iter()
            .flat_map(|function| function.instructions())
    }
}

pub mod adapters;
