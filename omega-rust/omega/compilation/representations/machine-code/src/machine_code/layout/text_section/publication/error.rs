#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentTextSectionManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
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

optimization_core::truncated_from_adapter!(FunctionFragmentTextSectionManifestDecodeError);

register_homes::decode_error_display! { FunctionFragmentTextSectionManifestDecodeError, "invalid function-fragment text-section manifest" }
