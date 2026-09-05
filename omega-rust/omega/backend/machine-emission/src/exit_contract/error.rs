use register_model::RegisterUnitId;
use selected_instructions::{SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::MachineId;

use post_allocation_machine_to_resolved_layout::{
    OptimizedResolvedSelectedFormLayoutError, OptimizedX86BranchRelaxationError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WholeFunctionExitContractError {
    Layout(OptimizedResolvedSelectedFormLayoutError),
    Relaxation(OptimizedX86BranchRelaxationError),
    OptimizationCustodyMismatch,
    RootMismatch,
    UnsupportedTargetPolicy,
    MissingArchitecturalView(&'static str),
    InvalidConvention,
    DuplicateInstruction(SelectedInstructionId),
    MissingInstruction(SelectedInstructionId),
    FunctionRosterMismatch(MachineId),
    FramePreservationMismatch(MachineId),
    StructuralFunctionRosterMismatch(MachineId),
    StructuralCallRosterMismatch(SelectedInstructionId),
    StructuralCallTopologyMismatch,
    StructuralCallLayoutMismatch(SelectedInstructionId),
    BlockRosterMismatch(SelectedBlockId),
    InstructionRosterMismatch(SelectedInstructionId),
    CalleeSavedWrite {
        instruction: SelectedInstructionId,
        unit: RegisterUnitId,
    },
    LinkRegisterWrite(SelectedInstructionId),
    NonReturnStackEffect(SelectedInstructionId),
    NonReturnMemoryEffect(SelectedInstructionId),
    NonReturnControlEffect(SelectedInstructionId),
    MissingReturn(MachineId),
    ReturnOperandMismatch(SelectedInstructionId),
    ReturnEncodingMismatch(SelectedInstructionId),
    ReturnEffectsMismatch(SelectedInstructionId),
    ReturnPlacementMismatch(SelectedInstructionId),
    OffsetOverflow,
    ArtifactMismatch,
}

impl std::fmt::Display for WholeFunctionExitContractError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid terminal whole-function exit contract: {self:?}"
        )
    }
}

impl std::error::Error for WholeFunctionExitContractError {}
