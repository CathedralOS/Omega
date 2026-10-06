//! Terminal artifact publication.
use crate::CompileReport;
use platform_custody::file_publication::{install_staged_files, stage_exact_bytes};

/// Publish the retained Terminal product as one `<root>.psi` artifact.
pub(super) fn publish(
    report: CompileReport,
    build_dir: &std::path::Path,
) -> Result<CompileReport, String> {
    let artifact = report.retained_terminal_artifact().ok_or_else(|| {
        "terminal publication requires exactly one retained terminal artifact".to_owned()
    })?;
    let stem = report
        .root_path()
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| "terminal publication requires a named root file".to_owned())?;
    std::fs::create_dir_all(build_dir).map_err(|error| {
        format!(
            "failed to create output directory {}: {error}",
            build_dir.display()
        )
    })?;
    let psi_path = build_dir.join(format!("{stem}.psi"));
    let psi_bytes = artifact.artifact().to_bytes();
    let staged_products = vec![stage_exact_bytes(&psi_path, &psi_bytes, false)?];
    install_staged_files(&staged_products)?;
    report.record_terminal_publication()
}
