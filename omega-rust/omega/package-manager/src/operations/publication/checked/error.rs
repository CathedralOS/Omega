use std::fmt;

#[derive(Debug)]
pub enum PublishReviewedPackageChangeError {
    Association(&'static str),
    Publication(super::super::PackagePublicationError),
    Source(package_source::SourceResolveError),
    Lock(crate::lock::PackageLockError),
    Review(crate::operations::PackageChangeError),
    Comparison(crate::review::PackagePolicyChangeError),
}

impl fmt::Display for PublishReviewedPackageChangeError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Association(message) => {
                write!(output, "cannot publish reviewed package change: {message}")
            }
            Self::Publication(error) => error.fmt(output),
            Self::Source(error) => error.fmt(output),
            Self::Lock(error) => error.fmt(output),
            Self::Review(error) => error.fmt(output),
            Self::Comparison(error) => error.fmt(output),
        }
    }
}
impl std::error::Error for PublishReviewedPackageChangeError {}
