//! Closed Git, transport, and compiler-helper selection.

use super::custody::{verify_git_executable_custody, verify_git_transport_invocation_path};
use super::identity::{
    GitExecutableMetadataIdentity, hash_git_executable, observe_git_executable_metadata,
};
use crate::SourceResolveError;
use crate::git::request::GitExecutionTransport;
use crate::observations::execution::GitTransportExecutableIdentity;
#[cfg(unix)]
use nix::unistd::{Uid, User};
use omega_resolver_execution::RESOLVER_CONNECT_HELPER_BASENAME;
#[cfg(any(test, feature = "test-fixtures"))]
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) struct GitTransportExecutableObservation {
    pub(crate) identity: GitTransportExecutableIdentity,
    pub(crate) metadata_identity: GitExecutableMetadataIdentity,
}

#[cfg(target_os = "macos")]
pub(super) fn open_resolver_execution_helpers(
    execution_transport: GitExecutionTransport,
) -> Result<Vec<GitTransportExecutableObservation>, SourceResolveError> {
    let mut paths = match execution_transport {
        GitExecutionTransport::Ssh => ssh_runtime_shell_paths(),
        GitExecutionTransport::Https => Vec::new(),
        #[cfg(any(test, feature = "test-fixtures"))]
        GitExecutionTransport::File => [
            "/bin/sh",
            "/bin/bash",
            "/bin/mv",
            "/bin/sleep",
            "/usr/bin/git-upload-pack",
        ]
        .into_iter()
        .map(PathBuf::from)
        .collect(),
    };
    if execution_transport == GitExecutionTransport::Ssh {
        paths.push(resolver_connect_helper_path()?);
    }
    paths
        .iter()
        .map(|path| open_git_transport_executable(path))
        .collect()
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(super) fn open_resolver_execution_helpers(
    execution_transport: GitExecutionTransport,
) -> Result<Vec<GitTransportExecutableObservation>, SourceResolveError> {
    if execution_transport != GitExecutionTransport::Ssh {
        return Ok(Vec::new());
    }
    let mut paths = ssh_runtime_shell_paths();
    paths.push(resolver_connect_helper_path()?);
    paths
        .iter()
        .map(|path| open_git_transport_executable(path))
        .collect()
}

#[cfg(windows)]
pub(super) fn open_resolver_execution_helpers(
    execution_transport: GitExecutionTransport,
) -> Result<Vec<GitTransportExecutableObservation>, SourceResolveError> {
    if execution_transport != GitExecutionTransport::Ssh {
        return Ok(Vec::new());
    }
    [resolver_connect_helper_path()?]
        .iter()
        .map(|path| open_git_transport_executable(path))
        .collect()
}

#[cfg(unix)]
fn ssh_runtime_shell_paths() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("/bin/sh")];
    if let Ok(Some(user)) = User::from_uid(Uid::effective())
        && user.shell.is_absolute()
        && !paths.contains(&user.shell)
    {
        paths.push(user.shell);
    }
    paths
}

pub(crate) fn resolver_connect_helper_path() -> Result<PathBuf, SourceResolveError> {
    let current_executable = std::env::current_exe().map_err(|error| {
        SourceResolveError::GitExecutionBoundaryInvalid {
            message: format!("cannot locate the Omega resolver CONNECT helper: {error}"),
        }
    })?;
    let executable_directory = current_executable.parent().ok_or_else(|| {
        SourceResolveError::GitExecutionBoundaryInvalid {
            message: "the running Omega executable has no installation directory".to_owned(),
        }
    })?;
    let helper_name = if cfg!(windows) {
        format!("{RESOLVER_CONNECT_HELPER_BASENAME}.exe")
    } else {
        RESOLVER_CONNECT_HELPER_BASENAME.to_owned()
    };
    let sibling = executable_directory.join(&helper_name);
    if sibling.is_file() {
        return Ok(sibling);
    }
    #[cfg(any(test, feature = "test-fixtures"))]
    {
        if executable_directory.file_name() == Some(OsStr::new("deps")) {
            let cargo_sibling = executable_directory
                .parent()
                .expect("Cargo deps directory has a target-profile parent")
                .join(&helper_name);
            if cargo_sibling.is_file() {
                return Ok(cargo_sibling);
            }
        }
        #[cfg(unix)]
        return Ok(PathBuf::from("/usr/bin/true"));
        #[cfg(windows)]
        return Ok(PathBuf::from(r"C:\Windows\System32\where.exe"));
    }
    #[cfg(not(any(test, feature = "test-fixtures")))]
    Err(SourceResolveError::GitExecutionBoundaryInvalid {
        message: format!(
            "compiler-owned resolver CONNECT helper is missing at {}",
            sibling.display()
        ),
    })
}

