#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetFrameProtocolEncodingError {
    RootMismatch,
    UnsupportedPolicy,
    /// The (architecture, object-format) pair declares no codec or unwind
    /// row, so no codec can be selected and no continuation mechanism can
    /// admit any recorded custody.
    UnsupportedTarget,
    FunctionRosterMismatch,
    ByteArenaOverflow,
    UnsupportedReturnAddressCustody,
    X86(isa_x86_64::X86_64FrameProtocolError),
    Aarch64(isa_aarch64::Aarch64FrameProtocolError),
    NonCanonicalEncoding,
}

diagnostics::failed_display!(
    TargetFrameProtocolEncodingError,
    "target frame protocol encoding failed"
);

impl std::error::Error for TargetFrameProtocolEncodingError {}
