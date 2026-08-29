use crate::resolution::acquisition::SourceResolveError;
use crate::resolution::acquisition::git::execution::format_sha256;
use crate::resolution::acquisition::git::objects::{git_object_algorithm, git_object_invalid};
use crate::resolution::acquisition::limits::{
    GIT_CACHE_POLICY, GIT_FIXED_COMMAND_ALLOWANCE, GIT_RESOLUTION_OBSERVATION_DOMAIN,
    GIT_RESOLUTION_OBSERVATION_SCHEMA_VERSION, GIT_SNAPSHOT_POLICY, LocalSourceLimits,
};
use crate::resolution::identity::GitObjectIdAlgorithm;
use omega_resolver_execution::ResolverExecutionPhase;
use sha2::{Digest, Sha256};
use std::path::Path;

use super::accounting::{
    git_captured_output_observation, git_network_transfer_observation,
    git_resolution_captured_output_ceiling, git_resolution_network_transfer_ceiling,
};
use super::{GitTransportExecutableIdentity, PendingResolvedGitSource};

/// Compact canonical identity of one locally successful Git resolution.
///
/// The observation is issued by the resolver and has no public constructor or
/// decoder. It binds the complete successful result and all retained native
/// execution provenance, but it does not claim strict isolation or admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSourceResolutionObservation {
    pub(in crate::resolution::acquisition) schema_version: u32,
    pub(in crate::resolution::acquisition) identity: String,
    pub(in crate::resolution::acquisition) command_count: usize,
    pub(in crate::resolution::acquisition) captured_output_ceiling: u64,
    pub(in crate::resolution::acquisition) captured_output_observed: u64,
    pub(in crate::resolution::acquisition) network_transfer_ceiling: u64,
    pub(in crate::resolution::acquisition) network_transfer_uploaded: u64,
    pub(in crate::resolution::acquisition) network_transfer_downloaded: u64,
}

impl GitSourceResolutionObservation {
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }

    pub const fn command_count(&self) -> usize {
        self.command_count
    }

    pub const fn captured_output_ceiling(&self) -> u64 {
        self.captured_output_ceiling
    }

    pub const fn captured_output_observed(&self) -> u64 {
        self.captured_output_observed
    }

    pub const fn network_transfer_ceiling(&self) -> u64 {
        self.network_transfer_ceiling
    }

    pub const fn network_transfer_uploaded(&self) -> u64 {
        self.network_transfer_uploaded
    }

    pub const fn network_transfer_downloaded(&self) -> u64 {
        self.network_transfer_downloaded
    }
}

