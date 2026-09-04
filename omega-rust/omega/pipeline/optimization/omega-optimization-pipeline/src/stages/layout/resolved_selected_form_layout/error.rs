use omega_isa_aarch64::Aarch64SelectedFormEncodingError;
use omega_isa_x86_64::X86_64SelectedFormEncodingError;
use omega_selected_instructions::SelectedInstructionId;
use psi_core::MachineId;

use crate::OptimizedSelectedFormEncodingError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizedResolvedSelectedFormLayoutError {
    PreLayout(OptimizedSelectedFormEncodingError),
    RootMismatch,
    MixedOrdinaryAndStructuralFunctions,
    StructuralFunctionRosterMismatch(MachineId),
    StructuralCallRosterMismatch(SelectedInstructionId),
    StructuralReturnRosterMismatch(SelectedInstructionId),
    StructuralEncodingMismatch(SelectedInstructionId),
    UnsupportedFunctionShape(MachineId),
    DuplicateInstruction(SelectedInstructionId),
    MissingInstruction(SelectedInstructionId),
    AlternativeMismatch(SelectedInstructionId),
    UnexpectedEncodingState(SelectedInstructionId),
    OffsetOverflow,
    BranchFallthroughMismatch(SelectedInstructionId),
    BranchEffectsMismatch(SelectedInstructionId),
    BranchSizeMismatch(SelectedInstructionId),
    OptimizationCustodyMismatch,
    OptimizationByteSavingsMismatch,
    X86_64(X86_64SelectedFormEncodingError),
    Aarch64(Aarch64SelectedFormEncodingError),
    ArtifactMismatch,
}

impl std::fmt::Display for OptimizedResolvedSelectedFormLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized resolved selected-form layout failed: {self:?}"
        )
    }
}

impl std::error::Error for OptimizedResolvedSelectedFormLayoutError {}
