use isa_aarch64::Aarch64SelectedFormEncodingError;
use isa_x86_64::X86_64SelectedFormEncodingError;
use selected_instructions::SelectedInstructionId;
use semantic_vocabulary::MachineId;

use post_allocation_machine_to_selected_form_encoding::OptimizedSelectedFormEncodingError;

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
