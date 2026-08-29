use omega_isa_x86_64::{
    X86_64StructuralUnitCallTemplateError, X86_64StructuralUnitInternalControlResolutionError,
};
use psi_core::MachineId;

use crate::FunctionFragmentEmissionError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationFreeTextSectionPlacementError {
    Source(FunctionFragmentEmissionError),
    DuplicateFunction(MachineId),
    MissingSemanticEntry(MachineId),
    DuplicateSemanticEntry(MachineId),
    OffsetOverflow,
    StatisticsOverflow,
    SourceShapeMismatch,
    MisalignedAarch64Span,
    UnsupportedRelocationShape,
    UnresolvedInternalMachineFixups,
    MissingInternalMachineTarget(MachineId),
    StructuralUnitCallTemplate(MachineId, X86_64StructuralUnitCallTemplateError),
    StructuralUnitCallResolution(
        MachineId,
        X86_64StructuralUnitInternalControlResolutionError,
    ),
    ArtifactMismatch,
    ManifestMismatch,
    ReceiptMismatch,
}

impl std::fmt::Display for RelocationFreeTextSectionPlacementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized relocation-free text-section placement failed: {self:?}"
        )
    }
}

impl std::error::Error for RelocationFreeTextSectionPlacementError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentTextSectionManifestDecodeError {
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
    InvalidSemanticEntry,
    UnknownPlacementPolicy(u8),
    UnknownRelocationRequirements(u8),
    UnknownUnavailableStatus,
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for FunctionFragmentTextSectionManifestDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid function-fragment text-section manifest: {self:?}"
        )
    }
}

impl std::error::Error for FunctionFragmentTextSectionManifestDecodeError {}
