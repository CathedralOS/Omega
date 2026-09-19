//! Canonical identity of one retained build observation.

#[cfg(test)]
use crate::{BUILD_OBSERVATION_SCHEMA_VERSION, BuildCapturedSourceInventory};
use crate::{
    BuildFilesystemGrantAccess, BuildFilesystemGrantRefusalReason,
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleKind,
    BuildFilesystemLogicalHandleOutputSource, BuildFilesystemOperationResult,
    BuildFilesystemProvider, BuildFilesystemRoot, BuildObservationSummary,
};
use sha2::{Digest, Sha256};

// This domain is an established serialized contract. The trailing `\\0` is
// intentionally the two literal bytes `\\` and `0`, not a NUL byte.
const BUILD_OBSERVATION_IDENTITY_DOMAIN: &[u8] = b"OMEGA-PACKAGE-BUILD-OBSERVATION-COMPARISON\\0";

/// Stable canonical identity of every fact retained by a build observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BuildObservationIdentity([u8; 32]);

impl BuildObservationIdentity {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl BuildObservationSummary {
    /// Computes the canonical identity of this complete retained observation.
    pub fn identity(&self) -> BuildObservationIdentity {
        let mut digest = Sha256::new();
        digest.update(BUILD_OBSERVATION_IDENTITY_DOMAIN);
        digest.update(self.schema_version().to_le_bytes());
        digest.update([u8::from(self.filesystem_host_observed())]);
        digest.update(self.filesystem_operation_schema_version().to_le_bytes());
        match self.canonical_source_metadata_identity() {
            None => digest.update([0]),
            Some(identity) => {
                digest.update([1]);
                digest.update(identity.policy_version().to_le_bytes());
                digest.update(identity.source_content_commitment());
            }
        }
        let activation = self.activation();
        match activation.root_package_identity() {
            None => digest.update([0]),
            Some(identity) => {
                digest.update([1]);
                digest.update(identity.digest());
            }
        }
        digest.update([declaration_role_tag(activation.root_role())]);
        match activation.selected_target_profile() {
            None => digest.update([0]),
            Some(profile) => {
                digest.update([1]);
                hash_bytes(&mut digest, profile.target_name().as_bytes());
            }
        }
        match activation.build_execution_profile() {
            None => digest.update([0]),
            Some(profile) => {
                digest.update([1]);
                hash_bytes(&mut digest, profile.target_name().as_bytes());
            }
        }
        match self.captured_source_inventory() {
            None => digest.update([0]),
            Some(inventory) => {
                digest.update([1]);
                digest.update(inventory.entry_count().to_le_bytes());
                digest.update(inventory.file_bytes().to_le_bytes());
                digest.update(
                    inventory
                        .source_metadata_identity()
                        .policy_version()
                        .to_le_bytes(),
                );
                digest.update(
                    inventory
                        .source_metadata_identity()
                        .source_content_commitment(),
                );
            }
        }
        digest.update(
            u64::try_from(self.included_source_handoffs().len())
                .expect("included-source handoff count fits u64")
                .to_le_bytes(),
        );
        for handoff in self.included_source_handoffs() {
            hash_bytes(&mut digest, handoff.relative_path());
            digest.update(handoff.filesystem_attempt_ordinal().to_le_bytes());
        }
        digest.update(
            u64::try_from(self.required_output_settlements().len())
                .expect("required-output settlement count fits u64")
                .to_le_bytes(),
        );
        for settlement in self.required_output_settlements() {
            hash_bytes(&mut digest, settlement.relative_path());
            digest.update(settlement.sealed_attempt_ordinal().to_le_bytes());
        }
        match self.staged_output_tree() {
            None => digest.update([0]),
            Some(tree) => {
                digest.update([1]);
                digest.update(tree.digest());
                digest.update(tree.entry_count().to_le_bytes());
                digest.update(tree.file_bytes().to_le_bytes());
            }
        }
        hash_bytes(&mut digest, self.build_log());
        digest.update(
            u64::try_from(self.filesystem_operation_attempts().len())
                .expect("build observation attempt count fits u64")
                .to_le_bytes(),
        );
        for attempt in self.filesystem_operation_attempts() {
            digest.update(attempt.operation_tag().to_le_bytes());
            digest.update([filesystem_provider_tag(attempt.provider())]);
            match attempt.result() {
                BuildFilesystemOperationResult::Scalar(value) => {
                    digest.update([0]);
                    digest.update(value.to_le_bytes());
                }
                BuildFilesystemOperationResult::LogicalHandle(identity) => {
                    digest.update([1]);
                    digest.update(identity.get().to_le_bytes());
                }
            }
            digest.update(attempt.post_error().to_le_bytes());
            digest.update(
                u64::try_from(attempt.authorized_paths().len())
                    .expect("build observation authorized-path count fits u64")
                    .to_le_bytes(),
            );
            for path in attempt.authorized_paths() {
                digest.update([path.operand_ordinal()]);
                digest.update([grant_access_tag(path.access())]);
                digest.update([filesystem_root_tag(path.root())]);
                hash_bytes(&mut digest, path.relative_path());
            }
            digest.update(
                u64::try_from(attempt.logical_handle_inputs().len())
                    .expect("build observation logical-handle input count fits u64")
                    .to_le_bytes(),
            );
            for input in attempt.logical_handle_inputs() {
                digest.update([input.operand_ordinal()]);
                digest.update([logical_handle_kind_tag(input.kind())]);
                match input.resolution() {
                    BuildFilesystemLogicalHandleInputResolution::Resolved(identity) => {
                        digest.update([0]);
                        digest.update(identity.get().to_le_bytes());
                    }
                    BuildFilesystemLogicalHandleInputResolution::Null => digest.update([1]),
                    BuildFilesystemLogicalHandleInputResolution::Unknown => digest.update([2]),
                }
            }
            match attempt.logical_handle_output() {
                None => digest.update([0]),
                Some(output) => {
                    digest.update([1]);
                    digest.update([logical_handle_kind_tag(output.kind())]);
                    digest.update(output.identity().get().to_le_bytes());
                    match output.source() {
                        BuildFilesystemLogicalHandleOutputSource::Created => digest.update([0]),
                        BuildFilesystemLogicalHandleOutputSource::Duplicated(identity) => {
                            digest.update([1]);
                            digest.update(identity.get().to_le_bytes());
                        }
                        BuildFilesystemLogicalHandleOutputSource::Borrowed(identity) => {
                            digest.update([2]);
                            digest.update(identity.get().to_le_bytes());
                        }
                    }
                }
            }
            digest.update(
                u64::try_from(attempt.retired_logical_handles().len())
                    .expect("build observation retired logical-handle count fits u64")
                    .to_le_bytes(),
            );
            for identity in attempt.retired_logical_handles() {
                digest.update(identity.get().to_le_bytes());
            }
            digest.update(
                u64::try_from(attempt.grant_refusals().len())
                    .expect("build observation refusal count fits u64")
                    .to_le_bytes(),
            );
            for refusal in attempt.grant_refusals() {
                digest.update([refusal.operand_ordinal()]);
                digest.update([grant_access_tag(refusal.access())]);
                digest.update([grant_refusal_reason_tag(refusal.reason())]);
            }
        }
        BuildObservationIdentity(digest.finalize().into())
    }
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(
        u64::try_from(bytes.len())
            .expect("build observation byte length fits u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
}

const fn filesystem_provider_tag(provider: BuildFilesystemProvider) -> u8 {
    match provider {
        BuildFilesystemProvider::Virtual => 0,
        BuildFilesystemProvider::RealUnscoped => 1,
        BuildFilesystemProvider::RealScoped => 2,
    }
}

const fn grant_access_tag(access: BuildFilesystemGrantAccess) -> u8 {
    match access {
        BuildFilesystemGrantAccess::Read => 0,
        BuildFilesystemGrantAccess::Write => 1,
    }
}

const fn filesystem_root_tag(root: BuildFilesystemRoot) -> u8 {
    match root {
        BuildFilesystemRoot::Source => 0,
        BuildFilesystemRoot::Output => 1,
    }
}

const fn logical_handle_kind_tag(kind: BuildFilesystemLogicalHandleKind) -> u8 {
    match kind {
        BuildFilesystemLogicalHandleKind::Descriptor => 0,
        BuildFilesystemLogicalHandleKind::Native => 1,
        BuildFilesystemLogicalHandleKind::Find => 2,
    }
}

const fn grant_refusal_reason_tag(reason: BuildFilesystemGrantRefusalReason) -> u8 {
    match reason {
        BuildFilesystemGrantRefusalReason::Unresolvable => 0,
        BuildFilesystemGrantRefusalReason::OutsideGrantedRoots => 1,
        BuildFilesystemGrantRefusalReason::UnrepresentableRootedPath => 2,
        BuildFilesystemGrantRefusalReason::ObservationEvidenceLimitExceeded => 3,
    }
}

const fn declaration_role_tag(role: Option<package_compilation::BuildDeclarationKind>) -> u8 {
    match role {
        None => 0,
        Some(package_compilation::BuildDeclarationKind::Package) => 1,
        Some(package_compilation::BuildDeclarationKind::Application) => 2,
        Some(package_compilation::BuildDeclarationKind::Workspace) => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BUILD_OBSERVATION_SCHEMA_VERSION, BuildCapturedSourceInventory, BuildObservationSummary,
    };
    use crate::BuildActivation;

    fn empty_summary() -> BuildObservationSummary {
        BuildObservationSummary {
            schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
            filesystem_host_observed: false,
            filesystem_operation_schema_version:
                checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
            filesystem_operation_attempts: Vec::new(),
            canonical_source_metadata_identity: None,
            activation: BuildActivation::default(),
            captured_source_inventory: None,
            included_source_handoffs: Vec::new(),
            required_output_settlements: Vec::new(),
            staged_output_tree: None,
            build_log: Vec::new(),
        }
    }

    #[test]
    fn identity_is_stable_and_exposes_the_same_digest_by_value_or_reference() {
        let summary = empty_summary();
        let identity = summary.identity();

        assert_eq!(identity, summary.clone().identity());
        assert_eq!(identity.as_bytes(), &identity.digest());
        assert_eq!(
            identity.digest(),
            [
                0x46, 0xfc, 0x54, 0x4d, 0xf3, 0xfd, 0x60, 0x8e, 0x83, 0x5e, 0x4f, 0xe3, 0xe2, 0x1f,
                0x38, 0x9d, 0x38, 0xa9, 0xdf, 0x65, 0x53, 0x40, 0x0f, 0x49, 0x58, 0xbf, 0xe6, 0x47,
                0x97, 0x4b, 0xec, 0x8b,
            ],
            "single-execution observation schema 80 with operation schema 20 has stable canonical bytes",
        );
    }

    #[test]
    fn identity_binds_top_level_observation_contract() {
        let baseline = empty_summary().identity();

        let mut changed = empty_summary();
        changed.schema_version += 1;
        assert_ne!(baseline, changed.identity());

        let mut changed = empty_summary();
        changed.filesystem_host_observed = true;
        assert_ne!(baseline, changed.identity());

        let mut changed = empty_summary();
        changed.filesystem_operation_schema_version += 1;
        assert_ne!(baseline, changed.identity());

        let mut changed = empty_summary();
        changed.captured_source_inventory = Some(BuildCapturedSourceInventory {
            entry_count: 3,
            file_bytes: 7,
            source_metadata_identity: crate::BuildCanonicalSourceMetadataIdentity::new(1, [1; 32]),
        });
        assert_ne!(baseline, changed.identity());
        let first_selection = changed.identity();
        changed
            .captured_source_inventory
            .as_mut()
            .unwrap()
            .source_metadata_identity =
            crate::BuildCanonicalSourceMetadataIdentity::new(1, [2; 32]);
        assert_ne!(first_selection, changed.identity());

        let mut changed = empty_summary();
        changed.activation = BuildActivation {
            selected_target_profile: Some(target::TargetProfile::LinuxX64),
            ..BuildActivation::default()
        };
        let selected_target_only = changed.identity();
        assert_ne!(baseline, selected_target_only);

        // The execution profile is its own activation member: binding the
        // same profile there as the selected target still changes the digest.
        let mut changed = empty_summary();
        changed.activation = BuildActivation {
            selected_target_profile: Some(target::TargetProfile::LinuxX64),
            build_execution_profile: Some(target::TargetProfile::LinuxX64),
            ..BuildActivation::default()
        };
        assert_ne!(baseline, changed.identity());
        assert_ne!(selected_target_only, changed.identity());

        let mut changed = empty_summary();
        changed.build_log = b"compiler-owned build log\n".to_vec();
        assert_ne!(baseline, changed.identity());
    }
}
