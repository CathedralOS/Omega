//! Framing and validation for compiler-issued records retained by a baseline.

use super::{Decoder, Encoder};
use crate::declarations::PackageKey;
use crate::review::baseline::validation::replay_record_limits;
use crate::review::baseline::{ReviewOnlyBaselineError, ReviewOnlyBaselineLimits};
use crate::review::candidate::ReviewOnlyCanonicalRow;
use build_evaluation::{
    ReviewOnlyBuildFilesystemReplayRecord, recover_review_only_build_filesystem_replay_record,
};
use package_evidence::encoding::{
    PackageReviewCanonicalRowRecoveryLimits, decode_package_review_canonical_row_with_limits,
};

pub(in crate::review::baseline) fn encode_replay_record_option(
    encoder: &mut Encoder,
    replay: Option<&ReviewOnlyBuildFilesystemReplayRecord>,
) -> Result<(), ReviewOnlyBaselineError> {
    match replay {
        None => encoder.byte(0),
        Some(replay) => {
            encoder.byte(1);
            encoder.bytes(replay.canonical_bytes())?;
        }
    }
    Ok(())
}

pub(in crate::review::baseline) fn decode_replay_record_option(
    decoder: &mut Decoder<'_>,
    limits: ReviewOnlyBaselineLimits,
) -> Result<Option<ReviewOnlyBuildFilesystemReplayRecord>, ReviewOnlyBaselineError> {
    match decoder.byte()? {
        0 => Ok(None),
        1 => recover_review_only_build_filesystem_replay_record(
            decoder.bytes(limits.maximum_capsule_bytes)?,
            replay_record_limits(limits),
        )
        .map(Some)
        .map_err(|_| ReviewOnlyBaselineError::new("invalid compiler filesystem replay record")),
        _ => Err(ReviewOnlyBaselineError::new(
            "invalid filesystem-replay-record option tag",
        )),
    }
}

pub(in crate::review::baseline) fn validate_recovery_row<'a>(
    row: &'a ReviewOnlyCanonicalRow,
    key: &PackageKey,
    target: &str,
    limits: PackageReviewCanonicalRowRecoveryLimits,
) -> Result<&'a [u8], ReviewOnlyBaselineError> {
    let recovery_bytes = row.recovery_bytes().ok_or_else(|| {
        ReviewOnlyBaselineError::new("review baseline contains a non-recoverable comparison row")
    })?;
    let decoded = decode_package_review_canonical_row_with_limits(recovery_bytes, limits)
        .map_err(|_| ReviewOnlyBaselineError::new("invalid recovered compiler review row"))?;
    if decoded.package() != key.identity()
        || decoded.target().target_name() != target
        || decoded.kind() != row.kind()
        || decoded.risk() != row.risk()
        || decoded.key_bytes() != row.key_bytes()
        || decoded.canonical_bytes() != row.canonical_bytes()
        || decoded.source() != row.source()
    {
        return Err(ReviewOnlyBaselineError::new(
            "recovered compiler review row disagrees with review-only comparison metadata",
        ));
    }
    Ok(recovery_bytes)
}
