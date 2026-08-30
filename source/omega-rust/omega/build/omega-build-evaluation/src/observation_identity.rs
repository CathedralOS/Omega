//! Canonical identity of one retained build observation.

#[cfg(test)]
use crate::{BUILD_OBSERVATION_SCHEMA_VERSION, BuildFilesystemReplayVerdict};
use crate::{
    BuildFilesystemGrantAccess, BuildFilesystemGrantRefusalReason,
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleKind,
    BuildFilesystemLogicalHandleOutputSource, BuildFilesystemOperationResult,
    BuildFilesystemProvider, BuildFilesystemReplayDisposition, BuildFilesystemRoot,
    BuildFilesystemScalarOperandValue, BuildObservationClass, BuildObservationSummary,
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
        digest.update([observation_class_tag(self.ceiling())]);
        digest.update([observation_class_tag(self.realized())]);
        digest.update(self.filesystem_operation_schema_version().to_le_bytes());
        match self.canonical_source_metadata_identity() {
            None => digest.update([0]),
            Some(identity) => {
                digest.update([1]);
                digest.update(identity.policy_version().to_le_bytes());
                digest.update(identity.source_content_commitment());
            }
        }
        let replay_verdict = self.filesystem_replay_verdict();
        digest.update(replay_verdict.schema_version().to_le_bytes());
        digest.update([filesystem_replay_disposition_tag(
            replay_verdict.disposition(),
        )]);
        digest.update(
            u64::try_from(self.included_source_handoffs().len())
                .expect("included-source handoff count fits u64")
                .to_le_bytes(),
        );
        for handoff in self.included_source_handoffs() {
            hash_bytes(&mut digest, handoff.relative_path());
            digest.update(handoff.filesystem_attempt_ordinal().to_le_bytes());
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
                u64::try_from(attempt.scalar_operands().len())
                    .expect("build observation scalar-operand count fits u64")
                    .to_le_bytes(),
            );
            for operand in attempt.scalar_operands() {
                digest.update([operand.operand_ordinal()]);
                match operand.value() {
                    BuildFilesystemScalarOperandValue::I32(value) => {
                        digest.update([0]);
                        digest.update(value.to_le_bytes());
                    }
                    BuildFilesystemScalarOperandValue::U32(value) => {
                        digest.update([1]);
                        digest.update(value.to_le_bytes());
                    }
                    BuildFilesystemScalarOperandValue::I64(value) => {
                        digest.update([2]);
                        digest.update(value.to_le_bytes());
                    }
                    BuildFilesystemScalarOperandValue::U64(value) => {
                        digest.update([3]);
                        digest.update(value.to_le_bytes());
                    }
                }
            }
            digest.update(
                u64::try_from(attempt.byte_operands().len())
                    .expect("build observation byte-operand count fits u64")
                    .to_le_bytes(),
            );
            for operand in attempt.byte_operands() {
                digest.update([operand.operand_ordinal()]);
                hash_bytes(&mut digest, operand.bytes());
            }
            digest.update(
                u64::try_from(attempt.path_like_operands().len())
                    .expect("build observation path-like-operand count fits u64")
                    .to_le_bytes(),
            );
            for operand in attempt.path_like_operands() {
                digest.update([operand.operand_ordinal()]);
                hash_bytes(&mut digest, operand.bytes());
            }
            digest.update(
                u64::try_from(attempt.rooted_path_operand_resolutions().len())
                    .expect("build observation rooted-path-resolution count fits u64")
                    .to_le_bytes(),
            );
            for operand in attempt.rooted_path_operand_resolutions() {
                digest.update([operand.operand_ordinal()]);
                digest.update([match operand.root() {
                    BuildFilesystemRoot::Source => 0,
                    BuildFilesystemRoot::Output => 1,
                }]);
                hash_bytes(&mut digest, operand.relative_path());
            }
            digest.update(
                u64::try_from(attempt.returned_paths().len())
                    .expect("build observation returned-path count fits u64")
                    .to_le_bytes(),
            );
            for returned in attempt.returned_paths() {
                digest.update([returned.operand_ordinal()]);
                digest.update([match returned.kind() {
                    crate::BuildFilesystemReturnedPathKind::ReadLinkPayload => 0,
                    crate::BuildFilesystemReturnedPathKind::CanonicalPath => 1,
                    crate::BuildFilesystemReturnedPathKind::FinalPath => 2,
                }]);
                digest.update([match returned.completeness() {
                    crate::BuildFilesystemReturnedPathCompleteness::Complete => 0,
                    crate::BuildFilesystemReturnedPathCompleteness::LimitReached => 1,
                }]);
                hash_bytes(&mut digest, returned.bytes());
            }
            digest.update(
                u64::try_from(attempt.observed_byte_regions().len())
                    .expect("build observation observed-byte-region count fits u64")
                    .to_le_bytes(),
            );
            for region in attempt.observed_byte_regions() {
                digest.update([region.output_operand_ordinal()]);
                digest.update([match region.kind() {
                    crate::BuildFilesystemObservedByteRegionKind::SequentialFileRead => 0,
                    crate::BuildFilesystemObservedByteRegionKind::PositionedFileRead => 1,
                    crate::BuildFilesystemObservedByteRegionKind::DirectoryRecords => 2,
                    crate::BuildFilesystemObservedByteRegionKind::FindEntry => 3,
                }]);
                digest.update(region.offset().to_le_bytes());
                digest.update(region.length().to_le_bytes());
            }
            digest.update(
                u64::try_from(attempt.metadata_observations().len())
                    .expect("build observation metadata count fits u64")
                    .to_le_bytes(),
            );
            for metadata in attempt.metadata_observations() {
                digest.update([metadata.output_operand_ordinal()]);
                digest.update([match metadata.kind() {
                    crate::BuildFilesystemMetadataObservationKind::FollowedPath => 0,
                    crate::BuildFilesystemMetadataObservationKind::OpenDescriptor => 1,
                    crate::BuildFilesystemMetadataObservationKind::UnfollowedFinalPath => 2,
                }]);
                digest.update(metadata.device().to_le_bytes());
                digest.update(metadata.mode().to_le_bytes());
                digest.update(metadata.link_count().to_le_bytes());
                digest.update(metadata.inode().to_le_bytes());
                digest.update(metadata.user().to_le_bytes());
                digest.update(metadata.group().to_le_bytes());
                digest.update(metadata.referenced_device().to_le_bytes());
                digest.update(metadata.access_time().to_le_bytes());
                digest.update(metadata.modification_time().to_le_bytes());
                digest.update(metadata.change_time().to_le_bytes());
                digest.update(metadata.birth_time().to_le_bytes());
                digest.update(metadata.size().to_le_bytes());
                digest.update(metadata.blocks_512().to_le_bytes());
                digest.update(metadata.preferred_block_size().to_le_bytes());
            }
            digest.update(
                u64::try_from(attempt.mutable_byte_operand_resolutions().len())
                    .expect("build observation mutable-byte-resolution count fits u64")
                    .to_le_bytes(),
            );
            for operand in attempt.mutable_byte_operand_resolutions() {
                digest.update([operand.operand_ordinal()]);
                hash_bytes(&mut digest, operand.bytes());
            }
            digest.update(
                u64::try_from(attempt.mutable_i64_operand_resolutions().len())
                    .expect("build observation mutable-i64-resolution count fits u64")
                    .to_le_bytes(),
            );
            for operand in attempt.mutable_i64_operand_resolutions() {
                digest.update([operand.operand_ordinal()]);
                digest.update(operand.value().to_le_bytes());
            }
            digest.update(
                u64::try_from(attempt.mutable_byte_operands().len())
                    .expect("build observation mutable-byte-operand count fits u64")
                    .to_le_bytes(),
            );
            for operand in attempt.mutable_byte_operands() {
                digest.update([operand.operand_ordinal()]);
                hash_bytes(&mut digest, operand.pre_bytes());
                hash_bytes(&mut digest, operand.post_bytes());
            }
            digest.update(
                u64::try_from(attempt.mutable_i64_operands().len())
                    .expect("build observation mutable-i64-operand count fits u64")
                    .to_le_bytes(),
            );
            for operand in attempt.mutable_i64_operands() {
                digest.update([operand.operand_ordinal()]);
                digest.update(operand.pre_value().to_le_bytes());
                digest.update(operand.post_value().to_le_bytes());
            }
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

const fn observation_class_tag(class: BuildObservationClass) -> u8 {
    match class {
        BuildObservationClass::Hermetic => 0,
        BuildObservationClass::Receipted => 1,
        BuildObservationClass::Volatile => 2,
    }
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

const fn filesystem_replay_disposition_tag(disposition: BuildFilesystemReplayDisposition) -> u8 {
    match disposition {
        BuildFilesystemReplayDisposition::NotReplayed => 0,
        BuildFilesystemReplayDisposition::SourceInputsOnly => 1,
        BuildFilesystemReplayDisposition::Complete => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_summary() -> BuildObservationSummary {
        BuildObservationSummary {
            schema_version: BUILD_OBSERVATION_SCHEMA_VERSION,
            ceiling: BuildObservationClass::Hermetic,
            realized: BuildObservationClass::Hermetic,
            filesystem_operation_schema_version:
                psi_checked_interpreter::FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION,
            filesystem_operation_attempts: Vec::new(),
            canonical_source_metadata_identity: None,
            filesystem_replay_verdict: BuildFilesystemReplayVerdict::new(
                BuildFilesystemReplayDisposition::NotReplayed,
            ),
            included_source_handoffs: Vec::new(),
            staged_output_tree: None,
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
                0xc3, 0x0a, 0xa3, 0x33, 0xc4, 0xf4, 0xa3, 0x13, 0xe8, 0x34, 0x35, 0xe9, 0x5a, 0x43,
                0x07, 0x70, 0x99, 0xb7, 0xae, 0xe8, 0xcc, 0x22, 0xa5, 0x6b, 0x31, 0xb0, 0xf4, 0xff,
                0x0f, 0x7d, 0xc5, 0x50,
            ],
            "the current package build-observation byte contract remains stable"
        );
    }

    #[test]
    fn identity_binds_top_level_observation_contract() {
        let baseline = empty_summary().identity();

        let mut changed = empty_summary();
        changed.schema_version += 1;
        assert_ne!(baseline, changed.identity());

        let mut changed = empty_summary();
        changed.ceiling = BuildObservationClass::Receipted;
        assert_ne!(baseline, changed.identity());

        let mut changed = empty_summary();
        changed.realized = BuildObservationClass::Volatile;
        assert_ne!(baseline, changed.identity());

        let mut changed = empty_summary();
        changed.filesystem_operation_schema_version += 1;
        assert_ne!(baseline, changed.identity());

        let mut changed = empty_summary();
        changed.filesystem_replay_verdict =
            BuildFilesystemReplayVerdict::new(BuildFilesystemReplayDisposition::SourceInputsOnly);
        assert_ne!(baseline, changed.identity());

        let mut changed = empty_summary();
        changed.filesystem_replay_verdict =
            BuildFilesystemReplayVerdict::new(BuildFilesystemReplayDisposition::Complete);
        assert_ne!(baseline, changed.identity());
    }

    #[test]
    fn replay_verdict_has_exactly_three_closed_dispositions() {
        let not_replayed =
            BuildFilesystemReplayVerdict::new(BuildFilesystemReplayDisposition::NotReplayed);
        assert_eq!(
            not_replayed.schema_version(),
            crate::BUILD_FILESYSTEM_REPLAY_VERDICT_SCHEMA_VERSION
        );
        assert!(!not_replayed.replays_source_inputs());
        assert!(!not_replayed.is_complete());

        let source_inputs_only =
            BuildFilesystemReplayVerdict::new(BuildFilesystemReplayDisposition::SourceInputsOnly);
        assert!(source_inputs_only.replays_source_inputs());
        assert!(!source_inputs_only.is_complete());

        let complete =
            BuildFilesystemReplayVerdict::new(BuildFilesystemReplayDisposition::Complete);
        assert!(complete.replays_source_inputs());
        assert!(complete.is_complete());
    }
}
