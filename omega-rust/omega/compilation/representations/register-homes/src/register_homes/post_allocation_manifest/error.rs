#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationOptimizationManifestError {
    RootMismatch,
    UnresolvedFixedViewTransitions,
    StatisticsOverflow,
    NonCanonicalTransformationLedger,
    IdentityMismatch,
    ContentMismatch,
}

impl std::fmt::Display for PostAllocationOptimizationManifestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid post-allocation optimization manifest: {self:?}"
        )
    }
}

impl std::error::Error for PostAllocationOptimizationManifestError {}

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

impl From<crate::cursor::Truncated> for PostAllocationOptimizationManifestDecodeError {
    fn from(_: crate::cursor::Truncated) -> Self {
        Self::Truncated
    }
}

impl From<crate::cursor::TakeError> for PostAllocationOptimizationManifestDecodeError {
    fn from(error: crate::cursor::TakeError) -> Self {
        match error {
            crate::cursor::TakeError::Truncated => Self::Truncated,
            crate::cursor::TakeError::LengthOverflow => Self::LengthOverflow,
        }
    }
}

impl std::fmt::Display for PostAllocationOptimizationManifestDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid post-allocation manifest encoding: {self:?}"
        )
    }
}

impl std::error::Error for PostAllocationOptimizationManifestDecodeError {}