pub(in crate::resolution::acquisition) fn issue_git_source_resolution_observation(
    resolved: &PendingResolvedGitSource,
    limits: LocalSourceLimits,
) -> Result<GitSourceResolutionObservation, SourceResolveError> {
    if resolved.execution_policy_observations.len() != resolved.command_execution_observations.len()
        || resolved.command_execution_observations.len() > GIT_FIXED_COMMAND_ALLOWANCE
    {
        return Err(SourceResolveError::GitExecutionBoundaryInvalid {
            message: "final Git resolution provenance has inconsistent command custody".to_owned(),
        });
    }
    for (policy, command) in resolved
        .execution_policy_observations
        .iter()
        .zip(&resolved.command_execution_observations)
    {
        if command.phase != policy.phase()
            || command.policy_identity != format_sha256(&Sha256::digest(policy.canonical_bytes()))
            || match (policy.endpoint_route(), &command.endpoint_observation) {
                (Some(route), Some(endpoint)) => endpoint.route() != route,
                (None, None) => false,
                _ => true,
            }
        {
            return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                message: "final Git execution rows are not joined to their native policies"
                    .to_owned(),
            });
        }
    }
    let expected_captured_output = git_captured_output_observation(
        &resolved.command_execution_observations,
        git_resolution_captured_output_ceiling(limits),
    )?;
    if resolved.captured_output_observation != expected_captured_output {
        return Err(SourceResolveError::GitExecutionBoundaryInvalid {
            message: "final Git resolution captured-output accounting is inconsistent".to_owned(),
        });
    }
    let expected_network_transfer = git_network_transfer_observation(
        &resolved.execution_policy_observations,
        &resolved.command_execution_observations,
        git_resolution_network_transfer_ceiling(limits),
    )?;
    if resolved.network_transfer_observation != expected_network_transfer {
        return Err(SourceResolveError::GitExecutionBoundaryInvalid {
            message: "final Git resolution network-transfer accounting is inconsistent".to_owned(),
        });
    }
    let object_algorithm = git_object_algorithm(&resolved.commit)?;
    if git_object_algorithm(&resolved.tree)? != object_algorithm {
        return Err(git_object_invalid(
            &resolved.tree,
            "final commit and tree use different object algorithms",
        ));
    }

    let mut hasher = Sha256::new();
    hash_resolution_field(&mut hasher, GIT_RESOLUTION_OBSERVATION_DOMAIN);
    hash_resolution_u64(
        &mut hasher,
        u64::from(GIT_RESOLUTION_OBSERVATION_SCHEMA_VERSION),
    );
    hash_resolution_field(&mut hasher, GIT_CACHE_POLICY);
    hash_resolution_field(&mut hasher, GIT_SNAPSHOT_POLICY);
    hash_resolution_field(&mut hasher, resolved.requested_locator.as_bytes());
    hash_resolution_field(&mut hasher, resolved.locator_identity.as_bytes());
    hash_resolution_field(&mut hasher, resolved.transport_profile.as_str().as_bytes());
    hash_resolution_field(&mut hasher, resolved.requested_rev.as_bytes());
    hash_resolution_field(
        &mut hasher,
        match object_algorithm {
            GitObjectIdAlgorithm::Sha1 => b"sha1",
            GitObjectIdAlgorithm::Sha256 => b"sha256",
        },
    );
    hash_resolution_field(&mut hasher, resolved.commit.as_bytes());
    hash_resolution_field(&mut hasher, resolved.tree.as_bytes());
    hash_resolution_path(&mut hasher, &resolved.snapshot_root);
    hash_resolution_path(&mut hasher, &resolved.local.root);
    hash_resolution_usize(&mut hasher, resolved.local.file_count);
    hash_resolution_u64(&mut hasher, resolved.local.byte_count);
    hash_resolution_field(&mut hasher, resolved.local.content_identity.as_bytes());
    hash_resolution_usize(&mut hasher, limits.max_files);
    hash_resolution_u64(&mut hasher, limits.max_bytes);
    hash_resolution_usize(&mut hasher, limits.max_depth);

    hash_resolution_path(&mut hasher, &resolved.git_executable.path);
    hash_resolution_field(
        &mut hasher,
        resolved.git_executable.content_identity.as_bytes(),
    );
    match &resolved.transport_executable {
        Some(executable) => {
            hash_resolution_field(&mut hasher, b"transport-present");
            hash_resolution_transport_executable(&mut hasher, executable);
        }
        None => hash_resolution_field(&mut hasher, b"transport-absent"),
    }
    hash_resolution_usize(&mut hasher, resolved.execution_helper_executables.len());
    for executable in &resolved.execution_helper_executables {
        hash_resolution_transport_executable(&mut hasher, executable);
    }

    hash_resolution_usize(&mut hasher, resolved.execution_policy_observations.len());
    for observation in &resolved.execution_policy_observations {
        hash_resolution_field(&mut hasher, &observation.canonical_bytes());
    }
    hash_resolution_usize(&mut hasher, resolved.command_execution_observations.len());
    for observation in &resolved.command_execution_observations {
        hash_resolution_field(
            &mut hasher,
            match observation.phase {
                ResolverExecutionPhase::TransportDiscovery => b"transport-discovery",
                ResolverExecutionPhase::RepositoryInitialization => b"repository-initialization",
                ResolverExecutionPhase::Fetch => b"fetch",
                ResolverExecutionPhase::RepositoryInspection => b"repository-inspection",
            },
        );
        hash_resolution_field(&mut hasher, observation.policy_identity.as_bytes());
        hash_resolution_field(&mut hasher, observation.command_identity.as_bytes());
        hash_resolution_optional_i32(&mut hasher, observation.status_code);
        hash_resolution_optional_i32(&mut hasher, observation.termination_signal);
        hash_resolution_u64(&mut hasher, observation.stdout_length);
        hash_resolution_field(&mut hasher, observation.stdout_identity.as_bytes());
        hash_resolution_u64(&mut hasher, observation.stderr_length);
        hash_resolution_field(&mut hasher, observation.stderr_identity.as_bytes());
        match &observation.endpoint_observation {
            Some(endpoint) => {
                hash_resolution_field(&mut hasher, b"endpoint-present");
                hash_resolution_field(&mut hasher, &endpoint.canonical_bytes());
            }
            None => hash_resolution_field(&mut hasher, b"endpoint-absent"),
        }
    }
    hash_resolution_u64(&mut hasher, resolved.captured_output_observation.ceiling);
    hash_resolution_u64(&mut hasher, resolved.captured_output_observation.observed);
    hash_resolution_u64(&mut hasher, resolved.network_transfer_observation.ceiling);
    hash_resolution_u64(&mut hasher, resolved.network_transfer_observation.uploaded);
    hash_resolution_u64(
        &mut hasher,
        resolved.network_transfer_observation.downloaded,
    );
    hash_resolution_field(&mut hasher, b"resolved-non-admitting");

    Ok(GitSourceResolutionObservation {
        schema_version: GIT_RESOLUTION_OBSERVATION_SCHEMA_VERSION,
        identity: format_sha256(&hasher.finalize()),
        command_count: resolved.command_execution_observations.len(),
        captured_output_ceiling: resolved.captured_output_observation.ceiling,
        captured_output_observed: resolved.captured_output_observation.observed,
        network_transfer_ceiling: resolved.network_transfer_observation.ceiling,
        network_transfer_uploaded: resolved.network_transfer_observation.uploaded,
        network_transfer_downloaded: resolved.network_transfer_observation.downloaded,
    })
}

