//! Stable identities for sealed Git commands and their exact standard input.

use crate::resolution::source::git::objects::{GitTreeEntry, GitTreeEntryKind};
use crate::resolution::source::local::{hash_bytes, hash_length};
use omega_resolver_execution::ResolverExecutionPhase;
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::process::Command;

pub(in crate::resolution::source) enum GitCommandStdinIdentity {
    Null,
    ExactBytes { length: u64, identity: String },
}

pub(in crate::resolution::source) fn git_batch_stdin_identity(
    entries: &[GitTreeEntry],
) -> GitCommandStdinIdentity {
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

pub(in crate::resolution::source) fn git_command_configuration_identity(
    command: &Command,
    phase: ResolverExecutionPhase,
    stdin: &GitCommandStdinIdentity,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"omega-git-command-configuration-v1\0");
    hasher.update([match phase {
        ResolverExecutionPhase::TransportDiscovery => 1,
        ResolverExecutionPhase::RepositoryInitialization => 2,
        ResolverExecutionPhase::Fetch => 3,
        ResolverExecutionPhase::RepositoryInspection => 4,
    }]);
    hash_command_os_str(&mut hasher, command.get_program());
    let arguments = command.get_args().collect::<Vec<_>>();
    hash_length(&mut hasher, arguments.len() as u64);
    for argument in arguments {
        hash_command_os_str(&mut hasher, argument);
    }
    let mut environment = command.get_envs().collect::<Vec<_>>();
    environment.sort_by(|left, right| left.0.cmp(right.0));
    hash_length(&mut hasher, environment.len() as u64);
    for (name, value) in environment {
        hash_command_os_str(&mut hasher, name);
        match value {
            Some(value) => {
                hasher.update([1]);
                hash_command_os_str(&mut hasher, value);
            }
            None => hasher.update([0]),
        }
    }
    match command.get_current_dir() {
        Some(directory) => {
            hasher.update([1]);
            hash_command_os_str(&mut hasher, directory.as_os_str());
        }
        None => hasher.update([0]),
    }
    match stdin {
        GitCommandStdinIdentity::Null => hasher.update([1]),
        GitCommandStdinIdentity::ExactBytes { length, identity } => {
            hasher.update([2]);
            hasher.update(length.to_le_bytes());
            hash_bytes(&mut hasher, identity.as_bytes());
        }
    }
    format_sha256(&hasher.finalize())
}

fn hash_command_os_str(hasher: &mut Sha256, value: &OsStr) {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        hash_bytes(hasher, value.as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let units = value.encode_wide().collect::<Vec<_>>();
        hash_length(hasher, units.len() as u64);
        for unit in units {
            hasher.update(unit.to_le_bytes());
        }
    }
}

pub(super) fn format_sha256(bytes: &[u8]) -> String {
    format_hex(bytes)
}

pub(super) fn format_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
