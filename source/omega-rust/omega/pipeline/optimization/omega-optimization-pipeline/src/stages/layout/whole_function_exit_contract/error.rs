use omega_register_model::RegisterUnitId;
use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId};
use psi_core::MachineId;

use crate::{OptimizedResolvedSelectedFormLayoutError, OptimizedX86BranchRelaxationError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WholeFunctionExitContractError {
    Layout(OptimizedResolvedSelectedFormLayoutError),
    MovnLayout,
    Relaxation(OptimizedX86BranchRelaxationError),
    OptimizationCustodyMismatch,
    RootMismatch,
    UnsupportedTargetPolicy,
    MissingArchitecturalView(&'static str),
    InvalidConvention,
    DuplicateInstruction(SelectedInstructionId),
    MissingInstruction(SelectedInstructionId),
    FunctionRosterMismatch(MachineId),
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
