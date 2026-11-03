//! Retain replay evidence before publication consumes its native product.
//!
//! Write the actual canonical product and a compact native identity. Debugging
//! the full native evidence graph here can expand a small image into gigabytes.

use super::RunError;
use compiler::CompileReport;
use std::path::Path;

pub(super) fn retain(report: &CompileReport, directory: &Path) -> Result<(), RunError> {
    // A Terminal-only run has no native identity to record.
    if report.retained_native_program().is_none()
        && let Some(artifact) = report.artifact()
    {
        return std::fs::write(directory.join("program.oterm"), artifact.to_bytes())
            .map_err(RunError::ReferenceOutput);
    }
    let program = report.retained_native_program().ok_or_else(|| {
        RunError::ReferenceOutput(std::io::Error::other(
            "native report lost its retained program before artifact retention",
        ))
    })?;
    std::fs::write(
        directory.join("program.oterm"),
        program.psi_artifact().to_bytes(),
    )
    .map_err(RunError::ReferenceOutput)?;
    std::fs::write(
        directory.join("native.identity.txt"),
        format!("{}\n", program.identity()),
    )
    .map_err(RunError::ReferenceOutput)
}
