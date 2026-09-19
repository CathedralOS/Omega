//! The replay record itself: its limits, error, the review-only carrier,
//! capturing a verified record from a build and recovering one for review.

use crate::evidence::replay_record::attempt_codec::{
    Encoder, encode_attempt, encode_replay_activation,
};
use crate::evidence::replay_record::rehydration::decode_shapes;
use crate::{
    BuildCanonicalSourceMetadataIdentity, BuildCapturedSourceInventory, BuildFilesystemGrantAccess,
    BuildFilesystemGrantRefusalReason, BuildFilesystemOperationObservationClass,
    BuildFilesystemRoot, BuildObservationSummary, BuildReplayActivation,
};
use sha2::Digest;
use sha2::Sha256;
use std::fmt;

pub(crate) const MAGIC: &[u8] = b"OMEGA-BUILD-FILESYSTEM-REPLAY-RECORD\0";

const COMMITMENT_DOMAIN: &[u8] = b"OMEGA-BUILD-FILESYSTEM-REPLAY-RECORD-COMMITMENT\0";

pub(crate) const VERSION: u16 = 58;

/// Resource ceilings for build-evaluation recovery of one partial filesystem
/// replay record. These are decoder sponsorship limits, not Omega language
/// limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildFilesystemReplayRecordLimits {
    pub(crate) maximum_bytes: usize,
    pub(crate) maximum_items_per_lane: usize,
}

impl BuildFilesystemReplayRecordLimits {
    pub const fn new(maximum_bytes: usize, maximum_items_per_lane: usize) -> Self {
        Self {
            maximum_bytes,
            maximum_items_per_lane,
        }
    }

    pub const fn maximum_bytes(self) -> usize {
        self.maximum_bytes
    }
}

