use super::error::DependencyProjectionError;
use super::projection::{
    extract_build_projection_from_source, extract_scoped_requests_from_source,
};
use super::{BuildDependencyProjection, DependencyPurpose, DependencySourceRequest};
use std::fs;
use std::path::Path;

pub(super) const BUILD_FILE_NAME: &str = "build.omg";

/// Project direct product dependency declarations from the immutable package
/// root.
///
/// This parses only the root `build.omg`; it does not evaluate build code,
/// imports, constants, helpers, control flow, or providers. The same parsed
/// tree must first produce one authoritative package, application, or workspace
/// declaration; absence is not a second implicit project kind. Build-purpose
/// requests are projected but not returned by this product-scope helper.
pub fn extract_dependency_projection(
    package_root: impl AsRef<Path>,
) -> Result<Vec<DependencySourceRequest>, DependencyProjectionError> {
    Ok(extract_build_dependency_projection(package_root)?
        .into_parts()
        .1
        .into_product()
        .into_authored_dependencies())
}

/// Project the project role and direct dependencies together from one parse.
pub fn extract_build_dependency_projection(
    package_root: impl AsRef<Path>,
) -> Result<BuildDependencyProjection, DependencyProjectionError> {
    let build_path = package_root.as_ref().join(BUILD_FILE_NAME);
    let source_bytes = match fs::read(&build_path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(DependencyProjectionError::MissingBuildFile { path: build_path });
        }
        Err(error) => {
            return Err(DependencyProjectionError::ReadBuildFile {
                path: build_path,
                message: error.to_string(),
            });
        }
    };
    let source = std::str::from_utf8(&source_bytes).map_err(|_| {
        DependencyProjectionError::InvalidBuildFileEncoding {
            path: build_path.clone(),
        }
    })?;
    extract_build_projection_from_source(source)
}

/// Project every unconditional direct dependency row in authored order,
/// tagged with the scope it authorizes. Edit planning needs this one flat
/// list to correlate rows and requests across both purposes.
pub(crate) fn extract_scoped_from_source(
    source: &str,
) -> Result<Vec<(DependencyPurpose, DependencySourceRequest)>, DependencyProjectionError> {
    extract_scoped_requests_from_source(source)
}
