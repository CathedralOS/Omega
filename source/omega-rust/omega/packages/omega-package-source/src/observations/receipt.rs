//! Local reconstruction of strict Git source receipts.

use crate::GitTransportProfile;
use crate::limits::{
    GIT_STRICT_RECEIPT_DOMAIN, GIT_STRICT_RECEIPT_SCHEMA_VERSION, LocalSourceLimits,
};
use omega_resolver_execution::{
    ResolverExecutionEndpointOutcome, ResolverExecutionNetworkTransport, ResolverExecutionPhase,
    ResolverStrictExecutionUnavailable,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;

use super::resolution::{GitSourceResolutionObservation, issue_git_source_resolution_observation};
use super::resolved::PendingResolvedGitSource;
use crate::git::process::identity::{
    format_sha256, git_command_configuration_identity_from_resolver,
};

/// Opaque, locally reconstructed evidence that one Git source resolution met
/// every strict native and resolver-owned requirement.
///
/// There is intentionally no public constructor or decoder. Persisted strings
/// cannot recreate this value; the resolver must reconstruct it from current
/// native policy, command, endpoint, executable, accounting, and source rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSourceStrictReceipt {
    schema_version: u32,
    identity: String,
    resolution_observation_identity: String,
}

impl GitSourceStrictReceipt {
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub fn resolution_observation_identity(&self) -> &str {
        &self.resolution_observation_identity
    }
}

/// Closed reason why current local evidence could not reconstruct a strict
/// source receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitSourceStrictReceiptError {
    /// A resolution with no complete policy/outcome pair cannot be strict.
    MissingExecutionRows,
    /// Existing rows failed their ordinary policy, completion, endpoint,
    /// executable, source-subject, or accounting reconciliation.
    InvalidResolutionObservation,
    /// Locally reconstructed rows do not reproduce the retained final
    /// resolution observation exactly.
    ResolutionObservationMismatch,
    /// The selected native backend explicitly reported a required guarantee as
    /// unavailable.
    ExecutionUnavailable(ResolverStrictExecutionUnavailable),
    /// A required non-native source-receipt row has no locally reconstructed
    /// evidence yet.
    MissingRequiredEvidence(GitSourceStrictReceiptRequirement),
}

impl GitSourceStrictReceiptError {
    pub const fn unavailable(&self) -> Option<&ResolverStrictExecutionUnavailable> {
        match self {
            Self::ExecutionUnavailable(unavailable) => Some(unavailable),
            _ => None,
        }
    }

    pub const fn missing_requirement(&self) -> Option<GitSourceStrictReceiptRequirement> {
        match self {
            Self::MissingRequiredEvidence(requirement) => Some(*requirement),
            _ => None,
        }
    }
}

/// Closed non-native rows that must exist before a strict Git source receipt
/// can be issued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitSourceStrictReceiptRequirement {
    ProductionTransport,
    TransportTrust,
    CredentialCustody,
    WholeOperationStorageCeiling,
    WholeOperationResourceCeiling,
    SameUserMutationIsolation,
}

impl fmt::Display for GitSourceStrictReceiptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingExecutionRows => {
                formatter.write_str("strict Git source receipt is missing native execution rows")
            }
            Self::InvalidResolutionObservation => formatter.write_str(
                "strict Git source receipt reconstruction found invalid resolution evidence",
            ),
            Self::ResolutionObservationMismatch => formatter.write_str(
                "strict Git source receipt reconstruction disagrees with the retained resolution observation",
            ),
            Self::ExecutionUnavailable(unavailable) => fmt::Display::fmt(unavailable, formatter),
            Self::MissingRequiredEvidence(requirement) => write!(
                formatter,
                "strict Git source receipt is missing required evidence for {requirement:?}"
            ),
        }
    }
}

impl std::error::Error for GitSourceStrictReceiptError {}

