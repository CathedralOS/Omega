//! Canonical commitments over compiler review and build observations.

use omega_build_evaluation::{
    BuildFilesystemGrantAccess, BuildFilesystemGrantRefusalReason,
    BuildFilesystemLogicalHandleInputResolution, BuildFilesystemLogicalHandleKind,
    BuildFilesystemLogicalHandleOutputSource, BuildFilesystemOperationResult,
    BuildFilesystemProvider, BuildFilesystemRoot, BuildFilesystemScalarOperandValue,
    BuildObservationClass, BuildObservationSummary,
};
use sha2::{Digest, Sha256};

const WHOLE_REVIEW_COMMITMENT_DOMAIN: &[u8] = b"OMEGA-PACKAGE-REVIEW-COMPARISON\\0";
const BUILD_OBSERVATION_COMMITMENT_DOMAIN: &[u8] = b"OMEGA-PACKAGE-BUILD-OBSERVATION-COMPARISON\\0";

pub(crate) fn whole_review_commitment(canonical_review_bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(WHOLE_REVIEW_COMMITMENT_DOMAIN);
    hash_bytes(&mut digest, canonical_review_bytes);
    digest.finalize().into()
}

pub(crate) fn build_observation_commitment(summary: &BuildObservationSummary) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(BUILD_OBSERVATION_COMMITMENT_DOMAIN);
    digest.update(summary.schema_version().to_le_bytes());
    digest.update([observation_class_tag(summary.ceiling())]);
    digest.update([observation_class_tag(summary.realized())]);
    digest.update(summary.filesystem_operation_schema_version().to_le_bytes());
    match summary.canonical_source_metadata_identity() {
        None => digest.update([0]),
        Some(identity) => {
            digest.update([1]);
            digest.update(identity.policy_version().to_le_bytes());
            digest.update(identity.source_content_commitment());
        }
    }
    digest.update([u8::from(summary.source_inputs_replay_verified())]);
    digest.update([u8::from(summary.operation_replay_verified())]);
    digest.update(
        u64::try_from(summary.included_source_paths().len())
            .expect("included-source path count fits u64")
            .to_le_bytes(),
    );
    for path in summary.included_source_paths() {
        hash_bytes(&mut digest, path);
    }
    match summary.staged_output_tree() {
        None => digest.update([0]),
        Some(tree) => {
            digest.update([1]);
            digest.update(tree.digest());
            digest.update(tree.entry_count().to_le_bytes());
            digest.update(tree.file_bytes().to_le_bytes());
        }
    }
    digest.update(
        u64::try_from(summary.filesystem_operation_attempts().len())
            .expect("build observation attempt count fits u64")
            .to_le_bytes(),
    );
    for attempt in summary.filesystem_operation_attempts() {
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
                omega_build_evaluation::BuildFilesystemRoot::Source => 0,
                omega_build_evaluation::BuildFilesystemRoot::Output => 1,
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
                omega_build_evaluation::BuildFilesystemReturnedPathKind::ReadLinkPayload => 0,
                omega_build_evaluation::BuildFilesystemReturnedPathKind::CanonicalPath => 1,
                omega_build_evaluation::BuildFilesystemReturnedPathKind::FinalPath => 2,
            }]);
            digest.update([match returned.completeness() {
                omega_build_evaluation::BuildFilesystemReturnedPathCompleteness::Complete => 0,
                omega_build_evaluation::BuildFilesystemReturnedPathCompleteness::LimitReached => 1,
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
                omega_build_evaluation::BuildFilesystemObservedByteRegionKind::SequentialFileRead => 0,
                omega_build_evaluation::BuildFilesystemObservedByteRegionKind::PositionedFileRead => 1,
                omega_build_evaluation::BuildFilesystemObservedByteRegionKind::DirectoryRecords => 2,
                omega_build_evaluation::BuildFilesystemObservedByteRegionKind::FindEntry => 3,
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
                omega_build_evaluation::BuildFilesystemMetadataObservationKind::FollowedPath => 0,
                omega_build_evaluation::BuildFilesystemMetadataObservationKind::OpenDescriptor => 1,
                omega_build_evaluation::BuildFilesystemMetadataObservationKind::UnfollowedFinalPath => 2,
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
    digest.finalize().into()
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(
        u64::try_from(bytes.len())
            .expect("review evidence byte length fits u64")
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

#[cfg(test)]
mod tests;
