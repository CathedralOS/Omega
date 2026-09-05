#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetFrameProtocolEncodingError {
    RootMismatch,
    UnsupportedPolicy,
    FunctionRosterMismatch,
    ByteArenaOverflow,
    UnsupportedReturnAddressCustody,
    X86(isa_x86_64::X86_64FrameProtocolError),
    Aarch64(isa_aarch64::Aarch64FrameProtocolError),
    NonCanonicalEncoding,
}

impl std::fmt::Display for TargetFrameProtocolEncodingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "target frame protocol encoding failed: {self:?}")
    }
}

impl std::error::Error for TargetFrameProtocolEncodingError {}