pub(crate) fn reconstruct_git_source_strict_receipt(
    resolved: &PendingResolvedGitSource,
    limits: LocalSourceLimits,
    retained: &GitSourceResolutionObservation,
) -> Result<GitSourceStrictReceipt, GitSourceStrictReceiptError> {
    if resolved.execution_policy_observations.is_empty()
        || resolved.command_execution_observations.is_empty()
        || resolved.execution_policy_observations.len()
            != resolved.command_execution_observations.len()
    {
        return Err(GitSourceStrictReceiptError::MissingExecutionRows);
    }
    validate_execution_custody(resolved)?;

    let reconstructed = issue_git_source_resolution_observation(resolved, limits)
        .map_err(|_| GitSourceStrictReceiptError::InvalidResolutionObservation)?;
    if &reconstructed != retained {
        return Err(GitSourceStrictReceiptError::ResolutionObservationMismatch);
    }

    for policy in &resolved.execution_policy_observations {
        policy
            .require_strict()
            .map_err(GitSourceStrictReceiptError::ExecutionUnavailable)?;
    }

    if let Some(requirement) = first_missing_source_requirement(resolved.transport_profile) {
        return Err(GitSourceStrictReceiptError::MissingRequiredEvidence(
            requirement,
        ));
    }

    let mut hasher = Sha256::new();
    hash_field(&mut hasher, GIT_STRICT_RECEIPT_DOMAIN);
    hash_field(
        &mut hasher,
        &GIT_STRICT_RECEIPT_SCHEMA_VERSION.to_le_bytes(),
    );
    hash_field(&mut hasher, &retained.schema_version().to_le_bytes());
    hash_field(&mut hasher, retained.identity().as_bytes());
    hash_field(&mut hasher, b"strict-native-and-source-reconstruction");

    Ok(GitSourceStrictReceipt {
        schema_version: GIT_STRICT_RECEIPT_SCHEMA_VERSION,
        identity: format_sha256(&hasher.finalize()),
        resolution_observation_identity: retained.identity().to_owned(),
    })
}

fn validate_execution_custody(
    resolved: &PendingResolvedGitSource,
) -> Result<(), GitSourceStrictReceiptError> {
    for (policy, command) in resolved
        .execution_policy_observations
        .iter()
        .zip(&resolved.command_execution_observations)
    {
        let network_phase = matches!(
            policy.phase(),
            ResolverExecutionPhase::TransportDiscovery | ResolverExecutionPhase::Fetch
        );
        let expected_transport = network_phase.then(|| match resolved.transport_profile {
            GitTransportProfile::Https => ResolverExecutionNetworkTransport::Https,
            GitTransportProfile::Ssh | GitTransportProfile::TestFile => {
                ResolverExecutionNetworkTransport::Ssh
            }
        });
        let mut expected_executables = BTreeSet::new();
        if network_phase {
            for executable in resolved
                .transport_executable
                .iter()
                .chain(resolved.execution_helper_executables.iter())
            {
                expected_executables.insert(executable.invocation_path.clone());
                expected_executables.insert(executable.path.clone());
            }
        }
        expected_executables.remove(&resolved.git_executable.path);
        let policy_executables = policy
            .additional_executables()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let endpoint_is_valid = match (policy.endpoint_route(), &command.endpoint_observation) {
            (Some(route), Some(endpoint)) if endpoint.route() == route => {
                resolved.transport_profile == GitTransportProfile::TestFile
                    || endpoint
                        .events()
                        .iter()
                        .any(|event| event.outcome() == ResolverExecutionEndpointOutcome::Connected)
            }
            (None, None) => true,
            _ => false,
        };
        if policy.executable() != resolved.git_executable.path
            || policy.network_transport() != expected_transport
            || policy_executables != expected_executables
            || command.command_identity
                != git_command_configuration_identity_from_resolver(
                    command.completion.command(),
                    command.phase,
                    &command.input,
                )
            || command.status_code != command.completion.status().code()
            || command.termination_signal != command.completion.status().unix_signal()
            || !endpoint_is_valid
        {
            return Err(GitSourceStrictReceiptError::InvalidResolutionObservation);
        }
    }
    Ok(())
}

fn first_missing_source_requirement(
    transport: GitTransportProfile,
) -> Option<GitSourceStrictReceiptRequirement> {
    match transport {
        GitTransportProfile::TestFile => {
            Some(GitSourceStrictReceiptRequirement::ProductionTransport)
        }
        GitTransportProfile::Https | GitTransportProfile::Ssh => {
            Some(GitSourceStrictReceiptRequirement::TransportTrust)
        }
    }
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update(
        u64::try_from(value.len())
            .expect("bounded strict-receipt fields fit canonical u64")
            .to_le_bytes(),
    );
    hasher.update(value);
}
