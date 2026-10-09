//! Host-filesystem byte and mode handling shared by the canonical Source
//! layouts. Unix takes the raw byte/mode surfaces; other hosts go through
//! portable UTF-8.

#[cfg(unix)]
use std::path::Path;

/// Reject a directory whose permission bits are not the canonical Source
/// mode. Checked on every trusted-tree entry so a widened physical layout
/// cannot impersonate the canonical one.
#[cfg(unix)]
pub(crate) fn require_canonical_mode(
    path: &Path,
    metadata: &std::fs::Metadata,
    expected: u32,
) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mode = metadata.permissions().mode() & 0o777;
    if mode == expected {
        Ok(())
    } else {
        Err(format!(
            "physical Source directory {} has noncanonical mode {mode:#o}; expected {expected:#o}",
            path.display()
        ))
    }
}

#[cfg(unix)]
pub(crate) fn os_str_bytes(value: &std::ffi::OsStr) -> Result<Vec<u8>, String> {
    use std::os::unix::ffi::OsStrExt;
    Ok(value.as_bytes().to_vec())
}

#[cfg(not(unix))]
pub(crate) fn os_str_bytes(value: &std::ffi::OsStr) -> Result<Vec<u8>, String> {
    value
        .to_str()
        .map(|value| value.as_bytes().to_vec())
        .ok_or_else(|| "physical Source path is not portable UTF-8".to_owned())
}

/// Reject the next canonical-metadata row when the walk would cross the
/// shared row ceiling — both walkers bound `rows + stack + 1` identically.
pub(crate) fn check_metadata_row_ceiling(rows: usize, stack: usize) -> Result<(), String> {
    if rows
        .checked_add(stack)
        .and_then(|count| count.checked_add(1))
        .is_none_or(|count| count > filesystem_authority::CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT)
    {
        return Err(format!(
            "canonical Source metadata exceeds its {}-row ceiling",
            filesystem_authority::CANONICAL_FILESYSTEM_METADATA_ROW_LIMIT
        ));
    }
    Ok(())
}
