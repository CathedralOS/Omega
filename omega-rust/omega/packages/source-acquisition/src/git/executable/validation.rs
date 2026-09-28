//! Ordinary regular-file and launchability checks for the selected primary Git.

use crate::SourceResolveError;
use std::path::Path;

pub(super) fn verify_git_executable_launchability(path: &Path) -> Result<(), SourceResolveError> {
    let metadata =
        std::fs::metadata(path).map_err(|error| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    if !metadata.is_file() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "selected Git executable is not a regular file".to_owned(),
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(SourceResolveError::GitExecutableInvalid {
                path: path.to_path_buf(),
                message: "selected Git file is not executable".to_owned(),
            });
        }
    }
    #[cfg(windows)]
    {
        if !is_direct_windows_git_executable(path) {
            return Err(SourceResolveError::GitExecutableInvalid {
                path: path.to_path_buf(),
                message: "selected Git must be a directly executable git.exe, not a batch wrapper"
                    .to_owned(),
            });
        }
    }
    Ok(())
}

#[cfg(any(windows, test))]
pub(crate) fn is_direct_windows_git_executable(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("git.exe"))
}
