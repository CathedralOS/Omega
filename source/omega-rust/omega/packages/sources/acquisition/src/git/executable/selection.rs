//! Operator-owned primary Git selection before package input is inspected.

use super::validation::verify_git_executable_launchability;
use crate::SourceResolveError;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// One primary Git executable frozen before package-controlled input is read.
///
/// The explicit path, when present, is an operator input and takes precedence
/// over `PATH`. Otherwise `PATH` is snapshotted once by [`Self::capture`].
/// Callers should provide every package-controlled workspace, source, build,
/// quarantine, and cache root already known at operation start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimaryGitSelection {
    pub(crate) path: PathBuf,
}

impl PrimaryGitSelection {
    pub fn capture(
        explicit_operator_path: Option<&Path>,
        package_controlled_roots: &[PathBuf],
    ) -> Result<Self, SourceResolveError> {
        Self::capture_optional(explicit_operator_path, package_controlled_roots)?
            .ok_or(SourceResolveError::GitExecutableUnavailable)
    }

    /// Freeze the selected Git when one is available without making Git a
    /// prerequisite for storage sessions that only resolve local sources.
    pub(crate) fn capture_optional(
        explicit_operator_path: Option<&Path>,
        package_controlled_roots: &[PathBuf],
    ) -> Result<Option<Self>, SourceResolveError> {
        let path_snapshot = if explicit_operator_path.is_none() {
            std::env::var_os("PATH")
        } else {
            None
        };
        capture_primary_git_optional(
            explicit_operator_path,
            path_snapshot.as_deref(),
            package_controlled_roots,
        )
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn verify_outside(
        &self,
        package_controlled_roots: &[PathBuf],
    ) -> Result<(), SourceResolveError> {
        let roots = ExcludedRoots::new(package_controlled_roots)?;
        if roots.contains(&self.path) {
            return Err(SourceResolveError::GitExecutableInvalid {
                path: self.path.clone(),
                message: "selected Git executable is inside a package-controlled root".to_owned(),
            });
        }
        Ok(())
    }
}

#[cfg(all(test, unix))]
fn capture_primary_git(
    explicit_operator_path: Option<&Path>,
    path_snapshot: Option<&OsStr>,
    package_controlled_roots: &[PathBuf],
) -> Result<PrimaryGitSelection, SourceResolveError> {
    capture_primary_git_optional(
        explicit_operator_path,
        path_snapshot,
        package_controlled_roots,
    )?
    .ok_or(SourceResolveError::GitExecutableUnavailable)
}

fn capture_primary_git_optional(
    explicit_operator_path: Option<&Path>,
    path_snapshot: Option<&OsStr>,
    package_controlled_roots: &[PathBuf],
) -> Result<Option<PrimaryGitSelection>, SourceResolveError> {
    let roots = ExcludedRoots::new(package_controlled_roots)?;
    if let Some(path) = explicit_operator_path {
        return freeze_explicit_primary_git(path, &roots).map(Some);
    }
    let Some(path_snapshot) = path_snapshot else {
        return Ok(None);
    };
    for directory in std::env::split_paths(path_snapshot) {
        if !directory.is_absolute() {
            continue;
        }
        let candidate = directory.join(automatic_git_file_name());
        if roots.contains(&candidate) {
            continue;
        }
        let Ok(canonical) = candidate.canonicalize() else {
            continue;
        };
        if roots.contains(&canonical) || verify_git_executable_launchability(&canonical).is_err() {
            continue;
        }
        return freeze_canonical_primary_git(canonical).map(Some);
    }
    Ok(None)
}

fn freeze_explicit_primary_git(
    path: &Path,
    roots: &ExcludedRoots,
) -> Result<PrimaryGitSelection, SourceResolveError> {
    if !path.is_absolute() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "explicit operator Git path is not absolute".to_owned(),
        });
    }
    if roots.contains(path) {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "explicit operator Git path is inside a package-controlled root".to_owned(),
        });
    }
    let canonical =
        path.canonicalize()
            .map_err(|error| SourceResolveError::GitExecutableInvalid {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
    if roots.contains(&canonical) {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: canonical,
            message: "explicit operator Git target is inside a package-controlled root".to_owned(),
        });
    }
    verify_git_executable_launchability(&canonical)?;
    freeze_canonical_primary_git(canonical)
}

fn freeze_canonical_primary_git(
    canonical: PathBuf,
) -> Result<PrimaryGitSelection, SourceResolveError> {
    verify_git_executable_launchability(&canonical)?;
    Ok(PrimaryGitSelection { path: canonical })
}

#[cfg(windows)]
fn automatic_git_file_name() -> &'static str {
    "git.exe"
}

#[cfg(not(windows))]
fn automatic_git_file_name() -> &'static str {
    "git"
}

#[derive(Debug)]
struct ExcludedRoots {
    paths: Vec<ExcludedRoot>,
}