fn hash_resolution_transport_executable(
    hasher: &mut Sha256,
    executable: &GitTransportExecutableIdentity,
) {
    hash_resolution_path(hasher, &executable.invocation_path);
    hash_resolution_path(hasher, &executable.path);
    hash_resolution_field(hasher, executable.content_identity.as_bytes());
}

fn hash_resolution_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update(
        u64::try_from(value.len())
            .expect("bounded resolution fields fit canonical u64")
            .to_le_bytes(),
    );
    hasher.update(value);
}

fn hash_resolution_u64(hasher: &mut Sha256, value: u64) {
    hash_resolution_field(hasher, &value.to_le_bytes());
}

fn hash_resolution_usize(hasher: &mut Sha256, value: usize) {
    hash_resolution_u64(
        hasher,
        u64::try_from(value).expect("compiler-owned source ceilings fit canonical u64"),
    );
}

fn hash_resolution_optional_i32(hasher: &mut Sha256, value: Option<i32>) {
    match value {
        Some(value) => {
            hash_resolution_field(hasher, b"some");
            hash_resolution_field(hasher, &value.to_le_bytes());
        }
        None => hash_resolution_field(hasher, b"none"),
    }
}

fn hash_resolution_path(hasher: &mut Sha256, path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        hash_resolution_field(hasher, b"unix-path");
        hash_resolution_field(hasher, path.as_os_str().as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        hash_resolution_field(hasher, b"windows-path");
        let units = path.as_os_str().encode_wide().collect::<Vec<_>>();
        hash_resolution_usize(hasher, units.len());
        for unit in units {
            hash_resolution_field(hasher, &unit.to_le_bytes());
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        hash_resolution_field(hasher, b"platform-path");
        hash_resolution_field(hasher, path.as_os_str().as_encoded_bytes());
    }
}
