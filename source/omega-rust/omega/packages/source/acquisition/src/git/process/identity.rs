//! Stable identities for sealed Git commands and their exact standard input.

use crate::SourceResolveError;
use crate::git::objects::{GitTreeEntry, GitTreeEntryKind};
use crate::local::capture::hash_bytes;
use omega_resolver_execution::{ResolverExecutionPhase, ResolverPreparedExecution};
use sha2::{Digest, Sha256};

pub(crate) enum GitCommandStdinIdentity {
    Null,
    ExactBytes { length: u64, identity: String },
}

pub(crate) fn git_batch_stdin_identity(entries: &[GitTreeEntry]) -> GitCommandStdinIdentity {
    let mut hasher = Sha256::new();
    let mut length = 0_u64;
    for entry in entries
        .iter()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        hasher.update(entry.oid.as_bytes());
        hasher.update(b"\n");
        length = length
            .saturating_add(entry.oid.len() as u64)
            .saturating_add(1);
    }
    GitCommandStdinIdentity::ExactBytes {
        length,
        identity: format_sha256(&hasher.finalize()),
    }
}

pub(crate) fn git_command_configuration_identity(
    command: &mut ResolverPreparedExecution,
    phase: ResolverExecutionPhase,
    stdin: &GitCommandStdinIdentity,
) -> Result<String, SourceResolveError> {
    match stdin {
        GitCommandStdinIdentity::Null => command.stdin_null(),
        GitCommandStdinIdentity::ExactBytes { .. } => command.stdin_piped(),
    };
    command.stdout_piped().stderr_piped();

    let mut hasher = Sha256::new();
    hasher.update(b"omega-git-command-configuration-v2\0");
    hasher.update([match phase {
        ResolverExecutionPhase::TransportDiscovery => 1,
        ResolverExecutionPhase::RepositoryInitialization => 2,
        ResolverExecutionPhase::Fetch => 3,
        ResolverExecutionPhase::RepositoryInspection => 4,
    }]);
    let resolver_identity = command.command_identity().map_err(|error| {
        SourceResolveError::GitExecutionBoundaryInvalid {
            message: format!("cannot identify the prepared Git command: {error}"),
        }
    })?;
    hash_bytes(&mut hasher, resolver_identity.as_bytes());
    match stdin {
        GitCommandStdinIdentity::Null => hasher.update([1]),
        GitCommandStdinIdentity::ExactBytes { length, identity } => {
            hasher.update([2]);
            hasher.update(length.to_le_bytes());
            hash_bytes(&mut hasher, identity.as_bytes());
        }
    }
    Ok(format_sha256(&hasher.finalize()))
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
