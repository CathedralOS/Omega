//! Resolver-issued source and execution observations exposed to callers.

use super::SourceResolveError;
use super::git::execution::format_sha256;
use super::git::objects::{git_object_algorithm, git_object_invalid};
use super::git::request::GitTransportProfile;
use super::limits::{
    GIT_CACHE_POLICY, GIT_CAPTURED_OUTPUT_ABSOLUTE_LIMIT, GIT_CAPTURED_OUTPUT_FIXED_ALLOWANCE,
    GIT_FIXED_COMMAND_ALLOWANCE, GIT_NETWORK_TRANSFER_ABSOLUTE_LIMIT,
    GIT_NETWORK_TRANSFER_FIXED_ALLOWANCE, GIT_RESOLUTION_OBSERVATION_DOMAIN,
    GIT_RESOLUTION_OBSERVATION_SCHEMA_VERSION, GIT_SNAPSHOT_POLICY, LocalSourceLimits,
};
use super::local::ResolvedLocalSource;
use crate::resolution::identity::GitObjectIdAlgorithm;
use omega_resolver_execution::{
    ResolverExecutionEndpointObservation, ResolverExecutionEndpointOutcome, ResolverExecutionPhase,
    ResolverExecutionPolicyObservation,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedGitSource {
    pub(in crate::resolution::source) requested_locator: String,
    pub(in crate::resolution::source) locator_identity: String,
    pub(in crate::resolution::source) transport_profile: GitTransportProfile,
    pub(in crate::resolution::source) requested_rev: String,
    pub(in crate::resolution::source) commit: String,
    pub(in crate::resolution::source) tree: String,
    pub(in crate::resolution::source) snapshot_root: PathBuf,
    pub(in crate::resolution::source) local: ResolvedLocalSource,
    /// Absolute parent Git executable identity observed before and after every launch.
    /// This is diagnostic custody, not certification of the executable.
    pub(in crate::resolution::source) git_executable: GitExecutableIdentity,
    /// Exact transport executable observed for HTTPS or SSH resolution.
    /// The test-only file adapter retains no transport executable here.
    pub(in crate::resolution::source) transport_executable: Option<GitTransportExecutableIdentity>,
    /// Fixed platform executables admitted in addition to Git and the selected
    /// transport helper. Each identity binds its invocation path, canonical
    /// target, and exact content digest.
    pub(in crate::resolution::source) execution_helper_executables:
        Vec<GitTransportExecutableIdentity>,
    /// Locally reconstructed native policy observations for every command
    /// configured during this resolution. These rows are provenance, not accepted source
    /// authority; strict admission must reject any unavailable required row.
    pub(in crate::resolution::source) execution_policy_observations:
        Vec<ResolverExecutionPolicyObservation>,
    pub(in crate::resolution::source) command_execution_observations:
        Vec<GitCommandExecutionObservation>,
    pub(in crate::resolution::source) captured_output_observation: GitCapturedOutputObservation,
    pub(in crate::resolution::source) network_transfer_observation: GitNetworkTransferObservation,
    pub(in crate::resolution::source) resolution_observation: GitSourceResolutionObservation,
}

impl ResolvedGitSource {
    pub fn requested_locator(&self) -> &str {
        &self.requested_locator
    }

    pub fn locator_identity(&self) -> &str {
        &self.locator_identity
    }

    pub const fn transport_profile(&self) -> GitTransportProfile {
        self.transport_profile
    }

    pub fn requested_revision(&self) -> &str {
        &self.requested_rev
    }

    pub fn commit(&self) -> &str {
        &self.commit
    }

    pub fn tree(&self) -> &str {
        &self.tree
    }

    pub fn snapshot_root(&self) -> &Path {
        &self.snapshot_root
    }

    pub const fn local(&self) -> &ResolvedLocalSource {
        &self.local
    }

    pub const fn git_executable(&self) -> &GitExecutableIdentity {
        &self.git_executable
    }

    pub const fn transport_executable(&self) -> Option<&GitTransportExecutableIdentity> {
        self.transport_executable.as_ref()
    }

    pub fn execution_policy_observations(&self) -> &[ResolverExecutionPolicyObservation] {
        &self.execution_policy_observations
    }

    pub fn execution_helper_executables(&self) -> &[GitTransportExecutableIdentity] {
        &self.execution_helper_executables
    }

    pub fn command_execution_observations(&self) -> &[GitCommandExecutionObservation] {
        &self.command_execution_observations
    }

    pub const fn captured_output_observation(&self) -> &GitCapturedOutputObservation {
        &self.captured_output_observation
    }

    pub const fn network_transfer_observation(&self) -> &GitNetworkTransferObservation {
        &self.network_transfer_observation
    }

    /// Canonical final-result provenance issued only after source, cache,
    /// executable, policy, and command reconciliation all succeed.
    ///
    /// This is not a strict source receipt: it preserves unavailable native
    /// guarantees rather than converting them into accepted authority.
    pub fn resolution_observation(&self) -> &GitSourceResolutionObservation {
        &self.resolution_observation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::resolution::source) struct PendingResolvedGitSource {
    pub(in crate::resolution::source) requested_locator: String,
    pub(in crate::resolution::source) locator_identity: String,
    pub(in crate::resolution::source) transport_profile: GitTransportProfile,
    pub(in crate::resolution::source) requested_rev: String,
    pub(in crate::resolution::source) commit: String,
    pub(in crate::resolution::source) tree: String,
    pub(in crate::resolution::source) snapshot_root: PathBuf,
    pub(in crate::resolution::source) local: ResolvedLocalSource,
    pub(in crate::resolution::source) git_executable: GitExecutableIdentity,
    pub(in crate::resolution::source) transport_executable: Option<GitTransportExecutableIdentity>,
    pub(in crate::resolution::source) execution_helper_executables:
        Vec<GitTransportExecutableIdentity>,
    pub(in crate::resolution::source) execution_policy_observations:
        Vec<ResolverExecutionPolicyObservation>,
    pub(in crate::resolution::source) command_execution_observations:
        Vec<GitCommandExecutionObservation>,
    pub(in crate::resolution::source) captured_output_observation: GitCapturedOutputObservation,
    pub(in crate::resolution::source) network_transfer_observation: GitNetworkTransferObservation,
}

#[cfg(test)]
impl PendingResolvedGitSource {
    pub(in crate::resolution::source) fn from_issued(resolved: &ResolvedGitSource) -> Self {
        Self {
            requested_locator: resolved.requested_locator.clone(),
            locator_identity: resolved.locator_identity.clone(),
            transport_profile: resolved.transport_profile,
            requested_rev: resolved.requested_rev.clone(),
            commit: resolved.commit.clone(),
            tree: resolved.tree.clone(),
            snapshot_root: resolved.snapshot_root.clone(),
            local: resolved.local.clone(),
            git_executable: resolved.git_executable.clone(),
            transport_executable: resolved.transport_executable.clone(),
            execution_helper_executables: resolved.execution_helper_executables.clone(),
            execution_policy_observations: resolved.execution_policy_observations.clone(),
            command_execution_observations: resolved.command_execution_observations.clone(),
            captured_output_observation: resolved.captured_output_observation.clone(),
            network_transfer_observation: resolved.network_transfer_observation.clone(),
        }
    }
}

/// Compiler-owned cumulative stdout/stderr accounting for one Git resolution.
///
/// This observation covers only bytes captured by the parent process. It does
/// not measure network transfer, object-store allocation, or descendant
/// aggregate resources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCapturedOutputObservation {
    pub(in crate::resolution::source) ceiling: u64,
    pub(in crate::resolution::source) observed: u64,
}

impl GitCapturedOutputObservation {
    pub const fn ceiling(&self) -> u64 {
        self.ceiling
    }

    pub const fn observed(&self) -> u64 {
        self.observed
    }
}

/// Compiler-owned bidirectional accounting for bytes accepted by the endpoint
/// broker across one Git resolution. CONNECT framing and DNS traffic are not
/// included. This does not claim that every platform prevents direct helper
/// egress around the broker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitNetworkTransferObservation {
    pub(in crate::resolution::source) ceiling: u64,
    pub(in crate::resolution::source) uploaded: u64,
    pub(in crate::resolution::source) downloaded: u64,
}

