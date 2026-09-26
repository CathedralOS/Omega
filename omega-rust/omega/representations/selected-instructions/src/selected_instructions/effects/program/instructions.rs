//! Per-function, block, instruction and structural-call effect rows.
use crate::{
    MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineMemoryEffect, MachineTrapBehavior, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance,
};
use register_model::{RegisterConstraintKey, RegisterUnitId};
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionMachineEffects {
    pub machine: MachineId,
    pub blocks: Vec<BlockMachineEffects>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockMachineEffects {
    pub block: SelectedBlockId,
    /// Ordinary selected instructions followed by the selected terminator.
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
