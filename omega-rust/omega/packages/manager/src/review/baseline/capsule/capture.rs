//! Capture of compiler-issued review evidence into a baseline capsule.

use super::{ReviewOnlyBaselineCapsule, ReviewOnlyBaselinePackage};
use crate::resolution::graph::ResolvedPackageSourceClosure;
use crate::review::CompilerIssuedPackageReviewSet;
use crate::review::baseline::encoding::replay_parent_binding;
use crate::review::baseline::validation::{canonical_graph, row_limits};
use crate::review::baseline::{ReviewOnlyBaselineError, ReviewOnlyBaselineLimits};
use crate::review::candidate::validation::validate_review_only_closure;
use crate::review::candidate::{
    ReviewOnlyCanonicalRow, build_observation_commitment, whole_review_commitment,
};
use build_evaluation::{
    BuildFilesystemReplayRecordLimits, capture_verified_build_filesystem_replay_record,
};
use package_evidence::encoding::{
    decode_package_review_canonical_row_with_limits,
    encode_package_review_canonical_row_with_limits,
};

impl ReviewOnlyBaselineCapsule {
    pub fn capture(
        sources: &ResolvedPackageSourceClosure,
        reviews: &CompilerIssuedPackageReviewSet,
        limits: ReviewOnlyBaselineLimits,
    ) -> Result<Self, ReviewOnlyBaselineError> {
        let validated = validate_review_only_closure(sources, reviews).map_err(|_| {
            ReviewOnlyBaselineError::new("cannot capture an invalid review-only source closure")
        })?;
        let mut packages = Vec::new();
        packages
            .try_reserve_exact(reviews.reviews().len())
            .map_err(|_| ReviewOnlyBaselineError::new("baseline package allocation failed"))?;
        let row_limits = row_limits(limits);
        let mut replay_record_bytes = 0usize;
        for review in validated.into_reviews_by_key() {
            let mut rows = Vec::new();
            rows.try_reserve_exact(review.canonical_rows().len())
                .map_err(|_| ReviewOnlyBaselineError::new("baseline row allocation failed"))?;
            for row in review.canonical_rows() {
                let encoded = encode_package_review_canonical_row_with_limits(row, row_limits)
                    .map_err(|_| {
                        ReviewOnlyBaselineError::new("compiler row cannot enter review baseline")
                    })?;
                let decoded = decode_package_review_canonical_row_with_limits(&encoded, row_limits)
                    .map_err(|_| {
                        ReviewOnlyBaselineError::new("compiler row recovery self-check failed")
                    })?;
                if decoded.package() != review.key().identity()
                    || decoded.target().target_name() != review.projection().target().target_name()
                {
                    return Err(ReviewOnlyBaselineError::new(
                        "compiler row package or target disagrees with its review",
                    ));
                }
                rows.push(ReviewOnlyCanonicalRow::from_recovered(&decoded, encoded));
            }
            let build_observation_commitment = review
                .build_observation_summary()
                .map(build_observation_commitment);
            let remaining_replay_bytes = limits
                .maximum_capsule_bytes
                .checked_sub(replay_record_bytes)
                .ok_or_else(|| {
                    ReviewOnlyBaselineError::new(
                        "review baseline replay records exceed their aggregate ceiling",
                    )
                })?;
            let filesystem_replay_record = review
                .build_observation_summary()
                .map(|summary| {
                    capture_verified_build_filesystem_replay_record(
                        summary,
                        BuildFilesystemReplayRecordLimits::new(remaining_replay_bytes, 4_096),
                    )
                    .map_err(|_| {
                        ReviewOnlyBaselineError::new(
                            "compiler replay record cannot enter review baseline",
                        )
                    })
                })
                .transpose()?
                .flatten();
            if let Some(record) = &filesystem_replay_record {
                replay_record_bytes = replay_record_bytes
                    .checked_add(record.canonical_bytes().len())
                    .filter(|bytes| *bytes <= limits.maximum_capsule_bytes)
                    .ok_or_else(|| {
                        ReviewOnlyBaselineError::new(
                            "review baseline replay records exceed their aggregate ceiling",
                        )
                    })?;
            }
            let replay_record_parent_binding = match (
                build_observation_commitment,
                filesystem_replay_record.as_ref(),
            ) {
                (Some(parent), Some(record)) => {
                    Some(replay_parent_binding(parent, record.commitment()))
                }
                (None, None) | (Some(_), None) => None,
                (None, Some(_)) => {
                    return Err(ReviewOnlyBaselineError::new(
                        "filesystem replay record has no parent build observation",
                    ));
                }
            };
            packages.push(ReviewOnlyBaselinePackage {
                key: review.key().clone(),
                resolution: review.resolution().clone(),
                target: review.projection().target().target_name().to_owned(),
                source_consumption_commitment: review.source_consumption_commitment().into(),
                build_observation_commitment,
                filesystem_replay_record,
                replay_record_parent_binding,
                whole_review_commitment: whole_review_commitment(review.canonical_review_bytes()),
                canonical_rows: rows,
            });
        }
        let graph = canonical_graph(sources.graph())?;
        let capsule = Self { graph, packages };
        capsule.validate(limits)?;
        let _ = capsule.encode(limits)?;
        Ok(capsule)
    }
}
