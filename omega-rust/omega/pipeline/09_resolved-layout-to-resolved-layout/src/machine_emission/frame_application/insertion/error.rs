use semantic_vocabulary::MachineId;
use target_operations_to_selected_instructions::{SelectedBlockId, SelectedInstructionId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameApplicationError {
    RootMismatch,
    FunctionRosterMismatch,
    MissingFunction(MachineId),
    InvalidProtocolSpan(MachineId),
    UnsupportedFramedControl(MachineId),
    MissingFinalReturn(MachineId),
    SourceShapeMismatch(MachineId),
    MissingTargetBlock(SelectedBlockId),
    BranchFallthroughMismatch(SelectedInstructionId),
    BranchEffectsMismatch(SelectedInstructionId),
    X86_64Branch(
        SelectedInstructionId,
        target_operations_to_selected_instructions::isa_x86_64::X86_64SelectedFormEncodingError,
    ),
    Aarch64Branch(
        SelectedInstructionId,
        target_operations_to_selected_instructions::isa_aarch64::Aarch64SelectedFormEncodingError,
    ),
    OffsetOverflow,
    ArtifactMismatch,
}

impl std::fmt::Display for FrameApplicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "function-fragment frame application failed: {self:?}"
        )
    }
}

impl std::error::Error for FrameApplicationError {}
