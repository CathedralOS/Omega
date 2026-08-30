//! Sealed Git command policy over host-routed transport.

use crate::SourceResolveError;
use crate::git::executable::executor::GitExecutor;
use crate::git::request::GitExecutionTransport;
use crate::tree::filesystem::io_error;
use omega_resolver_execution::{ResolverExecutionPhase, ResolverPreparedExecution};
use std::path::Path;

pub(crate) fn sealed_git_command(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
) -> Result<ResolverPreparedExecution, SourceResolveError> {
    executor.verify()?;
    if !working_directory.is_absolute() {
        return Err(SourceResolveError::Git {
            operation: "command configuration".to_owned(),
            status: None,
            stderr: format!(
                "working directory `{}` is not absolute",
                working_directory.display()
            ),
        });
    }
    let metadata =
        std::fs::metadata(working_directory).map_err(|error| io_error(working_directory, error))?;
    if !metadata.is_dir() {
        return Err(SourceResolveError::NotDirectory {
            path: working_directory.to_path_buf(),
        });
    }

    let mutable_root = match phase {
        ResolverExecutionPhase::RepositoryInitialization | ResolverExecutionPhase::Fetch => {
            Some(working_directory)
        }
        ResolverExecutionPhase::TransportDiscovery
        | ResolverExecutionPhase::RepositoryInspection => None,
    };
    let command_result = match phase {
        ResolverExecutionPhase::RepositoryInspection => executor
            .execution_backend
            .prepare_host_git_inspection(&executor.identity.path, working_directory),
        ResolverExecutionPhase::TransportDiscovery => executor
            .execution_backend
            .prepare_host_git_discovery(&executor.identity.path, working_directory),
        ResolverExecutionPhase::RepositoryInitialization | ResolverExecutionPhase::Fetch => {
            executor.execution_backend.prepare_host_git(
                &executor.identity.path,
                phase,
                mutable_root,
            )
        }
    };
    let mut command =
        command_result.map_err(|error| SourceResolveError::GitExecutionBoundaryInvalid {
            message: error.to_string(),
        })?;
    command
        .current_dir(working_directory)
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env(
            "GIT_ALLOW_PROTOCOL",
            executor.execution_transport.allowed_protocol(),
        )
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_LFS_SKIP_SMUDGE", "1")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_PROTOCOL_FROM_USER", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .arg("--no-replace-objects")
        .arg("-c")
        .arg(format!("core.hooksPath={}", null_device()))
        .args(["-c", "protocol.allow=never"])
        .arg("-c")
        .arg(format!("protocol.file.allow={}", {
            #[cfg(any(test, feature = "test-fixtures"))]
            {
                executor
                    .execution_transport
                    .permits(GitExecutionTransport::File)
            }
            #[cfg(not(any(test, feature = "test-fixtures")))]
            {
                "never"
            }
        }))
        .args(["-c", "protocol.http.allow=never"])
        .arg("-c")
        .arg(format!(
            "protocol.https.allow={}",
            executor
                .execution_transport
                .permits(GitExecutionTransport::Https)
        ))
        .arg("-c")
        .arg(format!(
            "protocol.ssh.allow={}",
            executor
                .execution_transport
                .permits(GitExecutionTransport::Ssh)
        ))
        .args([
            "-c",
            "protocol.git.allow=never",
            "-c",
            "protocol.ext.allow=never",
            "-c",
            "http.followRedirects=false",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.autocrlf=false",
            "-c",
            "gc.auto=0",
            "-c",
            "maintenance.auto=false",
            "-c",
            "fetch.fsckObjects=true",
            "-c",
            "transfer.fsckObjects=true",
            "-c",
            "fetch.recurseSubmodules=false",
            "-c",
            "submodule.recurse=false",
            "-c",
            "filter.lfs.smudge=",
            "-c",
            "filter.lfs.required=false",
        ]);
    Ok(command)
}

#[cfg(unix)]
pub(crate) fn null_device() -> &'static str {
    "/dev/null"
}

#[cfg(windows)]
pub(crate) fn null_device() -> &'static str {
    "NUL"
}
