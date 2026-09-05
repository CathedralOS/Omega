use omega_isa_x86_64::{
    X86_64StructuralUnitCallTemplateError, X86_64StructuralUnitInternalControlResolutionError,
};
use psi_core::MachineId;

use crate::FunctionFragmentEmissionError;
use crate::FunctionFragmentFrameApplicationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationFreeTextSectionPlacementError {
    Source(FunctionFragmentEmissionError),
    FrameSource(FunctionFragmentFrameApplicationError),
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
    InternalCallOutOfRange,
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

impl From<crate::TextPlacementError> for RelocationFreeTextSectionPlacementError {
    fn from(error: crate::TextPlacementError) -> Self {
        use crate::TextPlacementError as Source;
        match error {
            Source::OffsetOverflow => Self::OffsetOverflow,
            Source::StatisticsOverflow => Self::StatisticsOverflow,
            Source::SourceShapeMismatch => Self::SourceShapeMismatch,
            Source::MisalignedAarch64Span => Self::MisalignedAarch64Span,
            Source::UnsupportedRelocationShape => Self::UnsupportedRelocationShape,
            Source::UnresolvedInternalMachineFixups => Self::UnresolvedInternalMachineFixups,
            Source::InternalCallOutOfRange => Self::InternalCallOutOfRange,
            Source::ArtifactMismatch => Self::ArtifactMismatch,
            Source::DuplicateFunction(machine) => Self::DuplicateFunction(machine),
            Source::MissingSemanticEntry(machine) => Self::MissingSemanticEntry(machine),
            Source::DuplicateSemanticEntry(machine) => Self::DuplicateSemanticEntry(machine),
            Source::MissingInternalMachineTarget(machine) => {
                Self::MissingInternalMachineTarget(machine)
            }
            Source::StructuralUnitCallTemplate(machine, error) => {
                Self::StructuralUnitCallTemplate(machine, error)
            }
            Source::StructuralUnitCallResolution(machine, error) => {
                Self::StructuralUnitCallResolution(machine, error)
            }
        }
    }
}

pub use omega_machine_code::FunctionFragmentTextSectionManifestDecodeError;