impl GitNetworkTransferObservation {
    pub const fn ceiling(&self) -> u64 {
        self.ceiling
    }

    pub const fn uploaded(&self) -> u64 {
        self.uploaded
    }

    pub const fn downloaded(&self) -> u64 {
        self.downloaded
    }

    pub const fn observed(&self) -> u64 {
        self.uploaded + self.downloaded
    }
}

/// Compact canonical identity of one locally successful Git resolution.
///
/// The observation is issued by the resolver and has no public constructor or
/// decoder. It binds the complete successful result and all retained native
/// execution provenance, but it does not claim strict isolation or admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSourceResolutionObservation {
    pub(in crate::resolution::source) schema_version: u32,
    pub(in crate::resolution::source) identity: String,
    pub(in crate::resolution::source) command_count: usize,
    pub(in crate::resolution::source) captured_output_ceiling: u64,
    pub(in crate::resolution::source) captured_output_observed: u64,
    pub(in crate::resolution::source) network_transfer_ceiling: u64,
    pub(in crate::resolution::source) network_transfer_uploaded: u64,
    pub(in crate::resolution::source) network_transfer_downloaded: u64,
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

pub(in crate::resolution::source) fn issue_git_source_resolution_observation(
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

pub(in crate::resolution::source) fn git_resolution_captured_output_ceiling(
    limits: LocalSourceLimits,
) -> u64 {
    limits
        .max_bytes
        .saturating_add(GIT_CAPTURED_OUTPUT_FIXED_ALLOWANCE)
        .min(GIT_CAPTURED_OUTPUT_ABSOLUTE_LIMIT)
}

pub(in crate::resolution::source) fn git_resolution_network_transfer_ceiling(
    limits: LocalSourceLimits,
) -> u64 {
    limits
        .max_bytes
        .saturating_add(GIT_NETWORK_TRANSFER_FIXED_ALLOWANCE)
        .min(GIT_NETWORK_TRANSFER_ABSOLUTE_LIMIT)
}

pub(in crate::resolution::source) fn git_network_transfer_observation(
    policies: &[ResolverExecutionPolicyObservation],
    commands: &[GitCommandExecutionObservation],
    ceiling: u64,
) -> Result<GitNetworkTransferObservation, SourceResolveError> {
    for policy in policies {
        if let Some(route) = policy.endpoint_route()
            && route.transfer_byte_ceiling() != ceiling
        {
            return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                message: "Git endpoint route carries a different transfer ceiling".to_owned(),
            });
        }
    }
    let mut uploaded = 0_u64;
    let mut downloaded = 0_u64;
    for endpoint in commands
        .iter()
        .filter_map(|command| command.endpoint_observation.as_ref())
    {
        for event in endpoint.events() {
            if event.outcome() == ResolverExecutionEndpointOutcome::TransferCeilingReached {
                return Err(SourceResolveError::GitResolutionNetworkTransferCeiling { ceiling });
            }
            uploaded = uploaded
                .checked_add(event.uploaded_bytes())
                .ok_or_else(|| SourceResolveError::GitExecutionBoundaryInvalid {
                    message: "Git upload accounting overflowed".to_owned(),
                })?;
            downloaded = downloaded
                .checked_add(event.downloaded_bytes())
                .ok_or_else(|| SourceResolveError::GitExecutionBoundaryInvalid {
                    message: "Git download accounting overflowed".to_owned(),
                })?;
        }
    }
    let observed = uploaded.checked_add(downloaded).ok_or_else(|| {
        SourceResolveError::GitExecutionBoundaryInvalid {
            message: "Git network-transfer accounting overflowed".to_owned(),
        }
    })?;
    if observed > ceiling {
        return Err(SourceResolveError::GitExecutionBoundaryInvalid {
            message: "Git network-transfer observations exceed their compiler ceiling".to_owned(),
        });
    }
    Ok(GitNetworkTransferObservation {
        ceiling,
        uploaded,
        downloaded,
    })
}

