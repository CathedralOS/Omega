use std::path::{Path, PathBuf};

use omega_core::diagnostics::Diagnostic;
use omega_image::{EmittedImageOutput, ImageOutputKind};
use omega_target::NativeTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableFinalization {
    pub executable_path: PathBuf,
    pub status: ExecutableFinalizationStatus,
    pub command: Vec<String>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutableFinalizationStatus {
    AlreadyExecutable,
}

pub fn finalize_emitted_image_output(
    target: NativeTarget,
    emitted_output: &EmittedImageOutput,
    output_path: &Path,
) -> Result<ExecutableFinalization, Diagnostic> {
    if emitted_output.kind == ImageOutputKind::DirectExecutable {
        mark_executable_if_needed(output_path)?;
        return Ok(ExecutableFinalization {
            executable_path: output_path.to_path_buf(),
            status: ExecutableFinalizationStatus::AlreadyExecutable,
            command: Vec::new(),
            stdout: "native output is already an executable image".to_owned(),
            stderr: String::new(),
        });
    }

    Err(Diagnostic::error(format!(
        "native output `{}` is {:?} for {:?}; Omega does not invoke external linkers, so this target must emit a direct executable image",
        emitted_output.format, emitted_output.kind, target
    )))
}

#[cfg(unix)]
fn mark_executable_if_needed(path: &Path) -> Result<(), Diagnostic> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .map_err(|error| {
            Diagnostic::error(format!(
                "failed to read executable permissions {}: {error}",
                path.display()
            ))
        })?
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).map_err(|error| {
        Diagnostic::error(format!(
            "failed to mark executable {}: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn mark_executable_if_needed(_path: &Path) -> Result<(), Diagnostic> {
    Ok(())
}
