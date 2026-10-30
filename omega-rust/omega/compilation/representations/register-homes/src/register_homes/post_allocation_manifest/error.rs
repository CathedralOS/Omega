#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationOptimizationManifestError {
    RootMismatch,
    UnresolvedFixedViewTransitions,
    StatisticsOverflow,
    NonCanonicalTransformationLedger,
    IdentityMismatch,
    ContentMismatch,
}

crate::decode_error_display! { PostAllocationOptimizationManifestError, "invalid post-allocation optimization manifest" }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationOptimizationManifestDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownStage(u8),
    UnknownArchitecture(u8),
    UnknownObjectFormat(u8),
    TargetLayoutOverflow,
    LengthOverflow,
    UnknownTransformationTag(u8),
    UnknownCompletionStatus(u8),
    UnknownSpillStatus(u8),
    UnknownUnavailableStatus(u8),
    IdentityMismatch,
    TrailingBytes,
}

crate::decode_error! { PostAllocationOptimizationManifestDecodeError, "invalid post-allocation manifest encoding" }
