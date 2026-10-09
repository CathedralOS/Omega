#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetFrameLayoutError {
    RootMismatch,
    UnsupportedPolicy,
    UnsupportedTarget,
    FunctionRosterMismatch,
    MissingStackPointerView,
    MissingLinkRegisterView,
    GeometryOverflow,
    NonCanonicalLayout,
    /// The activation is larger than the largest frame every target can
    /// address: [`MAX_FRAME_BYTES`](super::MAX_FRAME_BYTES).
    FrameExceedsAddressableExtent {
        frame_size_bytes: u64,
    },
}

diagnostics::failed_display!(TargetFrameLayoutError, "target frame layout failed");

impl std::error::Error for TargetFrameLayoutError {}
