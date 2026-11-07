//! Preserve the first failed native relation without replaying it a second time.
#[derive(Debug)]
pub enum NativeArtifactValidationError {
    Invariant(&'static str),
    ImageReplay(diagnostics::Diagnostic),
}
impl From<&'static str> for NativeArtifactValidationError {
    fn from(reason: &'static str) -> Self {
        Self::Invariant(reason)
    }
}
impl std::fmt::Display for NativeArtifactValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invariant(reason) => f.write_str(reason),
            Self::ImageReplay(diagnostic) => {
                write!(f, "native artifact image replay: {}", diagnostic.message)
            }
        }
    }
}
impl std::error::Error for NativeArtifactValidationError {}
impl From<NativeArtifactValidationError> for String {
    fn from(error: NativeArtifactValidationError) -> Self {
        error.to_string()
    }
}
impl NativeArtifactValidationError {
    pub fn into_diagnostic(self) -> diagnostics::Diagnostic {
        match self {
            Self::ImageReplay(diagnostic) => diagnostic,
            Self::Invariant(reason) => diagnostics::Diagnostic::error(reason),
        }
    }
}
