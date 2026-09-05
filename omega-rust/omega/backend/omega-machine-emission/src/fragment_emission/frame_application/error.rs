use crate::FunctionFragmentEmissionError;
use omega_selected_instructions::{SelectedBlockId, SelectedInstructionId};
use psi_core::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionFragmentFrameApplicationError {
    Source(FunctionFragmentEmissionError),
    SourceKindMismatch,
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
        omega_isa_x86_64::X86_64SelectedFormEncodingError,
    ),
    Aarch64Branch(
        SelectedInstructionId,
        omega_isa_aarch64::Aarch64SelectedFormEncodingError,
    ),
    OffsetOverflow,
    ArtifactMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for FunctionFragmentFrameApplicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "function-fragment frame application failed: {self:?}"
        )
    }
}

impl std::error::Error for FunctionFragmentFrameApplicationError {}

impl From<crate::FrameApplicationError> for FunctionFragmentFrameApplicationError {
    fn from(error: crate::FrameApplicationError) -> Self {
        use crate::FrameApplicationError as Source;
        match error {
            Source::RootMismatch => Self::RootMismatch,
            Source::FunctionRosterMismatch => Self::FunctionRosterMismatch,
            Source::MissingFunction(value) => Self::MissingFunction(value),
            Source::InvalidProtocolSpan(value) => Self::InvalidProtocolSpan(value),
            Source::UnsupportedFramedControl(value) => Self::UnsupportedFramedControl(value),
            Source::MissingFinalReturn(value) => Self::MissingFinalReturn(value),
            Source::SourceShapeMismatch(value) => Self::SourceShapeMismatch(value),
            Source::MissingTargetBlock(value) => Self::MissingTargetBlock(value),
            Source::BranchFallthroughMismatch(value) => Self::BranchFallthroughMismatch(value),
            Source::BranchEffectsMismatch(value) => Self::BranchEffectsMismatch(value),
            Source::X86_64Branch(instruction, error) => Self::X86_64Branch(instruction, error),
            Source::Aarch64Branch(instruction, error) => Self::Aarch64Branch(instruction, error),
            Source::OffsetOverflow => Self::OffsetOverflow,
            Source::ArtifactMismatch => Self::ArtifactMismatch,
        }
    }
}
