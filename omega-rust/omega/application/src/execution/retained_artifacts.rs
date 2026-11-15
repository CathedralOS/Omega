// Write `program.oterm` and `native.identity.txt` to the build directory when `omega run --keep` is set, before publication consumes the retained native program.
//
// We write only the compact `program.identity()` line rather than debug-dumping
// the full native evidence graph, which can expand a small executable into
// gigabytes of text. A `--engine psi` run has no native program, so it
// writes only `program.oterm`.

use super::RunError;
use compiler::CompileReport;
use std::path::Path;

pub(super) fn retain(report: &CompileReport, directory: &Path) -> Result<(), RunError> {
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
