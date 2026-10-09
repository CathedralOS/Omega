use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TextPlacementError {
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
}

failed_display!(
    TextPlacementError,
    "optimized relocation-free text-section placement failed"
);

impl std::error::Error for TextPlacementError {}
