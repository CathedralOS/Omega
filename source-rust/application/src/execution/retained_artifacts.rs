// Write `program.oterm` and `native.identity.txt` to the build directory when `omega run --keep` is set, before publication consumes the retained native program.
//
// We write only the compact `program.identity()` line rather than debug-dumping
// the full native evidence graph, which can expand a small executable into
// gigabytes of text. A `--engine psi` run has no native program, so it
// writes only `program.oterm`.

use super::RunError;
use super::compilation::RunProduct;
use std::path::Path;

pub(super) fn retain(product: &RunProduct, directory: &Path) -> Result<(), RunError> {
    let program = product.artifact().ok_or_else(|| {
        RunError::ReferenceOutput(std::io::Error::other(
            "run lost its produced Terminal program before artifact retention",
        ))
    })?;
    std::fs::write(directory.join("program.oterm"), program.artifact.to_bytes())
        .map_err(RunError::ReferenceOutput)?;
    if let Some(identity) = program.native_identity {
        std::fs::write(
            directory.join("native.identity.txt"),
            format!("{identity}\n"),
        )
        .map_err(RunError::ReferenceOutput)?;
    }
    Ok(())
}
