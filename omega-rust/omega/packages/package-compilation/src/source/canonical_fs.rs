// Host-filesystem permission mode, path-byte, and row-ceiling helpers shared by canonical `Source` walkers.
//
// On Unix, physical `Source` directories must carry exact mode `0o555` and files must
// carry `0o444` (non-executable) or `0o555` (executable) so widened host permissions
// cannot impersonate a sealed canonical tree. Non-Unix hosts carry no POSIX mode
// surface, report `false` for the executable bit, and require portable UTF-8 path
// spellings.

#[cfg(unix)]
use std::path::Path;

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
pub(crate) const CANONICAL_FILE_MODE: u32 = 0o444;
#[cfg(unix)]
pub(crate) const CANONICAL_EXECUTABLE_FILE_MODE: u32 = 0o555;

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
