use crate::FunctionFragmentEmissionError;
use psi_core::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionFragmentFrameApplicationError {
    Source(FunctionFragmentEmissionError),
    RootMismatch,
    FunctionRosterMismatch,
    MissingFunction(MachineId),
    InvalidProtocolSpan(MachineId),
    UnsupportedFramedControl(MachineId),
    MissingFinalReturn(MachineId),
    SourceShapeMismatch(MachineId),
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
