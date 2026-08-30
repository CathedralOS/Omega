//! Operator-owned primary Git selection.

use crate::SourceResolveError;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// One absolute primary Git path frozen before package-controlled input is read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PrimaryGitSelection {
    path: PathBuf,
}

impl PrimaryGitSelection {
    pub(crate) fn from_operator_or_environment(
        explicit: Option<&Path>,
        excluded_roots: &[PathBuf],
    ) -> Result<Option<Self>, SourceResolveError> {
        let path_snapshot = std::env::var_os("PATH");
        Self::from_operator_or_snapshot(explicit, path_snapshot.as_deref(), excluded_roots)
    }

    fn from_operator_or_snapshot(
        explicit: Option<&Path>,
        path_snapshot: Option<&OsStr>,
        excluded_roots: &[PathBuf],
    ) -> Result<Option<Self>, SourceResolveError> {
        match explicit {
            Some(path) => Self::select_explicit(path, excluded_roots).map(Some),
            None => {
                Ok(path_snapshot
                    .and_then(|snapshot| Self::select_automatic(snapshot, excluded_roots)))
            }
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    fn select_explicit(
        path: &Path,
        excluded_roots: &[PathBuf],
    ) -> Result<Self, SourceResolveError> {
        if !path.is_absolute() {
            return Err(invalid_selection(path, "operator Git path is not absolute"));
        }
        if !direct_executable_name_is_allowed(path, false) {
            return Err(invalid_selection(
                path,
                "operator Git path names a command wrapper rather than a direct executable",
            ));
        }
        let canonical = path
            .canonicalize()
            .map_err(|error| invalid_selection(path, &error.to_string()))?;
        validate_candidate(path, &canonical, excluded_roots)?;
        Ok(Self { path: canonical })
    }

    fn select_automatic(snapshot: &OsStr, excluded_roots: &[PathBuf]) -> Option<Self> {
        for directory in std::env::split_paths(snapshot) {
            if !directory.is_absolute() {
                continue;
            }
            let candidate = directory.join(automatic_executable_name());
            if !direct_executable_name_is_allowed(&candidate, true) {
                continue;
            }
            let Ok(canonical) = candidate.canonicalize() else {
                continue;
            };
            if validate_candidate(&candidate, &canonical, excluded_roots).is_ok() {
                return Some(Self { path: canonical });
            }
        }
        None
    }
}

#[cfg(windows)]
fn automatic_executable_name() -> &'static str {
    "git.exe"
}

#[cfg(not(windows))]
fn automatic_executable_name() -> &'static str {
    "git"
}

fn direct_executable_name_is_allowed(path: &Path, automatic: bool) -> bool {
    #[cfg(windows)]
    {
        windows_direct_executable_name_is_allowed(path, automatic)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, automatic);
        true
    }
}

#[cfg(any(windows, test))]
fn windows_direct_executable_name_is_allowed(path: &Path, automatic: bool) -> bool {
    let Some(name) = path.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    if automatic {
        return name.eq_ignore_ascii_case("git.exe");
    }
    !name.to_ascii_lowercase().ends_with(".bat") && !name.to_ascii_lowercase().ends_with(".cmd")
}

fn validate_candidate(
    authored: &Path,
    canonical: &Path,
    excluded_roots: &[PathBuf],
) -> Result<(), SourceResolveError> {
    if excluded_roots
        .iter()
        .any(|root| authored.starts_with(root) || canonical.starts_with(root))
    {
        return Err(invalid_selection(
            authored,
            "Git executable is inside resolver-controlled storage",
        ));
    }
    let metadata = std::fs::metadata(canonical)
        .map_err(|error| invalid_selection(authored, &error.to_string()))?;
    if !metadata.is_file() {
        return Err(invalid_selection(
            authored,
            "Git executable is not a regular file",
        ));
    }
    if !metadata_is_launchable(&metadata) {
        return Err(invalid_selection(
            authored,
            "Git executable is not directly launchable",
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn metadata_is_launchable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn metadata_is_launchable(_metadata: &std::fs::Metadata) -> bool {
    true
}

fn invalid_selection(path: &Path, message: &str) -> SourceResolveError {
    SourceResolveError::GitExecutableInvalid {
        path: path.to_path_buf(),
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_root;

    #[cfg(unix)]
    fn write_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path, b"#!/bin/sh\nexit 0\n").expect("write executable");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .expect("make executable launchable");
    }

    #[cfg(unix)]
    #[test]
    fn explicit_selection_precedes_the_captured_path_snapshot() {
        let root = temp_root("primary-git-explicit");
        let automatic = root.join("automatic");
        std::fs::create_dir_all(&automatic).expect("create automatic fixture directory");
        let explicit = root.join("explicit-git");
        write_executable(&explicit);
        write_executable(&automatic.join("git"));
        let snapshot = std::env::join_paths([automatic.as_path()]).unwrap();

        let selection =
            PrimaryGitSelection::from_operator_or_snapshot(Some(&explicit), Some(&snapshot), &[])
                .expect("select explicit executable")
                .expect("retain explicit selection");
        assert_eq!(selection.path(), explicit.canonicalize().unwrap());

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn automatic_selection_ignores_relative_entries_and_controlled_roots() {
        let root = temp_root("primary-git-path");
        let controlled = root.join("controlled");
        let accepted = root.join("accepted");
        std::fs::create_dir_all(&controlled).expect("create controlled directory");
        std::fs::create_dir_all(&accepted).expect("create accepted directory");
        write_executable(&controlled.join("git"));
        write_executable(&accepted.join("git"));
        let snapshot = std::env::join_paths([
            Path::new(""),
            Path::new("relative"),
            controlled.as_path(),
            accepted.as_path(),
        ])
        .expect("join PATH fixture");

        let selection =
            PrimaryGitSelection::select_automatic(&snapshot, &[controlled.canonicalize().unwrap()])
                .expect("select the first admissible absolute entry");
        assert_eq!(
            selection.path(),
            accepted.join("git").canonicalize().unwrap()
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn selection_is_frozen_against_later_path_snapshots() {
        let root = temp_root("primary-git-frozen");
        let first = root.join("first");
        let second = root.join("second");
        std::fs::create_dir_all(&first).expect("create first directory");
        std::fs::create_dir_all(&second).expect("create second directory");
        write_executable(&first.join("git"));
        write_executable(&second.join("git"));
        let first_snapshot = std::env::join_paths([first.as_path()]).unwrap();
        let second_snapshot = std::env::join_paths([second.as_path()]).unwrap();

        let selection = PrimaryGitSelection::select_automatic(&first_snapshot, &[]).unwrap();
        assert_eq!(selection.path(), first.join("git").canonicalize().unwrap());
        assert_ne!(
            selection.path(),
            PrimaryGitSelection::select_automatic(&second_snapshot, &[])
                .unwrap()
                .path()
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn windows_automatic_selection_rejects_command_wrappers() {
        assert!(windows_direct_executable_name_is_allowed(
            Path::new("git.exe"),
            true,
        ));
        assert!(!windows_direct_executable_name_is_allowed(
            Path::new("git.bat"),
            true,
        ));
        assert!(!windows_direct_executable_name_is_allowed(
            Path::new("git.cmd"),
            false,
        ));
    }
}
