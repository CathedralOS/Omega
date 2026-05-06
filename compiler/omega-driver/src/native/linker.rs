use std::path::{Path, PathBuf};
use std::process::Command;

use crate::diagnostics::Diagnostic;
use crate::native::plan::NativePlan;
use crate::native::target::{Architecture, ObjectFormat};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkOutput {
    pub executable_path: PathBuf,
    pub status: LinkStatus,
    pub command: Vec<String>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkStatus {
    Linked,
    Skipped,
}

pub fn link_native_object(
    native_plan: &NativePlan,
    object_path: &Path,
    build_dir: &Path,
) -> Result<LinkOutput, Diagnostic> {
    let executable_path = build_dir.join("omega-program");

    if !can_link_on_this_host(native_plan) {
        return Ok(LinkOutput {
            executable_path: object_path.to_path_buf(),
            status: LinkStatus::Skipped,
            command: Vec::new(),
            stdout: String::new(),
            stderr: "link skipped: target object cannot be linked by this host yet".to_owned(),
        });
    }

    let command = vec![
        "cc".to_owned(),
        object_path.display().to_string(),
        "-o".to_owned(),
        executable_path.display().to_string(),
    ];
    let output = Command::new("cc")
        .arg(object_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .map_err(|error| Diagnostic::error(format!("failed to invoke system linker: {error}")))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if !output.status.success() {
        return Err(Diagnostic::error(format!(
            "system linker failed with status {}:\n{}",
            output.status, stderr
        )));
    }

    Ok(LinkOutput {
        executable_path,
        status: LinkStatus::Linked,
        command,
        stdout,
        stderr,
    })
}

fn can_link_on_this_host(native_plan: &NativePlan) -> bool {
    native_plan.target.object_format == ObjectFormat::MachO
        && native_plan.target.architecture == Architecture::Aarch64
        && cfg!(target_os = "macos")
        && cfg!(target_arch = "aarch64")
}
