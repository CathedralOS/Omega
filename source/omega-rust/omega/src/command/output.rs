//! Product-owned publication of compiler outputs.
//!
//! The compiler stops at an in-memory semantic product. Choosing a path and
//! making bytes visible is command/product policy, not compilation.

use std::path::{Path, PathBuf};

pub(super) fn publish_native_artifact(
    report: omega_compiler::CompileReport,
    build_dir: &Path,
) -> Result<PathBuf, String> {
    let artifact = report.into_retained_native_artifact().ok_or_else(|| {
        "native output publication requires a retained native compiler artifact".to_owned()
    })?;
    artifact
        .validate()
        .map_err(|error| format!("refusing to publish an invalid native artifact: {error}"))?;

    let output = artifact.image().output();
    if Path::new(&output.file_name).components().count() != 1 {
        return Err("native artifact supplied a non-local output filename".to_owned());
    }
    std::fs::create_dir_all(build_dir).map_err(|error| {
        format!(
            "failed to create output directory {}: {error}",
            build_dir.display()
        )
    })?;
    let path = build_dir.join(&output.file_name);
    std::fs::write(&path, &output.bytes)
        .map_err(|error| format!("failed to publish {}: {error}", path.display()))?;
    make_executable(&path)?;
    Ok(path)
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("failed to make {} executable: {error}", path.display()))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}
