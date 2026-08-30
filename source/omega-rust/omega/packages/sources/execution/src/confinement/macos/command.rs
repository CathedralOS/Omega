use super::policy::SeatbeltPolicy;
use crate::backend::{ResolverExecutionAuthorityRoots, ResolverExecutionBackend};
use crate::model::{ResolverExecutionBackendIdentity, ResolverExecutionPhase};
use std::io;
use std::path::Path;
use std::process::Command;

pub(crate) fn command(
    backend: &ResolverExecutionBackend,
    executable: &Path,
    phase: ResolverExecutionPhase,
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
    if phase.permits_network() {
        return Ok((Command::new(executable), None));
    }

    let policy = SeatbeltPolicy::construct(executable, phase, roots)?;
    let policy_sha256 = policy.sha256();

    let mut command = Command::new(sandbox_executable);
    for definition in policy.definitions() {
        command.arg("-D").arg(definition);
    }
    command.arg("-p").arg(policy.encoded()).arg(executable);
    Ok((command, Some(policy_sha256)))
}
