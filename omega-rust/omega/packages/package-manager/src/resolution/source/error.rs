use package_source::SourceResolveError;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvePackageSourceError {
    Source(SourceResolveError),
    RecordedGitPin {
        locator: String,
        revision: String,
        message: String,
    },
}
impl fmt::Display for ResolvePackageSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "cannot resolve package source: {error}"),
            Self::RecordedGitPin {
                locator,
                revision,
                message,
            } => write!(
                formatter,
                "cannot retain Git pin for `{locator}` at `{revision}`: {message}"
            ),
        }
    }
}
impl std::error::Error for ResolvePackageSourceError {}
impl From<SourceResolveError> for ResolvePackageSourceError {
    fn from(error: SourceResolveError) -> Self {
        Self::Source(error)
    }
}
