use isa_x86_64::{
    X86_64StructuralUnitCallTemplateError, X86_64StructuralUnitInternalControlResolutionError,
};
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextPlacementError {
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
}

impl std::fmt::Display for TextPlacementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "optimized relocation-free text-section placement failed: {self:?}"
        )
    }
}

impl std::error::Error for TextPlacementError {}
