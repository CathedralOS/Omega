/*
  Pre-allocation machine effects of a selected instruction plan
  (`PreAllocationMachineEffectPlan`, `FunctionMachineEffects`,
  `BlockMachineEffects`, `InstructionMachineEffects`) bound to its exact target,
  register-constraint, and machine-effect catalog identities.
*/

pub mod identity;
mod instructions {
    use crate::{
        MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
        MachineMemoryEffect, MachineTrapBehavior, SelectedBlockId, SelectedInstructionId,
        SelectedInstructionKind, SelectedInstructionProvenance,
    };
    use register_model::{RegisterConstraintKey, RegisterUnitId};
    use semantic_vocabulary::MachineId;
    impl FunctionMachineEffects {
        pub fn instructions(&self) -> impl Iterator<Item = &InstructionMachineEffects> + '_ {
            self.blocks.iter().flat_map(|block| &block.instructions)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct FunctionMachineEffects {
        pub machine: MachineId,
        pub blocks: Vec<BlockMachineEffects>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct BlockMachineEffects {
        pub block: SelectedBlockId,
        pub instructions: Vec<InstructionMachineEffects>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct InstructionMachineEffects {
        pub instruction: SelectedInstructionId,
        pub kind: SelectedInstructionKind,
        pub constraint: RegisterConstraintKey,
        pub unit_uses: Vec<RegisterUnitId>,
        pub unit_defs: Vec<RegisterUnitId>,
        pub unit_clobbers: Vec<RegisterUnitId>,
        pub memory: MachineMemoryEffect,
        pub trap: MachineTrapBehavior,
        pub barrier: MachineBarrier,
        pub call: MachineCallEffect,
        pub cleanup: MachineCleanupEffect,
        pub provenance: SelectedInstructionProvenance,
        pub alternatives: Vec<MachineAlternative>,
    }
}

pub use identity::pre_allocation_machine_effect_identity;
pub use instructions::{BlockMachineEffects, FunctionMachineEffects, InstructionMachineEffects};

use crate::{MachineEffectCatalogIdentity, SelectedInstructionPlanIdentity};
use optimization_core::OptimizationUnitIdentity;
use register_model::{RegisterConstraintCatalogIdentity, TargetRegisterEnvironmentIdentity};
use semantic_vocabulary::WorkScheduleIdentity;
use target::NativeTarget;

crate::identity::sha256_identity!(PreAllocationMachineEffectIdentity);

impl PreAllocationMachineEffectPlan {
    pub fn blocks(&self) -> impl Iterator<Item = &BlockMachineEffects> + '_ {
        self.functions.iter().flat_map(|function| &function.blocks)
    }

    pub fn instructions(&self) -> impl Iterator<Item = &InstructionMachineEffects> + '_ {
        self.functions
            .iter()
            .flat_map(|function| function.instructions())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreAllocationMachineEffectPlan {
    pub identity: PreAllocationMachineEffectIdentity,
    pub selected: SelectedInstructionPlanIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub target: NativeTarget,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub register_constraints: RegisterConstraintCatalogIdentity,
    pub machine_effect_catalog: MachineEffectCatalogIdentity,
    pub functions: Vec<FunctionMachineEffects>,
}
