//! Runs a built measurement tool once and reads the result line it printed.

use measure::ResultLine;
use std::path::Path;
use std::process::Command;

pub fn run_tool(
    executable_path: &Path,
    tool_arguments: &[String],
) -> Result<ResultLine, String> {
    let tool_output = Command::new(executable_path)
        .args(tool_arguments)
        .output()
        .map_err(|io_error| format!("Cannot run {}: {io_error}.", executable_path.display()))?;

    if !tool_output.status.success() {
        return Err(format!(
            "{} failed:\n{}",
            executable_path.display(),
            String::from_utf8_lossy(&tool_output.stderr).trim()
        ));
    }

    let printed_output = String::from_utf8_lossy(&tool_output.stdout);

    ResultLine::find_in_output(&printed_output).ok_or_else(|| {
        format!(
            "{} printed no RESULT line. A tool has to end with one, built with the `measure` crate, before its version can be compared.",
            executable_path.display()
        )
    })
}
