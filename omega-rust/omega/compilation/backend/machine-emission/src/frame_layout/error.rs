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

impl std::fmt::Display for TargetFrameLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "target frame layout failed: {self:?}")
    }
}

impl std::error::Error for TargetFrameLayoutError {}