impl Default for BuildFilesystemReplayRecordLimits {
    fn default() -> Self {
        Self::new(64 * 1024 * 1024, 4_096)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFilesystemReplayRecordError {
    message: &'static str,
}

impl BuildFilesystemReplayRecordError {
    pub(crate) const fn new(message: &'static str) -> Self {
        Self { message }
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for BuildFilesystemReplayRecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for BuildFilesystemReplayRecordError {}

/// Canonical bytes recovered by the compiler for review-only custody.
///
/// Recovery does not reproduce compiler-issued evidence, authorize a build,
/// or establish `Receipted`. The complete bytes can later be handed back to a
/// replay executor once that executor supports restart-stable input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewOnlyBuildFilesystemReplayRecord {
    canonical_bytes: Vec<u8>,
    commitment: [u8; 32],
    canonical_source_metadata_identity: Option<BuildCanonicalSourceMetadataIdentity>,
    captured_source_inventory: Option<BuildCapturedSourceInventory>,
    replay_activation: BuildReplayActivation,
}

impl ReviewOnlyBuildFilesystemReplayRecord {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn commitment(&self) -> [u8; 32] {
        self.commitment
    }

    pub const fn canonical_source_metadata_identity(
        &self,
    ) -> Option<BuildCanonicalSourceMetadataIdentity> {
        self.canonical_source_metadata_identity
    }

    /// The exact activation this record's evidence was captured under. A
    /// replay request admits the record only when the requesting
    /// compilation's own root package, declaration role, selected target, and
    /// build execution profile agree with it.
    pub const fn replay_activation(&self) -> BuildReplayActivation {
        self.replay_activation
    }

    pub const fn captured_source_inventory(&self) -> Option<BuildCapturedSourceInventory> {
        self.captured_source_inventory
    }
}

/// Capture the exact operation record only after the compiler has completed
/// the bounded provider-free replay. A false replay fact produces no record.
pub fn capture_verified_build_filesystem_replay_record(
    summary: &BuildObservationSummary,
    limits: BuildFilesystemReplayRecordLimits,
) -> Result<Option<ReviewOnlyBuildFilesystemReplayRecord>, BuildFilesystemReplayRecordError> {
    let replay_verdict = summary.filesystem_replay_verdict();
    if !replay_verdict.replays_source_inputs() {
        return Ok(None);
    }
    let includes_output = summary
        .filesystem_operation_attempts()
        .iter()
        .any(|attempt| {
            attempt
                .rooted_path_operand_resolutions()
                .iter()
                .any(|path| path.root() == BuildFilesystemRoot::Output)
                || attempt
                    .authorized_paths()
                    .iter()
                    .any(|path| path.root() == BuildFilesystemRoot::Output)
        });
    if includes_output && !replay_verdict.is_complete() {
        return Ok(None);
    }
    if summary
        .filesystem_operation_attempts()
        .iter()
        .any(|attempt| {
            attempt.observation_class() != BuildFilesystemOperationObservationClass::Receipted
        })
    {
        return Ok(None);
    }
    if let [attempt] = summary.filesystem_operation_attempts()
        && matches!(attempt.operation_tag(), 1 | 9)
        && matches!(attempt.rooted_path_operand_resolutions(), [rooted] if rooted.root() == BuildFilesystemRoot::Source)
        && matches!(attempt.grant_refusals(), [refusal]
            if refusal.access() == BuildFilesystemGrantAccess::Write
                && refusal.reason() == BuildFilesystemGrantRefusalReason::OutsideGrantedRoots)
        && !summary.build_log().is_empty()
    {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay refused Source operation cannot carry BuildLog output",
        ));
    }
    let mut encoder = Encoder::new(limits.maximum_bytes);
    encoder.fixed(MAGIC);
    encoder.u16(VERSION);
    encoder.u32(summary.schema_version());
    encoder.u32(summary.filesystem_operation_schema_version());
    match summary.canonical_source_metadata_identity() {
        None => encoder.byte(0),
        Some(identity) => {
            encoder.byte(1);
            encoder.u32(identity.policy_version());
            encoder.fixed(&identity.source_content_commitment());
        }
    }
    match summary.captured_source_inventory() {
        None => encoder.byte(0),
        Some(inventory) => {
            encoder.byte(1);
            encoder.u32(inventory.source_metadata_identity().policy_version());
            encoder.fixed(
                &inventory
                    .source_metadata_identity()
                    .source_content_commitment(),
            );
            encoder.u64(inventory.entry_count());
            encoder.u64(inventory.file_bytes());
        }
    }
    encode_replay_activation(&mut encoder, summary.replay_activation())?;
    encoder.count(summary.included_source_handoffs().len())?;
    for handoff in summary.included_source_handoffs() {
        encoder.bytes(handoff.relative_path())?;
        encoder.u64(handoff.filesystem_attempt_ordinal());
    }
    encoder.count(summary.filesystem_operation_attempts().len())?;
    for attempt in summary.filesystem_operation_attempts() {
        encode_attempt(&mut encoder, attempt)?;
    }
    let bytes = encoder.finish()?;
    let recovered = recover_review_only_build_filesystem_replay_record(&bytes, limits)?;
    Ok(Some(recovered))
}

/// Strictly recover a canonical, non-authoritative replay record.
pub fn recover_review_only_build_filesystem_replay_record(
    bytes: &[u8],
    limits: BuildFilesystemReplayRecordLimits,
) -> Result<ReviewOnlyBuildFilesystemReplayRecord, BuildFilesystemReplayRecordError> {
    let decoded = decode_shapes(bytes, limits)?;
    let canonical_bytes = clone_bytes(bytes)?;
    Ok(ReviewOnlyBuildFilesystemReplayRecord {
        commitment: record_commitment(&canonical_bytes),
        canonical_bytes,
        canonical_source_metadata_identity: decoded.canonical_source_metadata_identity,
        captured_source_inventory: decoded.captured_source_inventory,
        replay_activation: decoded.replay_activation,
    })
}

fn record_commitment(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(COMMITMENT_DOMAIN);
    digest.update(
        u64::try_from(bytes.len())
            .expect("bounded replay bytes fit u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
    digest.finalize().into()
}

pub(crate) fn clone_bytes(bytes: &[u8]) -> Result<Vec<u8>, BuildFilesystemReplayRecordError> {
    let mut cloned = Vec::new();
    cloned
        .try_reserve_exact(bytes.len())
        .map_err(|_| BuildFilesystemReplayRecordError::new("replay record allocation failed"))?;
    cloned.extend_from_slice(bytes);
    Ok(cloned)
}
