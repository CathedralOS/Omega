use crate::{ResolverExecutionCommandIdentity, ResolverExecutionPolicyObservation};
use std::process::ExitStatus;

const COMPLETION_SCHEMA_VERSION: u32 = 1;

/// How the resolver-owned native process container was closed before a
/// completion observation was issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolverExecutionTerminationDisposition {
    /// The native container accepted an explicit whole-container termination.
    Requested,
    /// The native container was already absent when termination was attempted.
    AlreadyAbsent,
}

impl ResolverExecutionTerminationDisposition {
    const fn tag(self) -> u8 {
        match self {
            Self::Requested => 1,
            Self::AlreadyAbsent => 2,
        }
    }
}

/// Platform-neutral process status observed while reaping the prepared
/// resolver execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolverExecutionExitStatus {
    success: bool,
    code: Option<i32>,
    unix_signal: Option<i32>,
}

impl ResolverExecutionExitStatus {
    pub const fn success(&self) -> bool {
        self.success
    }

    pub const fn code(&self) -> Option<i32> {
        self.code
    }

    pub const fn unix_signal(&self) -> Option<i32> {
        self.unix_signal
    }

    pub(super) fn from_status(status: ExitStatus) -> Self {
        #[cfg(unix)]
        let unix_signal = {
            use std::os::unix::process::ExitStatusExt;
            status.signal()
        };
        #[cfg(not(unix))]
        let unix_signal = None;
        Self {
            success: status.success(),
            code: status.code(),
            unix_signal,
        }
    }

    fn encode(self, bytes: &mut Vec<u8>) {
        bytes.push(u8::from(self.success));
        encode_optional_i32(bytes, self.code);
        encode_optional_i32(bytes, self.unix_signal);
    }
}

/// Opaque completion issued only after a prepared command was spawned, its
/// native container was explicitly closed or confirmed absent, and the child
/// status was reaped.
///
/// This binds process lifecycle to the exact prepared command and policy. It
/// does not claim source authenticity, Git protocol correctness, package
/// admission, or that an audit occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverExecutionCompletionObservation {
    policy: ResolverExecutionPolicyObservation,
    command: ResolverExecutionCommandIdentity,
    termination: ResolverExecutionTerminationDisposition,
    status: ResolverExecutionExitStatus,
}

impl ResolverExecutionCompletionObservation {
    pub(super) const fn new(
        policy: ResolverExecutionPolicyObservation,
        command: ResolverExecutionCommandIdentity,
        termination: ResolverExecutionTerminationDisposition,
        status: ResolverExecutionExitStatus,
    ) -> Self {
        Self {
            policy,
            command,
            termination,
            status,
        }
    }

    pub const fn policy(&self) -> &ResolverExecutionPolicyObservation {
        &self.policy
    }

    pub const fn command(&self) -> ResolverExecutionCommandIdentity {
        self.command
    }

    pub const fn termination(&self) -> ResolverExecutionTerminationDisposition {
        self.termination
    }

    pub const fn status(&self) -> ResolverExecutionExitStatus {
        self.status
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        let policy = self.policy.canonical_bytes();
        let mut bytes = Vec::with_capacity(policy.len().saturating_add(64));
        bytes.extend_from_slice(b"omega-resolver-execution-completion\0");
        bytes.extend_from_slice(&COMPLETION_SCHEMA_VERSION.to_le_bytes());
        bytes.extend_from_slice(
            &u64::try_from(policy.len())
                .expect("bounded resolver policy length fits u64")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&policy);
        bytes.extend_from_slice(self.command.as_bytes());
        bytes.push(self.termination.tag());
        self.status.encode(&mut bytes);
        bytes
    }
}

fn encode_optional_i32(bytes: &mut Vec<u8>, value: Option<i32>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        None => bytes.push(0),
    }
}
