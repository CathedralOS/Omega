//! Apply a dependency edit plan to a candidate snapshot, not the live project.

use std::path::Path;

use crate::declarations::dependencies::edit::BuildFileReplacement;
use package_source::local::staging::{StagedLocalSnapshot, stage_local_source_replacement_in_lane};
use package_source::{
    LocalSourceLimits, SourceRelativePath, SourceResolveError, SourceResolverStorage,
};

/// Stage an automatic dependency edit after checking the planner's old-file
/// digest. Manual and unchanged plans do not contain a replacement to stage.
///
/// Keep the plan and this result through review: the file transaction must
/// recheck live source and project files immediately before publication. The
/// staged closure resolver uses the original root, not the snapshot directory,
/// as package lineage and the base for relative dependencies.
pub fn stage_build_dependency_edit(
    replacement: &BuildFileReplacement,
    storage: &SourceResolverStorage,
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
