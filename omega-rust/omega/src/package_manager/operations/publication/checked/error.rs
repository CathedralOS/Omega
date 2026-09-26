use std::fmt;

#[derive(Debug)]
pub enum PublishReviewedPackageChangeError {
    Association(&'static str),
    Publication(super::super::PackagePublicationError),
    Source(crate::package_source::SourceResolveError),
    Lock(crate::package_manager::lock::PackageLockError),
    Review(crate::package_manager::operations::PackageChangeError),
    Comparison(crate::package_manager::review::PackagePolicyChangeError),
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
