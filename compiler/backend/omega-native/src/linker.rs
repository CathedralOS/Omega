use std::path::{Path, PathBuf};
use std::process::Command;

use crate::plan::NativePlan;
use crate::target::{Architecture, ObjectFormat};
use omega_core::diagnostics::Diagnostic;

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkInvocation {
    program: String,
    arguments: Vec<String>,
    executable_path: PathBuf,
}

pub fn link_native_object(
    native_plan: &NativePlan,
    object_path: &Path,
    build_dir: &Path,
) -> Result<LinkOutput, Diagnostic> {
    let Some(invocation) = plan_link_invocation(native_plan, object_path, build_dir) else {
        mark_direct_executable_if_needed(object_path)?;
        return Ok(LinkOutput {
            executable_path: object_path.to_path_buf(),
            status: LinkStatus::Skipped,
            command: Vec::new(),
            stdout: String::new(),
            stderr: "link skipped: target object cannot be linked by this host yet".to_owned(),
        });
    };

    let command = link_command(&invocation);
    let output = Command::new(&invocation.program)
        .args(&invocation.arguments)
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
        executable_path: invocation.executable_path,
        status: LinkStatus::Linked,
        command,
        stdout,
        stderr,
    })
}

fn mark_direct_executable_if_needed(path: &Path) -> Result<(), Diagnostic> {
    if path
        .file_name()
        .is_some_and(|file_name| file_name == "omega-program")
    {
        mark_executable(path)?;
    }

    Ok(())
}

#[cfg(unix)]
fn mark_executable(path: &Path) -> Result<(), Diagnostic> {
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
fn mark_executable(_path: &Path) -> Result<(), Diagnostic> {
    Ok(())
}

fn plan_link_invocation(
    native_plan: &NativePlan,
    object_path: &Path,
    build_dir: &Path,
) -> Option<LinkInvocation> {
    if native_plan.target.object_format == ObjectFormat::MachO
        && native_plan.target.architecture == Architecture::Aarch64
        && cfg!(target_os = "macos")
        && cfg!(target_arch = "aarch64")
    {
        let executable_path = build_dir.join("omega-program");
        return Some(LinkInvocation {
            program: "cc".to_owned(),
            arguments: vec![
                object_path.display().to_string(),
                "-o".to_owned(),
                executable_path.display().to_string(),
            ],
            executable_path,
        });
    }

    None
}

fn link_command(invocation: &LinkInvocation) -> Vec<String> {
    let mut command = vec![invocation.program.clone()];
    command.extend(invocation.arguments.iter().cloned());
    command
}
