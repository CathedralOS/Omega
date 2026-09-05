#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentTextSectionManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownSourceCustody(u8),
    SourceCustodyMismatch,
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