pub(in crate::resolution::source) fn git_captured_output_observation(
    commands: &[GitCommandExecutionObservation],
    ceiling: u64,
) -> Result<GitCapturedOutputObservation, SourceResolveError> {
    let observed = commands.iter().try_fold(0_u64, |total, command| {
        total
            .checked_add(command.stdout_length)
            .and_then(|total| total.checked_add(command.stderr_length))
            .ok_or_else(|| SourceResolveError::GitExecutionBoundaryInvalid {
                message: "Git captured-output accounting overflowed".to_owned(),
            })
    })?;
    if observed > ceiling {
        return Err(SourceResolveError::GitExecutionBoundaryInvalid {
            message: "Git captured-output observations exceed their compiler ceiling".to_owned(),
        });
    }
    Ok(GitCapturedOutputObservation { ceiling, observed })
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitExecutableIdentity {
    pub(in crate::resolution::source) path: PathBuf,
    pub(in crate::resolution::source) content_identity: String,
}

impl GitExecutableIdentity {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn content_identity(&self) -> &str {
        &self.content_identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitTransportExecutableIdentity {
    pub(in crate::resolution::source) invocation_path: PathBuf,
    pub(in crate::resolution::source) path: PathBuf,
    pub(in crate::resolution::source) content_identity: String,
}

impl GitTransportExecutableIdentity {
    /// Exact path through which Git selects this transport executable.
    ///
    /// HTTPS uses the install-owned `git-remote-https` entry while `path()`
    /// names its canonical executable target. SSH is invoked directly through
    /// the canonical path, so both paths are normally equal.
    pub fn invocation_path(&self) -> &Path {
        &self.invocation_path
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn content_identity(&self) -> &str {
        &self.content_identity
    }
}

/// Bounded result provenance for one successfully completed native Git
/// command. This is locally constructed observation, not an admission receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitCommandExecutionObservation {
    pub(in crate::resolution::source) phase: ResolverExecutionPhase,
    pub(in crate::resolution::source) policy_identity: String,
    pub(in crate::resolution::source) command_identity: String,
    pub(in crate::resolution::source) status_code: Option<i32>,
    pub(in crate::resolution::source) termination_signal: Option<i32>,
    pub(in crate::resolution::source) stdout_length: u64,
    pub(in crate::resolution::source) stdout_identity: String,
    pub(in crate::resolution::source) stderr_length: u64,
    pub(in crate::resolution::source) stderr_identity: String,
    pub(in crate::resolution::source) endpoint_observation:
        Option<ResolverExecutionEndpointObservation>,
}

impl GitCommandExecutionObservation {
    pub const fn phase(&self) -> ResolverExecutionPhase {
        self.phase
    }

    pub fn policy_identity(&self) -> &str {
        &self.policy_identity
    }

    pub fn command_identity(&self) -> &str {
        &self.command_identity
    }

    pub const fn status_code(&self) -> Option<i32> {
        self.status_code
    }

    pub const fn termination_signal(&self) -> Option<i32> {
        self.termination_signal
    }

    pub const fn stdout_length(&self) -> u64 {
        self.stdout_length
    }

    pub fn stdout_identity(&self) -> &str {
        &self.stdout_identity
    }

    pub const fn stderr_length(&self) -> u64 {
        self.stderr_length
    }

    pub fn stderr_identity(&self) -> &str {
        &self.stderr_identity
    }

    pub const fn endpoint_observation(&self) -> Option<&ResolverExecutionEndpointObservation> {
        self.endpoint_observation.as_ref()
    }
}
