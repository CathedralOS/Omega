use crate::SourceResolveError;
use crate::git::commands::identity::git_command_configuration_identity_from_resolver;
use crate::git::objects::identity::{git_object_algorithm, git_object_invalid};
use crate::identity::GitObjectIdAlgorithm;
use crate::identity::digest::format_sha256;
use crate::limits::{
    GIT_CACHE_POLICY, GIT_FIXED_COMMAND_ALLOWANCE, GIT_SNAPSHOT_POLICY, GIT_SOURCE_RECEIPT_DOMAIN,
    GIT_SOURCE_RECEIPT_SCHEMA_VERSION, LocalSourceLimits,
};
use omega_resolver_execution::ResolverExecutionPhase;
use sha2::{Digest, Sha256};
use std::path::Path;

use super::accounting::{git_captured_output_observation, git_resolution_captured_output_ceiling};
use super::resolved::PendingResolvedGitSource;
use super::storage::{GitRetainedStorageObservation, validate_git_retained_storage_observation};

/// Compact canonical receipt of one locally successful Git resolution.
///
/// The observation is issued by the resolver and has no public constructor or
/// decoder. It binds the complete successful result and all retained execution
/// provenance. Platform hardening rows report what was enforced; they do not
/// claim that the host's ordinary user authority was excluded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSourceReceipt {
    pub(crate) schema_version: u32,
    pub(crate) identity: String,
    pub(crate) command_count: usize,
    pub(crate) captured_output_ceiling: u64,
    pub(crate) captured_output_observed: u64,
}

impl GitSourceReceipt {
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
}

