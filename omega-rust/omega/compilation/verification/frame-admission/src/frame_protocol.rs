//! Choose the frame protocol supported by the admitted target.
use target::{Architecture, NativeTarget, ObjectFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameProtocolCodec {
    X86_64,
    Aarch64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetFrameProtocolEncodingError {
    RootMismatch,
    UnsupportedPolicy,
    UnsupportedTarget,
    FunctionRosterMismatch,
    ByteArenaOverflow,
    UnsupportedReturnAddressCustody,
    X86(target_x86_64::X86_64FrameProtocolError),
    Aarch64(target_aarch64::Aarch64FrameProtocolError),
    NonCanonicalEncoding,
}

diagnostics::failed_display!(
    TargetFrameProtocolEncodingError,
    "target frame protocol encoding failed"
);

impl std::error::Error for TargetFrameProtocolEncodingError {}

pub const fn frame_protocol_codec(target: NativeTarget) -> Option<FrameProtocolCodec> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf)
        | (Architecture::X86_64, ObjectFormat::Coff)
        | (Architecture::X86_64, ObjectFormat::MachO) => Some(FrameProtocolCodec::X86_64),
        (Architecture::Aarch64, ObjectFormat::Elf)
        | (Architecture::Aarch64, ObjectFormat::MachO) => Some(FrameProtocolCodec::Aarch64),
        (Architecture::Aarch64, ObjectFormat::Coff) => None,
    }
}

// Reconstruct expected bytes from the submitted frame geometry. This replay
// retains the existing shared ISA recipes; it does not issue a physical plan.
