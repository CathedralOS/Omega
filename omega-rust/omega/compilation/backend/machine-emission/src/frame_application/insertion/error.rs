use selected_instructions::{SelectedBlockId, SelectedInstructionId};
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FrameApplicationError {
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
        isa_x86_64::X86_64SelectedFormEncodingError,
    ),
    Aarch64Branch(
        SelectedInstructionId,
        isa_aarch64::Aarch64SelectedFormEncodingError,
    ),
    OffsetOverflow,
    ArtifactMismatch,
}

diagnostics::failed_display!(
    FrameApplicationError,
    "function-fragment frame application failed"
);

impl std::error::Error for FrameApplicationError {}
