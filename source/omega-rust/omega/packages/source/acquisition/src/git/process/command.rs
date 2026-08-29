//! Sealed Git command policy and platform-specific helper configuration.

use crate::SourceResolveError;
use crate::git::executable::executor::GitExecutor;
use crate::git::request::GitExecutionTransport;
use crate::local::capture::io_error;
#[cfg(unix)]
use omega_resolver_execution::RESOLVER_CONNECT_HELPER_BASENAME;
use omega_resolver_execution::{
    RESOLVER_CONNECT_BROKER_ENVIRONMENT, RESOLVER_CONNECT_TARGET_ENVIRONMENT,
    ResolverExecutionEndpointRoute, ResolverExecutionPhase, ResolverPreparedExecution,
};
use std::ffi::{OsStr, OsString};
use std::path::Path;

pub(crate) fn sealed_git_command_with_route(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
    endpoint_route: Option<&ResolverExecutionEndpointRoute>,
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

    let network_phase = matches!(
        phase,
        ResolverExecutionPhase::TransportDiscovery | ResolverExecutionPhase::Fetch
    );
    let mut helper_executables = Vec::new();
    if network_phase && let Some(helper) = &executor.transport_executable {
        helper_executables.push(helper.identity.invocation_path.clone());
        if helper.identity.path != helper.identity.invocation_path {
            helper_executables.push(helper.identity.path.clone());
        }
    }
    if network_phase {
        for helper in &executor.execution_helpers {
            helper_executables.push(helper.identity.invocation_path.clone());
            if helper.identity.path != helper.identity.invocation_path {
                helper_executables.push(helper.identity.path.clone());
            }
        }
    }
    let mutable_root = match phase {
        ResolverExecutionPhase::RepositoryInitialization | ResolverExecutionPhase::Fetch => {
            Some(working_directory)
        }
        ResolverExecutionPhase::TransportDiscovery
        | ResolverExecutionPhase::RepositoryInspection => None,
    };
    let network_transport =
        network_phase.then(|| executor.execution_transport.resolver_network_transport());
    let command_result = match phase {
        ResolverExecutionPhase::RepositoryInspection => {
            executor.execution_backend.prepare_inspection(
                &executor.identity.path,
                &helper_executables,
                working_directory,
            )
        }
        ResolverExecutionPhase::TransportDiscovery => executor.execution_backend.prepare_discovery(
            &executor.identity.path,
            &helper_executables,
            network_transport.expect("discovery transport derived from the closed phase"),
            endpoint_route.expect("discovery route opened from the validated request"),
            working_directory,
        ),
        ResolverExecutionPhase::RepositoryInitialization | ResolverExecutionPhase::Fetch => {
            executor.execution_backend.prepare_with_endpoint_route(
                &executor.identity.path,
                &helper_executables,
                phase,
                network_transport,
                endpoint_route,
                mutable_root,
            )
        }
    };
    let mut command =
        command_result.map_err(|error| SourceResolveError::GitExecutionBoundaryInvalid {
            message: error.to_string(),
        })?;
    command
        .env_clear()
        .current_dir(working_directory)
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("PATH", git_helper_path(executor))
        .env(
            "GIT_ALLOW_PROTOCOL",
            executor.execution_transport.allowed_protocol(),
        )
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", null_device())
        .env("GIT_CONFIG_NOSYSTEM", "1")
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
    if network_phase && executor.execution_transport == GitExecutionTransport::Https {
        let helper = executor
            .transport_executable
            .as_ref()
            .expect("validated HTTPS executor retains its transport helper");
        let helper_directory = helper
            .identity
            .invocation_path
            .parent()
            .expect("validated HTTPS helper has an absolute parent");
        command.env("GIT_EXEC_PATH", helper_directory);
        let route = endpoint_route.expect("networked HTTPS command retains an endpoint route");
        command
            .arg("-c")
            .arg(format!("http.proxy={}", route.policy().http_proxy_url()));
    }
    if let Some(transport_executable) = &executor.transport_executable {
        if network_phase && executor.execution_transport == GitExecutionTransport::Ssh {
            let route = endpoint_route.expect("networked SSH command retains an endpoint route");
            let connector = executor
                .resolver_connect_helper()
                .expect("validated SSH executor retains its CONNECT helper");
            let connector_directory = connector
                .identity
                .invocation_path
                .parent()
                .expect("validated CONNECT helper has an absolute parent");
            command
                .env(
                    "GIT_SSH_COMMAND",
                    sealed_ssh_command(&transport_executable.identity.path),
                )
                .env("GIT_SSH_VARIANT", "ssh")
                .env("PATH", connector_directory)
                .env(
                    RESOLVER_CONNECT_BROKER_ENVIRONMENT,
                    route.policy().broker_endpoint().to_string(),
                )
                .env(
                    RESOLVER_CONNECT_TARGET_ENVIRONMENT,
                    route.policy().requested_endpoint().authority(),
                );
        }
    }
    Ok(command)
}

