//! Save the interpreted artifact for `omega run --keep`.

use super::RunError;
use super::compilation::RunProduct;
use std::path::Path;

pub(super) fn retain(product: &RunProduct, directory: &Path) -> Result<(), RunError> {
    let program = product.artifact().ok_or_else(|| {
        RunError::ReferenceOutput(std::io::Error::other(
            "run lost its produced Psi program before artifact retention",
        ))
    })?;
    std::fs::write(directory.join("program.oterm"), program.artifact.to_bytes())
        .map_err(RunError::ReferenceOutput)?;
    Ok(())
}
