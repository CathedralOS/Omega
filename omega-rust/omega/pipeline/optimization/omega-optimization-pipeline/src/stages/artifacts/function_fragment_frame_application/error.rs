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
