use semantic_vocabulary::MachineId;

use super::placement::TextPlacementError;
use crate::fragment_emission::FunctionFragmentEmissionError;
use crate::frame_application::FunctionFragmentFrameApplicationError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelocationFreeTextSectionPlacementError {
    Source(FunctionFragmentEmissionError),
    FrameSource(FunctionFragmentFrameApplicationError),
    DuplicateFunction(MachineId),
    MissingSemanticEntry(MachineId),
    OffsetOverflow,
    StatisticsOverflow,
    SourceShapeMismatch,
    MisalignedAarch64Span,
    UnsupportedRelocationShape,
    UnresolvedInternalMachineFixups,
    MissingInternalMachineTarget(MachineId),
    InternalCallOutOfRange,
    ArtifactMismatch,
    ManifestMismatch,
    ReceiptMismatch,
}

diagnostics::failed_display!(
    RelocationFreeTextSectionPlacementError,
    "optimized relocation-free text-section placement failed"
);

impl std::error::Error for RelocationFreeTextSectionPlacementError {}

impl From<TextPlacementError> for RelocationFreeTextSectionPlacementError {
    fn from(error: TextPlacementError) -> Self {
        use TextPlacementError as Source;
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
            Source::MissingInternalMachineTarget(machine) => {
                Self::MissingInternalMachineTarget(machine)
            }
        }
    }
}

pub use machine_code::FunctionFragmentTextSectionManifestDecodeError;
