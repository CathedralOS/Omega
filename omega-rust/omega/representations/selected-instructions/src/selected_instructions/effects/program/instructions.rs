//! Per-function, block, instruction and structural-call effect rows.
use crate::{
    MachineAlternative, MachineBarrier, MachineCallEffect, MachineCleanupEffect,
    MachineMemoryEffect, MachineTrapBehavior, SelectedBlockId, SelectedInstructionId,
    SelectedInstructionKind, SelectedInstructionProvenance,
    SelectedMicrosoftX64OwnedIndirectPairLayout, StructuralUnitCallEffectDeclaration,
};
use register_model::{RegisterConstraintKey, RegisterUnitId};
use semantic_vocabulary::{MachineId, OperationId};
use terminal_psi::ClaimTransfer;

/// Independently replayable effects for one selected structural-signature
/// Unit function. This remains parallel to the ordinary scalar/VReg roster so
/// it cannot be mistaken for an encoded target alternative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralUnitFunctionMachineEffects {
    pub machine: MachineId,
    pub block: SelectedBlockId,
    pub call: Option<StructuralUnitCallMachineEffects>,
    pub return_instruction: InstructionMachineEffects,
    pub return_effect: optimization_unit::EffectLink,
    pub return_ownership: Vec<optimization_unit::OwnershipEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralUnitCallMachineEffects {
    pub instruction: SelectedInstructionId,
    pub operation: OperationId,
    pub callee: MachineId,
    pub constraint: RegisterConstraintKey,
    pub unit_uses: Vec<RegisterUnitId>,
    pub unit_defs: Vec<RegisterUnitId>,
    pub unit_clobbers: Vec<RegisterUnitId>,
    pub layout: SelectedMicrosoftX64OwnedIndirectPairLayout,
    pub effect: optimization_unit::EffectLink,
    pub ownership: Vec<optimization_unit::OwnershipEvent>,
    pub claim_transfers: Vec<ClaimTransfer>,
    pub provenance: SelectedInstructionProvenance,
    pub declaration: StructuralUnitCallEffectDeclaration,
}

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
