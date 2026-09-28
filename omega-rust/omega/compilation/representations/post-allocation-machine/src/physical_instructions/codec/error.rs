//! Closed decode failures for the post-allocation machine-plan protocol.

/// Failure while decoding a framed post-allocation machine-plan artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAllocationMachineDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidField,
    InvalidIdentity,
    TrailingBytes,
}

impl std::fmt::Display for PostAllocationMachineDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid post-allocation machine artifact: {self:?}"
        )
    }
}

impl std::error::Error for PostAllocationMachineDecodeError {}
