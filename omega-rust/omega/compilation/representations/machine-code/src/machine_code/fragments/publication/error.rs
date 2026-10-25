#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionFragmentEmissionManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownVocabulary(u16),
    InvalidFuelSchedule,
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    TargetLayoutOverflow,
    UnknownUnavailableStatus,
    IdentityMismatch,
    TrailingBytes,
}

optimization_core::truncated_from_adapter!(FunctionFragmentEmissionManifestDecodeError);

register_homes::decode_error_display! { FunctionFragmentEmissionManifestDecodeError, "invalid function-fragment emission manifest" }
