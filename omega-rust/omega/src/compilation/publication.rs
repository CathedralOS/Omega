//! Product-owned publication of compiler outputs.
//!
//! The compiler stops at an in-memory semantic product. Choosing a path and
//! making bytes visible is command/product policy, not compilation.

use std::path::{Path, PathBuf};

pub fn publish_compilation(
    report: crate::compiler::CompileReport,
    build_dir: &Path,
) -> Result<(crate::compiler::CompileReport, Option<PathBuf>), String> {
    let report = match report.output_kind() {
        crate::compiler::CompileOutputKind::BuildArtifacts => report,
        crate::compiler::CompileOutputKind::RetainedNativeArtifact => {
            report.publish_retained_native_artifact(build_dir)?
        }
        _ => return Err("compilation did not retain a publishable product".into()),
    };
    let report = report.publish_completed_build_outputs(build_dir)?;
    let executable = report
        .checked_native_executable_path()
        .map(Path::to_path_buf);
    Ok((report, executable))
}

pub fn publish_native_artifact(
    report: crate::compiler::CompileReport,
    build_dir: &Path,
) -> Result<(crate::compiler::CompileReport, PathBuf), String> {
    if report.output_kind() == crate::compiler::CompileOutputKind::BuildArtifacts {
        return Err("an artifact-only build has no executable to run".into());
    }
    let (published, path) = publish_compilation(report, build_dir)?;
    let path =
        path.ok_or_else(|| "native publication did not retain executable custody".to_owned())?;
    Ok((published, path))
}
