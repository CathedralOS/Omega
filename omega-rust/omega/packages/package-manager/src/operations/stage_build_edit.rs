// Stage an automatic `build.omg` dependency replacement into a private candidate snapshot (`stage_build_dependency_edit`) without mutating the live project checkout.

use std::path::Path;

use crate::declarations::dependencies::edit::BuildFileReplacement;
use package_source::local::staging::{StagedLocalSnapshot, stage_local_source_replacement_in_lane};
use package_source::{LocalSourceLimits, SourceRelativePath, SourceResolveError, SourceResolver};

pub fn stage_build_dependency_edit(
    replacement: &BuildFileReplacement,
    storage: &SourceResolver,
    limits: LocalSourceLimits,
) -> Result<StagedLocalSnapshot, SourceResolveError> {
    let root = replacement
        .build_path()
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let relative_path = SourceRelativePath::parse("build.omg")
        .expect("the compiler build filename is a portable source path");
    storage.verify_path_identity()?;
    let result = stage_local_source_replacement_in_lane(
        root,
        &relative_path,
        replacement.expected_sha256(),
        replacement.replacement_source().as_bytes(),
        storage.external_local_sources(),
        limits,
    );
    storage.verify_path_identity()?;
    result
}
