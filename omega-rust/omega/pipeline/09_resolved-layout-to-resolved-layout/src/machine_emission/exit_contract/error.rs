use semantic_vocabulary::MachineId;
use target_operations_to_selected_instructions::register_model::RegisterUnitId;
use target_operations_to_selected_instructions::{SelectedBlockId, SelectedInstructionId};

use crate::{OptimizedX86BranchRelaxationError, ResolvedLayoutOptimizationError};
use selected_form_encoding_to_resolved_layout::OptimizedResolvedSelectedFormLayoutError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WholeFunctionExitContractError {
    Layout(OptimizedResolvedSelectedFormLayoutError),
    Relaxation(OptimizedX86BranchRelaxationError),
    LayoutOptimization(ResolvedLayoutOptimizationError),
    OptimizationCustodyMismatch,
    RootMismatch,
    UnsupportedTargetPolicy,
    MissingArchitecturalView(&'static str),
    InvalidConvention,
    DuplicateInstruction(SelectedInstructionId),
    MissingInstruction(SelectedInstructionId),
    FunctionRosterMismatch(MachineId),
    FramePreservationMismatch(MachineId),
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
