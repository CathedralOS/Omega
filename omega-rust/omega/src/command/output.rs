//! Product-owned publication of compiler outputs.
//!
//! The compiler stops at an in-memory semantic product. Choosing a path and
//! making bytes visible is command/product policy, not compilation.

use std::path::{Path, PathBuf};

pub(super) fn publish_native_artifact(
    report: omega_compiler::CompileReport,
    build_dir: &Path,
) -> Result<(omega_compiler::CompileReport, PathBuf), String> {
    let published = report.publish_retained_native_artifact(build_dir)?;
    let path = published
        .checked_native_executable_path()
        .map(Path::to_path_buf)
        .ok_or_else(|| "native publication did not retain executable custody".to_owned())?;
    Ok((published, path))
}
