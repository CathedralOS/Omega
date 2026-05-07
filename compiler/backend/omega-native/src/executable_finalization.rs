use std::path::{Path, PathBuf};
use std::process::Command;

use crate::emitter::{EmittedNativeOutput, NativeOutputKind};
use crate::plan::NativePlan;
use crate::target::{Architecture, ObjectFormat};
use omega_core::diagnostics::Diagnostic;

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
    UsedExternalLinker,
    AlreadyExecutable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExternalLinkerInvocation {
    program: String,
    arguments: Vec<String>,
    executable_path: PathBuf,
}

pub fn finalize_native_output(
    native_plan: &NativePlan,
    emitted_output: &EmittedNativeOutput,
    output_path: &Path,
    build_dir: &Path,
) -> Result<ExecutableFinalization, Diagnostic> {
    if emitted_output.kind == NativeOutputKind::DirectExecutable {
        mark_executable_if_needed(output_path)?;
        return Ok(ExecutableFinalization {
            executable_path: output_path.to_path_buf(),
            status: ExecutableFinalizationStatus::AlreadyExecutable,
            command: Vec::new(),
            stdout: "native output is already an executable image".to_owned(),
            stderr: String::new(),
        });
    }

    let Some(invocation) = plan_external_linker_invocation(native_plan, output_path, build_dir)
    else {
        return Err(Diagnostic::error(format!(
            "native output `{}` is {:?}, but no executable finalizer exists for {:?}",
            emitted_output.format, emitted_output.kind, native_plan.target
        )));
    };

    let command = external_linker_command(&invocation);
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

    Ok(ExecutableFinalization {
        executable_path: invocation.executable_path,
        status: ExecutableFinalizationStatus::UsedExternalLinker,
        command,
        stdout,
        stderr,
    })
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

fn plan_external_linker_invocation(
    native_plan: &NativePlan,
    output_path: &Path,
    build_dir: &Path,
) -> Option<ExternalLinkerInvocation> {
    if native_plan.target.object_format == ObjectFormat::MachO
        && native_plan.target.architecture == Architecture::Aarch64
        && cfg!(target_os = "macos")
        && cfg!(target_arch = "aarch64")
    {
        let executable_path = build_dir.join("omega-program");
        return Some(ExternalLinkerInvocation {
            program: "cc".to_owned(),
            arguments: vec![
                output_path.display().to_string(),
                "-o".to_owned(),
                executable_path.display().to_string(),
            ],
            executable_path,
        });
    }

    None
}

fn external_linker_command(invocation: &ExternalLinkerInvocation) -> Vec<String> {
    let mut command = vec![invocation.program.clone()];
    command.extend(invocation.arguments.iter().cloned());
    command
}
