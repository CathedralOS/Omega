//! Bounded allocation and string validation shared by baseline codecs.

use crate::review::baseline::ReviewOnlyBaselineError;

pub(in crate::review::baseline) fn clone_baseline_bytes(
    bytes: &[u8],
    allocation_error: &'static str,
) -> Result<Vec<u8>, ReviewOnlyBaselineError> {
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(bytes.len())
        .map_err(|_| ReviewOnlyBaselineError::new(allocation_error))?;
    owned.extend_from_slice(bytes);
    Ok(owned)
}

pub(in crate::review::baseline) fn ensure_bounded_string(
    value: &str,
    maximum_bytes: usize,
    error: &'static str,
) -> Result<(), ReviewOnlyBaselineError> {
    if value.is_empty() || value.len() > maximum_bytes {
        Err(ReviewOnlyBaselineError::new(error))
    } else {
        Ok(())
    }
}
