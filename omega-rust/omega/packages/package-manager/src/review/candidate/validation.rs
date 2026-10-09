//! Exact-key validation across compiler review and resolved source custody.

use super::PackageReviewEvidence;
use crate::declarations::PackageKey;
use crate::resolution::graph::ResolvedPackageSourceClosure;
use crate::resolution::source::PackageSourceCustody;
use crate::review::CompilerIssuedPackageReviewSet;
use package_source::ImmutableSourceResolution;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReviewOnlySetValidationError {
    DuplicateReview {
        package: PackageKey,
    },
    ProjectionIdentityMismatch {
        package: PackageKey,
    },
    MixedTarget {
        first: PackageKey,
        conflicting: PackageKey,
    },
    AllocationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReviewOnlyClosureValidationError {
    ReviewSet(ReviewOnlySetValidationError),
    MissingReview { package: PackageKey },
    UnexpectedReview { package: PackageKey },
    ResolutionMismatch { package: PackageKey },
    AllocationFailed,
}

/// Validate a compiler review set independently of source custody. Success
/// does not make the rows accepted evidence or a persistable lock baseline.
pub(crate) fn validate_review_only_records<R: PackageReviewEvidence>(
    reviews: &[R],
) -> Result<(), ReviewOnlySetValidationError> {
    validate_review_records(reviews).map(drop)
}

/// Validate the exact-key join between resolver custody and compiler reviews.
/// This does not seal compiler/toolchain provenance or construct a
/// `PackageInstance`.
pub(crate) fn validate_review_only_closure(
    sources: &ResolvedPackageSourceClosure,
    reviews: &CompilerIssuedPackageReviewSet,
) -> Result<(), ReviewOnlyClosureValidationError> {
    validate_review_closure_records(sources.custodies(), reviews.reviews())
}

trait SourceRecord {
    fn key(&self) -> &PackageKey;
    fn resolution(&self) -> &ImmutableSourceResolution;
}

trait ReviewRecord: SourceRecord {
    fn projection_identity_matches(&self) -> bool;
    fn target_matches(&self, other: &Self) -> bool;
}

impl SourceRecord for PackageSourceCustody {
    fn key(&self) -> &PackageKey {
        PackageSourceCustody::key(self)
    }

    fn resolution(&self) -> &ImmutableSourceResolution {
        PackageSourceCustody::resolution(self)
    }
}

impl<R: PackageReviewEvidence> SourceRecord for R {
    fn key(&self) -> &PackageKey {
        PackageReviewEvidence::key(self)
    }

    fn resolution(&self) -> &ImmutableSourceResolution {
        PackageReviewEvidence::resolution(self)
    }
}

impl<R: PackageReviewEvidence> ReviewRecord for R {
    fn projection_identity_matches(&self) -> bool {
        PackageReviewEvidence::projection_identity_matches(self)
    }

    fn target_matches(&self, other: &Self) -> bool {
        self.target_name() == other.target_name()
    }
}

fn validate_review_records<R: ReviewRecord>(
    reviews: &[R],
) -> Result<Vec<&R>, ReviewOnlySetValidationError> {
    let mut reviews_by_key = Vec::new();
    reviews_by_key
        .try_reserve_exact(reviews.len())
        .map_err(|_| ReviewOnlySetValidationError::AllocationFailed)?;
    reviews_by_key.extend(reviews);
    reviews_by_key.sort_by(|left, right| left.key().cmp(right.key()));

    if let Some(pair) = reviews_by_key
        .windows(2)
        .find(|pair| pair[0].key() == pair[1].key())
    {
        return Err(ReviewOnlySetValidationError::DuplicateReview {
            package: pair[0].key().clone(),
        });
    }

    for review in &reviews_by_key {
        if !review.projection_identity_matches() {
            return Err(ReviewOnlySetValidationError::ProjectionIdentityMismatch {
                package: review.key().clone(),
            });
        }
    }

    if let Some(first) = reviews_by_key.first().copied() {
        for review in reviews_by_key.iter().copied().skip(1) {
            if !first.target_matches(review) {
                return Err(ReviewOnlySetValidationError::MixedTarget {
                    first: first.key().clone(),
                    conflicting: review.key().clone(),
                });
            }
        }
    }

    Ok(reviews_by_key)
}

fn validate_review_closure_records<S: SourceRecord, R: ReviewRecord>(
    sources: &[S],
    reviews: &[R],
) -> Result<(), ReviewOnlyClosureValidationError> {
    let reviews_by_key =
        validate_review_records(reviews).map_err(ReviewOnlyClosureValidationError::ReviewSet)?;
    let mut sources_by_key = Vec::new();
    sources_by_key
        .try_reserve_exact(sources.len())
        .map_err(|_| ReviewOnlyClosureValidationError::AllocationFailed)?;
    sources_by_key.extend(sources);
    sources_by_key.sort_by(|left, right| left.key().cmp(right.key()));

    for source in &sources_by_key {
        let Ok(review_index) =
            reviews_by_key.binary_search_by(|review| review.key().cmp(source.key()))
        else {
            return Err(ReviewOnlyClosureValidationError::MissingReview {
                package: source.key().clone(),
            });
        };
        if reviews_by_key[review_index].resolution() != source.resolution() {
            return Err(ReviewOnlyClosureValidationError::ResolutionMismatch {
                package: source.key().clone(),
            });
        }
    }

    for review in &reviews_by_key {
        if sources_by_key
            .binary_search_by(|source| source.key().cmp(review.key()))
            .is_err()
        {
            return Err(ReviewOnlyClosureValidationError::UnexpectedReview {
                package: review.key().clone(),
            });
        }
    }

    Ok(())
}
