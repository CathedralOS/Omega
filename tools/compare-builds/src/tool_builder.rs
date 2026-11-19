//! Builds one version's measurement tool one way, in a folder nothing else builds into.

use crate::build_setting::BuildSetting;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Builds `tool_name`, an example of `package_name`, from the workspace in `workspace_directory`,
/// and returns the path of the program it produced.
///
/// `target_directory` must belong to this version and this setting alone. Given a folder another
/// version already built into, Cargo can decide nothing changed and hand back that version's
/// binary: it compares file names and timestamps, and two copies of a workspace share both.
pub fn build_tool(
    workspace_directory: &Path,
    package_name: &str,
    tool_name: &str,
    build_setting: BuildSetting,
    target_directory: &Path,
) -> Result<PathBuf, String> {
    let mut build_command = Command::new("cargo");

    build_command
        .current_dir(workspace_directory)
        .args([
            "build",
            "--release",
            "--package",
            package_name,
            "--example",
            tool_name,
            "--target-dir",
        ])
        .arg(target_directory);

    // An ordinary build must not pick up flags from whoever is running this, or the plain
    // setting would not be plain.
    if build_setting.get_rust_flags().is_empty() {
        build_command.env_remove("RUSTFLAGS");
    } else {
        build_command.env("RUSTFLAGS", build_setting.get_rust_flags());
    }

    let build_output = build_command
        .output()
        .map_err(|io_error| format!("Cannot run cargo: {io_error}."))?;

    if !build_output.status.success() {
        return Err(format!(
            "The {} build in {} failed:\n{}",
            build_setting.get_label(),
            workspace_directory.display(),
            String::from_utf8_lossy(&build_output.stderr).trim()
        ));
    }

    let executable_path = target_directory
        .join("release")
        .join("examples")
        .join(format!("{tool_name}{}", std::env::consts::EXE_SUFFIX));

    if !executable_path.is_file() {
        return Err(format!("The build succeeded but {} is not there.", executable_path.display()));
    }

    Ok(executable_path)
}
