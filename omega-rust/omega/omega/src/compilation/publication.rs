//! Product-owned publication of compiler outputs.
//!
//! The compiler stops at an in-memory semantic product. Choosing a path and
//! making bytes visible is command/product policy, not compilation.

use std::path::{Path, PathBuf};

pub fn publish_compilation(
    report: compilation_products::CompileReport,
    build_dir: &Path,
) -> Result<(compilation_products::CompileReport, Option<PathBuf>), String> {
    let report = compilation_products::publication::publish_compilation(report, build_dir)?;
    let executable = report
        .checked_native_executable_path()
        .map(Path::to_path_buf);
    Ok((report, executable))
}

pub fn publish_native_artifact(
    report: compilation_products::CompileReport,
    build_dir: &Path,
) -> Result<(compilation_products::CompileReport, PathBuf), String> {
    if report.output_kind() == compilation_products::CompileOutputKind::BuildArtifacts {
        return Err("an artifact-only build has no executable to run".into());
    }
    if report.output_kind() != compilation_products::CompileOutputKind::RetainedNativeArtifact {
        return Err("running requires a retained native executable product".into());
    }
    let (published, path) = publish_compilation(report, build_dir)?;
    let path =
        path.ok_or_else(|| "native publication did not retain executable custody".to_owned())?;
    Ok((published, path))
}
