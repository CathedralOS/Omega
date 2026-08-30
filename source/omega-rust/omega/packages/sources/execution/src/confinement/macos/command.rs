use super::policy::SeatbeltPolicy;
use crate::backend::{ResolverExecutionAuthorityRoots, ResolverExecutionBackend};
use crate::model::{
    ResolverExecutionBackendIdentity, ResolverExecutionNetworkTransport, ResolverExecutionPhase,
};
use crate::network::ResolverExecutionEndpointRoutePolicy;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) fn command(
    backend: &ResolverExecutionBackend,
    executable: &Path,
    additional_executables: &[PathBuf],
    phase: ResolverExecutionPhase,
    network_transport: Option<ResolverExecutionNetworkTransport>,
    endpoint_route: Option<&ResolverExecutionEndpointRoutePolicy>,
    roots: ResolverExecutionAuthorityRoots<'_>,
) -> io::Result<(Command, Option<String>)> {
    let ResolverExecutionBackendIdentity::MacosSeatbelt {
        executable: sandbox_executable,
        ..
    } = &backend.identity
    else {
        return Err(io::Error::other(
            "macOS resolver selected a non-Seatbelt backend",
        ));
    };
    let policy = SeatbeltPolicy::construct(
        executable,
        additional_executables,
        phase,
        network_transport,
        endpoint_route,
        roots,
    )?;
    let policy_sha256 = policy.sha256();

    let mut command = Command::new(sandbox_executable);
    for definition in policy.definitions() {
        command.arg("-D").arg(definition);
    }
    command.arg("-p").arg(policy.encoded()).arg(executable);
    Ok((command, Some(policy_sha256)))
}