pub(crate) fn issue_git_source_receipt(
    resolved: &PendingResolvedGitSource,
    limits: LocalSourceLimits,
    retained_storage: &GitRetainedStorageObservation,
) -> Result<GitSourceReceipt, SourceResolveError> {
    if !validate_git_retained_storage_observation(retained_storage, &retained_storage.root, limits)
    {
        return Err(SourceResolveError::GitExecutionBoundaryInvalid {
            message: "final Git retained-storage evidence is inconsistent".to_owned(),
        });
    }
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
            || command.completion.policy() != policy
            || command.policy_identity != format_sha256(&Sha256::digest(policy.canonical_bytes()))
            || command.command_identity
                != git_command_configuration_identity_from_resolver(
                    command.completion.command(),
                    command.phase,
                    &command.input,
                )
            || command.status_code != command.completion.status().code()
            || command.termination_signal != command.completion.status().unix_signal()
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
    let object_algorithm = git_object_algorithm(&resolved.commit)?;
    if git_object_algorithm(&resolved.tree)? != object_algorithm {
        return Err(git_object_invalid(
            &resolved.tree,
            "final commit and tree use different object algorithms",
        ));
    }
    if git_object_algorithm(&resolved.materialized_tree)? != object_algorithm {
        return Err(git_object_invalid(
            &resolved.materialized_tree,
            "materialized tree and repository root use different object algorithms",
        ));
    }

    let mut hasher = Sha256::new();
    hash_resolution_field(&mut hasher, GIT_SOURCE_RECEIPT_DOMAIN);
    hash_resolution_u64(&mut hasher, u64::from(GIT_SOURCE_RECEIPT_SCHEMA_VERSION));
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
    hash_resolution_field(&mut hasher, resolved.materialized_tree.as_bytes());
    match &resolved.workspace_projection {
        Some(projection) => {
            hash_resolution_field(&mut hasher, b"workspace-member");
            hash_resolution_field(
                &mut hasher,
                projection.selected_member_path().as_str().as_bytes(),
            );
            hash_resolution_field(&mut hasher, projection.selected_member_tree().as_bytes());
            hash_workspace_declaration(&mut hasher, projection.root_declaration());
            hash_resolution_usize(&mut hasher, projection.member_declarations().len());
            for declaration in projection.member_declarations() {
                hash_workspace_declaration(&mut hasher, declaration);
            }
        }
        None => hash_resolution_field(&mut hasher, b"repository-root"),
    }
    hash_resolution_path(&mut hasher, &resolved.snapshot_root);
    hash_resolution_path(&mut hasher, &resolved.local.root);
    hash_resolution_usize(&mut hasher, resolved.local.file_count);
    hash_resolution_u64(&mut hasher, resolved.local.byte_count);
    hash_resolution_field(&mut hasher, resolved.local.content_identity.as_bytes());
    hash_resolution_usize(&mut hasher, limits.max_entries);
    hash_resolution_u64(&mut hasher, limits.max_bytes);
    hash_resolution_usize(&mut hasher, limits.max_depth);

    hash_resolution_path(&mut hasher, &resolved.git_executable.path);
    hash_resolution_field(
        &mut hasher,
        resolved.git_executable.content_identity.as_bytes(),
    );
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
        hash_resolution_field(&mut hasher, &observation.completion.canonical_bytes());
        hash_resolution_optional_i32(&mut hasher, observation.status_code);
        hash_resolution_optional_i32(&mut hasher, observation.termination_signal);
        hash_resolution_u64(&mut hasher, observation.stdout_length);
        hash_resolution_field(&mut hasher, observation.stdout_identity.as_bytes());
        hash_resolution_u64(&mut hasher, observation.stderr_length);
        hash_resolution_field(&mut hasher, observation.stderr_identity.as_bytes());
    }
    hash_resolution_u64(&mut hasher, resolved.captured_output_observation.ceiling);
    hash_resolution_u64(&mut hasher, resolved.captured_output_observation.observed);
    hash_resolution_u64(&mut hasher, u64::from(retained_storage.schema_version));
    hash_resolution_field(&mut hasher, retained_storage.identity.as_bytes());
    hash_resolution_usize(&mut hasher, retained_storage.entry_ceiling);
    hash_resolution_u64(&mut hasher, retained_storage.byte_ceiling);
    hash_resolution_usize(&mut hasher, retained_storage.depth_ceiling);
    hash_resolution_usize(&mut hasher, retained_storage.entry_count);
    hash_resolution_u64(&mut hasher, retained_storage.logical_bytes);
    hash_resolution_usize(&mut hasher, retained_storage.maximum_depth);
    hash_resolution_field(&mut hasher, b"resolved");

    Ok(GitSourceReceipt {
        schema_version: GIT_SOURCE_RECEIPT_SCHEMA_VERSION,
        identity: format_sha256(&hasher.finalize()),
        command_count: resolved.command_execution_observations.len(),
        captured_output_ceiling: resolved.captured_output_observation.ceiling,
        captured_output_observed: resolved.captured_output_observation.observed,
    })
}

fn hash_workspace_declaration(
    hasher: &mut Sha256,
    declaration: &crate::git::workspace::GitWorkspaceDeclaration,
) {
    match declaration.member_path() {
        Some(member_path) => {
            hash_resolution_field(hasher, b"member");
            hash_resolution_field(hasher, member_path.as_str().as_bytes());
        }
        None => hash_resolution_field(hasher, b"root"),
    }
    hash_resolution_field(hasher, declaration.repository_path().as_bytes());
    hash_resolution_field(hasher, declaration.object_id().as_bytes());
    hash_resolution_field(hasher, declaration.bytes());
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

#[cfg(test)]
mod tests {
    use super::GitSourceReceipt;
    use crate::limits::GIT_SOURCE_RECEIPT_SCHEMA_VERSION;

    #[test]
    fn universal_receipt_has_no_broker_transfer_placeholders() {
        let receipt = GitSourceReceipt {
            schema_version: GIT_SOURCE_RECEIPT_SCHEMA_VERSION,
            identity: "receipt-identity".to_owned(),
            command_count: 4,
            captured_output_ceiling: 8_192,
            captured_output_observed: 128,
        };

        let debug = format!("{receipt:?}");
        assert!(!debug.contains("network_transfer"));
        assert!(!debug.contains("uploaded"));
        assert!(!debug.contains("downloaded"));
        assert!(!debug.contains("endpoint"));
        assert!(!debug.contains("peer"));
    }
}