pub(super) fn open_ssh_transport_executable(
    git_executable: &Path,
) -> Result<GitTransportExecutableObservation, SourceResolveError> {
    let requested_path = ssh_transport_executable_path(git_executable);
    let mut observation = open_git_transport_executable(&requested_path)?;
    // SSH is supplied through `GIT_SSH_COMMAND`, so invoke the already
    // authenticated canonical target directly rather than retaining an alias.
    observation.identity.invocation_path = observation.identity.path.clone();
    Ok(observation)
}

pub(crate) fn open_https_transport_executable(
    git_executable: &Path,
) -> Result<GitTransportExecutableObservation, SourceResolveError> {
    let candidates = https_transport_executable_candidates(git_executable);
    for requested_path in &candidates {
        match std::fs::symlink_metadata(requested_path) {
            Ok(_) => return open_git_transport_executable(requested_path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(SourceResolveError::GitExecutableInvalid {
                    path: requested_path.clone(),
                    message: format!("HTTPS transport executable is unavailable: {error}"),
                });
            }
        }
    }
    Err(SourceResolveError::GitExecutableInvalid {
        path: git_executable.to_path_buf(),
        message: format!(
            "HTTPS transport executable is unavailable at the closed install-relative candidates: {}",
            candidates
                .iter()
                .map(|candidate| candidate.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    })
}

pub(crate) fn open_git_transport_executable(
    requested_path: &Path,
) -> Result<GitTransportExecutableObservation, SourceResolveError> {
    if !requested_path.is_absolute() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: requested_path.to_path_buf(),
            message: "transport executable path is not absolute".to_owned(),
        });
    }
    let canonical = requested_path.canonicalize().map_err(|error| {
        SourceResolveError::GitExecutableInvalid {
            path: requested_path.to_path_buf(),
            message: format!("transport executable is unavailable: {error}"),
        }
    })?;
    verify_git_transport_invocation_path(requested_path, &canonical)?;
    verify_git_executable_custody(&canonical)?;
    let metadata_identity = observe_git_executable_metadata(&canonical)?;
    let content_identity = hash_git_executable(&canonical)?;
    if observe_git_executable_metadata(&canonical)? != metadata_identity {
        return Err(SourceResolveError::GitExecutableChanged { path: canonical });
    }
    Ok(GitTransportExecutableObservation {
        identity: GitTransportExecutableIdentity {
            invocation_path: requested_path.to_path_buf(),
            path: canonical,
            content_identity,
        },
        metadata_identity,
    })
}

pub(crate) fn verify_git_transport_executable(
    executable: &GitTransportExecutableObservation,
) -> Result<(), SourceResolveError> {
    verify_git_transport_invocation_path(
        &executable.identity.invocation_path,
        &executable.identity.path,
    )?;
    if observe_git_executable_metadata(&executable.identity.path)? != executable.metadata_identity {
        return Err(SourceResolveError::GitExecutableChanged {
            path: executable.identity.path.clone(),
        });
    }
    verify_git_executable_custody(&executable.identity.path)
}

#[cfg(target_os = "macos")]
pub(crate) fn system_git_candidates() -> &'static [&'static str] {
    &[
        "/Library/Developer/CommandLineTools/usr/bin/git",
        "/Applications/Xcode.app/Contents/Developer/usr/bin/git",
        "/usr/local/bin/git",
        "/opt/homebrew/bin/git",
    ]
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn system_git_candidates() -> &'static [&'static str] {
    &["/usr/bin/git", "/usr/local/bin/git"]
}

#[cfg(windows)]
pub(crate) fn system_git_candidates() -> &'static [&'static str] {
    &[
        r"C:\Program Files\Git\cmd\git.exe",
        r"C:\Program Files\Git\bin\git.exe",
        r"C:\Program Files (x86)\Git\cmd\git.exe",
    ]
}

#[cfg(unix)]
fn https_transport_executable_candidates(git_executable: &Path) -> Vec<PathBuf> {
    let Some(installation_root) = git_executable.parent().and_then(Path::parent) else {
        return Vec::new();
    };
    vec![
        installation_root.join("libexec/git-core/git-remote-https"),
        installation_root.join("lib/git-core/git-remote-https"),
    ]
}

#[cfg(windows)]
fn https_transport_executable_candidates(git_executable: &Path) -> Vec<PathBuf> {
    let Some(installation_root) = git_executable.parent().and_then(Path::parent) else {
        return Vec::new();
    };
    vec![installation_root.join("mingw64/libexec/git-core/git-remote-https.exe")]
}

#[cfg(unix)]
fn ssh_transport_executable_path(_git_executable: &Path) -> PathBuf {
    PathBuf::from("/usr/bin/ssh")
}

#[cfg(windows)]
fn ssh_transport_executable_path(git_executable: &Path) -> PathBuf {
    git_executable
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("usr/bin/ssh.exe"))
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files\Git\usr\bin\ssh.exe"))
}
