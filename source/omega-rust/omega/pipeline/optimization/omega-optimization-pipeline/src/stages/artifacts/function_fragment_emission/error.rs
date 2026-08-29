use psi_core::MachineId;

use crate::{
    FunctionRelativeOptimizationRealizationError,
    OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
    OptimizedStructuralUnitFunctionRelativeRealizationError,
    OptimizedUnitFunctionRelativeRealizationError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunctionFragmentEmissionError {
    Source(FunctionRelativeOptimizationRealizationError),
    ActiveResidentRematerializationSource(
        OptimizedActiveResidentRematerializationFunctionRelativeRealizationError,
    ),
    UnitSource(OptimizedUnitFunctionRelativeRealizationError),
    StructuralUnitSource(OptimizedStructuralUnitFunctionRelativeRealizationError),
    MissingX86Rel8Realization,
    SourceKindMismatch,
    MissingFunction(MachineId),
    MissingBlock(omega_selected_instructions::SelectedBlockId),
    MissingInstruction(omega_selected_instructions::SelectedInstructionId),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentEmissionManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownSourceKind(u8),
    UnknownPostAllocationMachineOptimization(u8),
    UnknownVocabulary(u16),
    InvalidFuelSchedule,
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    TargetLayoutOverflow,
    UnknownUnavailableStatus,
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for FunctionFragmentEmissionManifestDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid function-fragment emission manifest: {self:?}"
        )
    }
}

impl std::error::Error for FunctionFragmentEmissionManifestDecodeError {}
