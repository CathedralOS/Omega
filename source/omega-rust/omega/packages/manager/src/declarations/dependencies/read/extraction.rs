use super::error::DependencyProjectionError;
use super::model::{BuildDependencyProjection, DependencySourceRequest};
use super::projection::extract_build_projection_from_source;
use std::fs;
use std::path::Path;

pub(super) const BUILD_FILE_NAME: &str = "build.omg";

/// Project direct dependency declarations from the immutable package root.
///
/// This parses only the root `build.omg`; it does not evaluate build code,
/// imports, constants, helpers, control flow, or providers. The same parsed
/// tree must first produce one authoritative package, application, or workspace
/// declaration; absence is not a second implicit project kind.
pub fn extract_dependency_projection(
    package_root: impl AsRef<Path>,
) -> Result<Vec<DependencySourceRequest>, DependencyProjectionError> {
    Ok(extract_build_dependency_projection(package_root)?
        .into_parts()
        .1
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

pub(crate) fn extract_from_source(
    source: &str,
) -> Result<Vec<DependencySourceRequest>, DependencyProjectionError> {
    Ok(extract_build_projection_from_source(source)?
        .into_parts()
        .1
        .into_authored_dependencies())
}
