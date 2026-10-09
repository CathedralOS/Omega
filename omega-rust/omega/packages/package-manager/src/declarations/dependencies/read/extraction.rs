use super::error::DependencyProjectionError;
use super::projection::{
    extract_build_projection_from_source, extract_scoped_requests_from_source,
};
use super::{BuildDependencyProjection, DependencyPurpose, DependencySourceRequest};
use std::path::Path;



/// Project the project role and direct dependencies together from one parse.
pub(crate) fn extract_build_dependency_projection(
    package_root: impl AsRef<Path>,
) -> Result<BuildDependencyProjection, DependencyProjectionError> {
    let source = build_declarations::read_build_file_source(
        package_root,
        |path| DependencyProjectionError::MissingBuildFile { path },
        |path, message| DependencyProjectionError::ReadBuildFile { path, message },
        |path| DependencyProjectionError::InvalidBuildFileEncoding { path },
    )?;
    extract_build_projection_from_source(&source)
}

/// Project every unconditional direct dependency row in authored order,
/// tagged with the scope it authorizes. Edit planning needs this one flat
/// list to correlate rows and requests across both purposes.
pub(crate) fn extract_scoped_from_source(
    source: &str,
) -> Result<Vec<(DependencyPurpose, DependencySourceRequest)>, DependencyProjectionError> {
    extract_scoped_requests_from_source(source)
}