#[cfg(test)]
pub(crate) fn sealed_git_command(
    executor: &GitExecutor,
    working_directory: &Path,
    phase: ResolverExecutionPhase,
) -> Result<ResolverPreparedExecution, SourceResolveError> {
    let route = if matches!(
        phase,
        ResolverExecutionPhase::TransportDiscovery | ResolverExecutionPhase::Fetch
    ) {
        Some(
            executor
                .execution_backend
                .open_endpoint_route(
                    executor.requested_network_endpoint.clone(),
                    executor.network_transfer_budget.clone(),
                )
                .map_err(|error| SourceResolveError::GitExecutionBoundaryInvalid {
                    message: format!("cannot open the compiler-owned endpoint route: {error}"),
                })?,
        )
    } else {
        None
    };
    sealed_git_command_with_route(executor, working_directory, phase, route.as_ref())
}

#[cfg(unix)]
pub(crate) fn git_helper_path(executor: &GitExecutor) -> OsString {
    if executor.execution_transport == GitExecutionTransport::Https {
        return executor
            .transport_executable
            .as_ref()
            .and_then(|helper| helper.identity.invocation_path.parent())
            .map(Path::as_os_str)
            .map(OsStr::to_os_string)
            .unwrap_or_default();
    }
    OsString::from("/usr/bin:/bin")
}

#[cfg(unix)]
pub(crate) fn sealed_ssh_command(ssh_executable: &Path) -> OsString {
    OsString::from(format!(
        "{} -F none -oBatchMode=yes -oPasswordAuthentication=no -oKbdInteractiveAuthentication=no -oNumberOfPasswordPrompts=0 -oStrictHostKeyChecking=yes -oProxyUseFdpass=no -oProxyCommand={}",
        ssh_executable.display(),
        resolver_connect_helper_command_name(),
    ))
}

#[cfg(unix)]
fn resolver_connect_helper_command_name() -> String {
    if cfg!(windows) {
        format!("{RESOLVER_CONNECT_HELPER_BASENAME}.exe")
    } else {
        RESOLVER_CONNECT_HELPER_BASENAME.to_owned()
    }
}

#[cfg(windows)]
pub(crate) fn git_helper_path(executor: &GitExecutor) -> OsString {
    if executor.execution_transport == GitExecutionTransport::Https {
        return executor
            .transport_executable
            .as_ref()
            .and_then(|helper| helper.identity.invocation_path.parent())
            .map(Path::as_os_str)
            .map(OsStr::to_os_string)
            .unwrap_or_default();
    }
    let mut directories = Vec::new();
    if let Some(parent) = executor.identity.path.parent() {
        directories.push(parent.to_path_buf());
        if let Some(root) = parent.parent() {
            directories.push(root.join("bin"));
            directories.push(root.join("usr/bin"));
        }
    }
    std::env::join_paths(directories).unwrap_or_default()
}

#[cfg(windows)]
pub(crate) fn sealed_ssh_command(ssh_executable: &Path) -> OsString {
    OsString::from(format!(
        "\"{}\" -F NUL -oBatchMode=yes -oPasswordAuthentication=no -oKbdInteractiveAuthentication=no -oNumberOfPasswordPrompts=0 -oStrictHostKeyChecking=yes",
        ssh_executable.display()
    ))
}

#[cfg(unix)]
pub(crate) fn null_device() -> &'static str {
    "/dev/null"
}

#[cfg(windows)]
pub(crate) fn null_device() -> &'static str {
    "NUL"
}
