// Stage an automatic `build.omg` dependency replacement into a private candidate snapshot (`stage_build_dependency_edit`) without mutating the live project checkout.

use std::path::Path;

use crate::declarations::dependencies::edit::BuildFileReplacement;
use source_acquisition::local::staging::{
    StagedLocalSnapshot, stage_selected_local_source_replacement_in_lane,
};
use source_acquisition::{
    LocalSourceLimits, SourceRelativePath, SourceResolveError, SourceResolver,
};

pub fn stage_build_dependency_edit(
    replacement: &BuildFileReplacement,
    storage: &SourceResolver,
    limits: LocalSourceLimits,
    selected: Option<&source_acquisition::SourceCaptureRequest>,
) -> Result<StagedLocalSnapshot, SourceResolveError> {
    let root = replacement
        .build_path()
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let relative_path = SourceRelativePath::parse("build.omg")
        .expect("the compiler build filename is a portable source path");
    storage.verify_path_identity()?;
    let entire = source_acquisition::SourceCaptureRequest::entire_tree();
    let result = stage_selected_local_source_replacement_in_lane(
        root,
        &relative_path,
        replacement.expected_sha256(),
        replacement.replacement_source().as_bytes(),
        storage.external_local_sources(),
        limits,
        selected.unwrap_or(&entire),
    );
    storage.verify_path_identity()?;
    result
}
