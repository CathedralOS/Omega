use semantic_vocabulary::MachineId;

use super::projection::{FunctionFragmentStatisticsOverflow, ResolvedFragmentEmissionError};
use crate::function_realization::FunctionRelativeOptimizationRealizationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionFragmentEmissionError {
    Source(FunctionRelativeOptimizationRealizationError),
    SourceKindMismatch,
    MissingFunction(MachineId),
    MissingBlock(selected_instructions::SelectedBlockId),
    MissingInstruction(selected_instructions::SelectedInstructionId),
    OffsetOverflow,
    StatisticsOverflow,
    RootMismatch,
    ArtifactMismatch,
    ManifestMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for FunctionFragmentEmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized function-fragment emission failed: {self:?}"
        )
    }
}

impl std::error::Error for FunctionFragmentEmissionError {}

impl From<ResolvedFragmentEmissionError> for FunctionFragmentEmissionError {
    fn from(error: ResolvedFragmentEmissionError) -> Self {
        use ResolvedFragmentEmissionError as Source;
        match error {
            Source::MissingFunction(value) => Self::MissingFunction(value),
            Source::MissingBlock(value) => Self::MissingBlock(value),
            Source::MissingInstruction(value) => Self::MissingInstruction(value),
            Source::OffsetOverflow => Self::OffsetOverflow,
            Source::RootMismatch => Self::RootMismatch,
            Source::ArtifactMismatch => Self::ArtifactMismatch,
        }
    }
}

pub use machine_code::FunctionFragmentEmissionManifestDecodeError;

impl From<FunctionFragmentStatisticsOverflow> for FunctionFragmentEmissionError {
    fn from(_: FunctionFragmentStatisticsOverflow) -> Self {
        Self::StatisticsOverflow
    }
}
