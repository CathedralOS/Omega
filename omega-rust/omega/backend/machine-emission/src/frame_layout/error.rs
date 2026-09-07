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
}

impl std::fmt::Display for TargetFrameLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "target frame layout failed: {self:?}")
    }
}

impl std::error::Error for TargetFrameLayoutError {}
