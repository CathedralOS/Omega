//! Canonical identity of one retained build observation.

use crate::{
    BuildFilesystemGrantAccess, BuildFilesystemGrantRefusalReason,
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleKind,
    BuildFilesystemLogicalHandleOutputSource, BuildFilesystemOperationResult,
    BuildFilesystemProvider, BuildFilesystemRoot, BuildObservationSummary,
};

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
        let mut digest =
            crate::digest_encoder::DomainDigestEncoder::new(BUILD_OBSERVATION_IDENTITY_DOMAIN);
        digest.u32(self.schema_version());
        digest.byte(u8::from(self.filesystem_host_observed()));
        digest.u32(self.filesystem_operation_schema_version());
        match self.canonical_source_metadata_identity() {
            None => digest.byte(0),
            Some(identity) => {
                digest.byte(1);
                digest.u32(identity.policy_version());
                digest.raw(&identity.source_content_commitment());
            }
        }
        let activation = self.activation();
        match activation.root_package_identity() {
            None => digest.byte(0),
            Some(identity) => {
                digest.byte(1);
                digest.raw(&identity.digest());
            }
        }
        digest.byte(declaration_role_tag(activation.root_role()));
        match activation.build_execution_profile() {
            None => digest.byte(0),
            Some(profile) => {
                digest.byte(1);
                digest.bytes(profile.target_name().as_bytes());
            }
        }
        match self.captured_source_inventory() {
            None => digest.byte(0),
            Some(inventory) => {
                digest.byte(1);
                digest.u64(inventory.entry_count());
                digest.u64(inventory.file_bytes());
                digest.u32(inventory.source_metadata_identity().policy_version());
                digest.raw(
                    &inventory
                        .source_metadata_identity()
                        .source_content_commitment(),
                );
            }
        }
        digest.u64(self.named_input_inventories().len() as u64);
        for input in self.named_input_inventories() {
            digest.bytes(input.name());
            digest.u32(input.root_identity());
            let inventory = input.inventory();
            digest.u64(inventory.entry_count());
            digest.u64(inventory.file_bytes());
            digest.u32(inventory.source_metadata_identity().policy_version());
            digest.raw(
                &inventory
                    .source_metadata_identity()
                    .source_content_commitment(),
            );
        }
        digest.u64(
            u64::try_from(self.included_source_handoffs().len())
                .expect("included-source handoff count fits u64"),
        );
        for handoff in self.included_source_handoffs() {
            digest.bytes(handoff.relative_path());
            digest.u64(handoff.filesystem_attempt_ordinal());
        }
        digest.u64(
            u64::try_from(self.required_output_settlements().len())
                .expect("required-output settlement count fits u64"),
        );
        for settlement in self.required_output_settlements() {
            digest.bytes(settlement.relative_path());
            digest.u64(settlement.sealed_attempt_ordinal());
        }
        match self.staged_output_tree() {
            None => digest.byte(0),
            Some(tree) => {
                digest.byte(1);
                digest.raw(&tree.digest());
                digest.u64(tree.entry_count());
                digest.u64(tree.file_bytes());
            }
        }
        digest.bytes(self.build_log());
        digest.u64(
            u64::try_from(self.filesystem_operation_attempts().len())
                .expect("build observation attempt count fits u64"),
        );
        for attempt in self.filesystem_operation_attempts() {
            digest.u16(attempt.operation_tag());
            digest.byte(filesystem_provider_tag(attempt.provider()));
            match attempt.result() {
                BuildFilesystemOperationResult::Scalar(value) => {
                    digest.byte(0);
                    digest.i64(value);
                }
                BuildFilesystemOperationResult::LogicalHandle(identity) => {
                    digest.byte(1);
                    digest.u64(identity.get());
                }
            }
            digest.raw(&attempt.post_error().to_le_bytes());
            digest.u64(
                u64::try_from(attempt.authorized_paths().len())
                    .expect("build observation authorized-path count fits u64"),
            );
            for path in attempt.authorized_paths() {
                digest.byte(path.operand_ordinal());
                digest.byte(grant_access_tag(path.access()));
                digest.byte(filesystem_root_tag(path.root()));
                if let BuildFilesystemRoot::NamedInput(identity) = path.root() {
                    digest.u32(identity);
                }
                digest.bytes(path.relative_path());
            }
            digest.u64(
                u64::try_from(attempt.logical_handle_inputs().len())
                    .expect("build observation logical-handle input count fits u64"),
            );
            for input in attempt.logical_handle_inputs() {
                digest.byte(input.operand_ordinal());
                digest.byte(logical_handle_kind_tag(input.kind()));
                match input.resolution() {
                    BuildFilesystemLogicalHandleInputResolution::Resolved(identity) => {
                        digest.byte(0);
                        digest.u64(identity.get());
                    }
                    BuildFilesystemLogicalHandleInputResolution::Null => digest.byte(1),
                    BuildFilesystemLogicalHandleInputResolution::Unknown => digest.byte(2),
                }
            }
            match attempt.logical_handle_output() {
                None => digest.byte(0),
                Some(output) => {
                    digest.byte(1);
                    digest.byte(logical_handle_kind_tag(output.kind()));
                    digest.u64(output.identity().get());
                    match output.source() {
                        BuildFilesystemLogicalHandleOutputSource::Created => digest.byte(0),
                        BuildFilesystemLogicalHandleOutputSource::Duplicated(identity) => {
                            digest.byte(1);
                            digest.u64(identity.get());
                        }
                        BuildFilesystemLogicalHandleOutputSource::Borrowed(identity) => {
                            digest.byte(2);
                            digest.u64(identity.get());
                        }
                    }
                }
            }
            digest.u64(
                u64::try_from(attempt.retired_logical_handles().len())
                    .expect("build observation retired logical-handle count fits u64"),
            );
            for identity in attempt.retired_logical_handles() {
                digest.u64(identity.get());
            }
            digest.u64(
                u64::try_from(attempt.grant_refusals().len())
                    .expect("build observation refusal count fits u64"),
            );
            for refusal in attempt.grant_refusals() {
                digest.byte(refusal.operand_ordinal());
                digest.byte(grant_access_tag(refusal.access()));
                digest.byte(grant_refusal_reason_tag(refusal.reason()));
            }
        }
        BuildObservationIdentity(digest.finish())
    }
}

const fn filesystem_provider_tag(provider: BuildFilesystemProvider) -> u8 {
    match provider {
        BuildFilesystemProvider::Virtual => 0,
        BuildFilesystemProvider::RealUnscoped => 1,
        BuildFilesystemProvider::RealScoped => 2,
        BuildFilesystemProvider::Captured => 3,
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
        BuildFilesystemRoot::NamedInput(_) => 2,
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

const fn declaration_role_tag(role: Option<crate::BuildDeclarationKind>) -> u8 {
    match role {
        None => 0,
        Some(crate::BuildDeclarationKind::Package) => 1,
        Some(crate::BuildDeclarationKind::Application) => 2,
        Some(crate::BuildDeclarationKind::Workspace) => 3,
    }
}
