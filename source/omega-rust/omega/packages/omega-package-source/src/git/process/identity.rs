//! Stable identities for sealed Git commands and committed standard input.

use crate::SourceResolveError;
use crate::local::capture::hash_bytes;
use crate::observations::execution::GitCommandInputCommitment;
use omega_resolver_execution::{ResolverExecutionPhase, ResolverPreparedExecution};
use sha2::{Digest, Sha256};

pub(crate) fn git_exact_input_identity(bytes: &[u8]) -> GitCommandInputCommitment {
    GitCommandInputCommitment::ExactBytes {
        length: u64::try_from(bytes.len()).expect("bounded Git command input fits canonical u64"),
        identity: format_sha256(&Sha256::digest(bytes)),
    }
}

pub(crate) fn git_command_configuration_identity(
    command: &mut ResolverPreparedExecution,
    phase: ResolverExecutionPhase,
    input: &GitCommandInputCommitment,
) -> Result<String, SourceResolveError> {
    match input {
        GitCommandInputCommitment::Null => command.stdin_null(),
        GitCommandInputCommitment::ExactBytes { .. } => command.stdin_piped(),
    };
    command.stdout_piped().stderr_piped();

    let resolver_identity = command.command_identity().map_err(|error| {
        SourceResolveError::GitExecutionBoundaryInvalid {
            message: format!("cannot identify the prepared Git command: {error}"),
        }
    })?;
    Ok(git_command_configuration_identity_from_resolver(
        resolver_identity,
        phase,
        input,
    ))
}

pub(crate) fn git_command_configuration_identity_from_resolver(
    resolver_identity: omega_resolver_execution::ResolverExecutionCommandIdentity,
    phase: ResolverExecutionPhase,
    input: &GitCommandInputCommitment,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"omega-git-command-configuration-v2\0");
    hasher.update([match phase {
        ResolverExecutionPhase::TransportDiscovery => 1,
        ResolverExecutionPhase::RepositoryInitialization => 2,
        ResolverExecutionPhase::Fetch => 3,
        ResolverExecutionPhase::RepositoryInspection => 4,
    }]);
    hash_bytes(&mut hasher, resolver_identity.as_bytes());
    match input {
        GitCommandInputCommitment::Null => hasher.update([1]),
        GitCommandInputCommitment::ExactBytes { length, identity } => {
            hasher.update([2]);
            hasher.update(length.to_le_bytes());
            hash_bytes(&mut hasher, identity.as_bytes());
        }
    }
    format_sha256(&hasher.finalize())
}

pub(crate) fn format_sha256(bytes: &[u8]) -> String {
    format_hex(bytes)
}

pub(crate) fn format_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
