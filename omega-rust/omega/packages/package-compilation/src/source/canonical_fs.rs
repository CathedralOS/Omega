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

/// The two authored Source file modes: plain files are read-only, executables
/// are read-execute. A widened physical mode impersonates neither.
pub(crate) const CANONICAL_FILE_MODE: u32 = 0o444;
pub(crate) const CANONICAL_EXECUTABLE_FILE_MODE: u32 = 0o555;

/// The executable bit of a canonical Source file: only the two authored modes
/// are legal, so the flag is the mode itself. Non-unix hosts carry no mode
/// surface and always report false.
pub(crate) fn canonical_file_executable(
    path: &std::path::Path,
    metadata: &std::fs::Metadata,
) -> Result<bool, String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        match metadata.permissions().mode() & 0o777 {
            CANONICAL_FILE_MODE => Ok(false),
            CANONICAL_EXECUTABLE_FILE_MODE => Ok(true),
            mode => Err(format!(
                "physical Source file {} has noncanonical mode {mode:#o}",
                path.display()
            )),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (path, metadata);
        Ok(false)
    }
}

/// Read one canonical symlink target and charge its spelling against the
/// caller's aggregate ceiling — the sequence every metadata walker repeats.
pub(crate) fn canonical_symlink_target(
    path: &std::path::Path,
    aggregate_content_bytes: &mut u64,
    ceiling: u64,
) -> Result<(Vec<u8>, u64), String> {
    let target = std::fs::read_link(path).map_err(|error| {
        format!(
            "could not read canonical Source symlink {}: {error}",
            path.display()
        )
    })?;
    let target = os_str_bytes(target.as_os_str())?;
    let target_length = u64::try_from(target.len())
        .map_err(|_| "canonical Source symlink target length exceeds u64".to_owned())?;
    *aggregate_content_bytes = aggregate_content_bytes
        .checked_add(target_length)
        .filter(|total| *total <= ceiling)
        .ok_or_else(|| format!("canonical Source content exceeds its {ceiling}-byte ceiling"))?;
    Ok((target, target_length))
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