impl ExcludedRoots {
    fn new(paths: &[PathBuf]) -> Result<Self, SourceResolveError> {
        let paths = paths
            .iter()
            .map(|path| {
                if !path.is_absolute() {
                    return Err(SourceResolveError::GitExecutableInvalid {
                        path: path.clone(),
                        message: "package-controlled executable exclusion root is not absolute"
                            .to_owned(),
                    });
                }
                Ok(ExcludedRoot {
                    path: path.clone(),
                    canonical: path.canonicalize().ok(),
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { paths })
    }

    fn contains(&self, candidate: &Path) -> bool {
        self.paths.iter().any(|root| {
            candidate.starts_with(&root.path)
                || root
                    .canonical
                    .as_ref()
                    .is_some_and(|canonical| candidate.starts_with(canonical))
        })
    }
}

#[derive(Debug)]
struct ExcludedRoot {
    path: PathBuf,
    canonical: Option<PathBuf>,
}

#[cfg(all(test, unix))]
pub(crate) fn capture_primary_git_from_snapshot(
    explicit_operator_path: Option<&Path>,
    path_snapshot: Option<&OsStr>,
    package_controlled_roots: &[PathBuf],
) -> Result<PrimaryGitSelection, SourceResolveError> {
    capture_primary_git(
        explicit_operator_path,
        path_snapshot,
        package_controlled_roots,
    )
}

pub(crate) fn resolver_package_controlled_roots(
    retained_roots: &[&Path],
) -> Result<Vec<PathBuf>, SourceResolveError> {
    let current_directory = std::env::current_dir().map_err(|error| {
        SourceResolveError::GitExecutionBoundaryInvalid {
            message: format!("could not snapshot the invoking working directory: {error}"),
        }
    })?;
    let mut roots = Vec::with_capacity(retained_roots.len() + 1);
    roots.push(current_directory);
    roots.extend(retained_roots.iter().map(|root| (*root).to_path_buf()));
    Ok(roots)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::executable::validation::is_direct_windows_git_executable;
    #[cfg(unix)]
    use crate::test_support::temp_root;

    #[cfg(unix)]
    fn write_fake_git(directory: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        std::fs::create_dir_all(directory).expect("create fake Git directory");
        let executable = directory.join("git");
        std::fs::write(&executable, b"#!/bin/sh\nexit 0\n").expect("write fake Git");
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
            .expect("make fake Git launchable");
        executable
    }

    #[cfg(unix)]
    #[test]
    fn explicit_operator_setting_precedes_the_path_snapshot_without_fallback() {
        let root = temp_root("primary-git-explicit-precedence");
        let explicit = write_fake_git(&root.join("explicit"));
        let automatic = write_fake_git(&root.join("automatic"));
        let snapshot = std::env::join_paths([automatic.parent().unwrap()])
            .expect("construct constrained PATH snapshot");

        let selected =
            capture_primary_git_from_snapshot(Some(&explicit), Some(snapshot.as_os_str()), &[])
                .expect("explicit operator Git selection");
        assert_eq!(selected.path(), explicit.canonicalize().unwrap());
        assert!(matches!(
            capture_primary_git_from_snapshot(
                Some(Path::new("relative-git")),
                Some(snapshot.as_os_str()),
                &[],
            ),
            Err(SourceResolveError::GitExecutableInvalid { .. })
        ));

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn automatic_selection_uses_only_absolute_path_entries() {
        let root = temp_root("primary-git-absolute-path");
        let automatic = write_fake_git(&root.join("absolute"));
        let snapshot = std::env::join_paths([
            PathBuf::new(),
            PathBuf::from("relative-tools"),
            automatic.parent().unwrap().to_path_buf(),
        ])
        .expect("construct constrained PATH snapshot");

        let selected = capture_primary_git_from_snapshot(None, Some(snapshot.as_os_str()), &[])
            .expect("select from the only absolute PATH entry");
        assert_eq!(selected.path(), automatic.canonicalize().unwrap());

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn automatic_selection_excludes_package_controlled_directories() {
        let root = temp_root("primary-git-package-exclusion");
        let package_root = root.join("package");
        let package_git = write_fake_git(&package_root.join("tools"));
        let host_git = write_fake_git(&root.join("host-tools"));
        let snapshot =
            std::env::join_paths([package_git.parent().unwrap(), host_git.parent().unwrap()])
                .expect("construct package-first PATH snapshot");

        let selected = capture_primary_git_from_snapshot(
            None,
            Some(snapshot.as_os_str()),
            std::slice::from_ref(&package_root),
        )
        .expect("skip package-controlled Git and select host Git");
        assert_eq!(selected.path(), host_git.canonicalize().unwrap());
        let package_only = std::env::join_paths([package_git.parent().unwrap()])
            .expect("construct package-only PATH snapshot");
        assert_eq!(
            capture_primary_git_from_snapshot(
                None,
                Some(package_only.as_os_str()),
                &[package_root],
            ),
            Err(SourceResolveError::GitExecutableUnavailable)
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn managed_git_link_resolves_to_one_frozen_absolute_target() {
        use std::os::unix::fs::symlink;

        let root = temp_root("primary-git-managed-link");
        let target = write_fake_git(&root.join("installation"));
        let link = root.join("managed-git");
        symlink(&target, &link).expect("create managed Git link");

        let selected = capture_primary_git_from_snapshot(Some(&link), None, &[])
            .expect("resolve managed Git link");
        assert_eq!(selected.path(), target.canonicalize().unwrap());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn windows_batch_wrappers_are_not_direct_git_executables() {
        assert!(is_direct_windows_git_executable(Path::new("git.exe")));
        assert!(is_direct_windows_git_executable(Path::new("GIT.EXE")));
        assert!(!is_direct_windows_git_executable(Path::new("git.cmd")));
        assert!(!is_direct_windows_git_executable(Path::new("git.bat")));
        assert!(!is_direct_windows_git_executable(Path::new("git")));
    }
}
