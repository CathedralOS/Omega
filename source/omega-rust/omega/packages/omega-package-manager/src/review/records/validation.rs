//! Exact-key validation across compiler review and resolved source custody.

use crate::resolution::{PackageSourceCustody, ResolvedPackageSourceClosure};
use crate::review::CompilerIssuedPackageReviewSet;
use crate::review::records::PackageReviewEvidence;
use omega_package_source::{ImmutableSourceResolution, PackageKey};

/// A compiler review set validated independently of source custody.
///
/// This remains review-only state. In particular, successful validation does
/// not make the rows accepted evidence or a persistable lock baseline.
pub(crate) struct ValidatedReviewOnlySet<'review, R> {
    reviews_by_key: Vec<&'review R>,
}

impl<'review, R> ValidatedReviewOnlySet<'review, R> {
    pub(crate) fn into_reviews_by_key(self) -> Vec<&'review R> {
        self.reviews_by_key
    }
}

/// A complete exact-key join between resolver custody and compiler reviews.
///
/// Construction proves only the invariants checked in this module. It does not
/// seal compiler/toolchain provenance or construct a `PackageInstance`.
pub(crate) struct ValidatedReviewOnlyClosure<'review, R> {
    reviews_by_key: Vec<&'review R>,
}

impl<'review, R> ValidatedReviewOnlyClosure<'review, R> {
    pub(crate) fn into_reviews_by_key(self) -> Vec<&'review R> {
        self.reviews_by_key
    }
}

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

pub(crate) fn validate_review_only_records<R: PackageReviewEvidence>(
    reviews: &[R],
) -> Result<ValidatedReviewOnlySet<'_, R>, ReviewOnlySetValidationError> {
    let reviews_by_key = validate_review_records(reviews)?;
    Ok(ValidatedReviewOnlySet { reviews_by_key })
}

pub(crate) fn validate_review_only_closure<'review>(
    sources: &ResolvedPackageSourceClosure,
    reviews: &'review CompilerIssuedPackageReviewSet,
) -> Result<
    ValidatedReviewOnlyClosure<'review, crate::review::CompilerIssuedPackageReview>,
    ReviewOnlyClosureValidationError,
> {
    let reviews_by_key = validate_review_closure_records(sources.custodies(), reviews.reviews())?;
    Ok(ValidatedReviewOnlyClosure { reviews_by_key })
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

fn validate_review_closure_records<'review, S: SourceRecord, R: ReviewRecord>(
    sources: &[S],
    reviews: &'review [R],
) -> Result<Vec<&'review R>, ReviewOnlyClosureValidationError> {
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

    Ok(reviews_by_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_package_source::{GitCommitId, GitTreeId, PackageName, SourceLineage};

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestSource {
        key: PackageKey,
        resolution: ImmutableSourceResolution,
    }

    impl SourceRecord for TestSource {
        fn key(&self) -> &PackageKey {
            &self.key
        }

        fn resolution(&self) -> &ImmutableSourceResolution {
            &self.resolution
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestReview {
        source: TestSource,
        projection_identity_matches: bool,
        target: u8,
    }

    impl SourceRecord for TestReview {
        fn key(&self) -> &PackageKey {
            self.source.key()
        }

        fn resolution(&self) -> &ImmutableSourceResolution {
            self.source.resolution()
        }
    }

    impl ReviewRecord for TestReview {
        fn projection_identity_matches(&self) -> bool {
            self.projection_identity_matches
        }

        fn target_matches(&self, other: &Self) -> bool {
            self.target == other.target
        }
    }

    fn source(name: &str, marker: u8) -> TestSource {
        let key = PackageKey::new(
            PackageName::parse(name).expect("valid package name"),
            SourceLineage::git(&format!("https://github.com/CathedralOS/{name}.git"))
                .expect("valid package lineage"),
        );
        TestSource {
            key,
            resolution: resolution(marker),
        }
    }

    fn resolution(marker: u8) -> ImmutableSourceResolution {
        ImmutableSourceResolution::git(
            GitCommitId::parse_hex(&format!("{marker:02x}").repeat(20)).expect("valid commit"),
            GitTreeId::parse_hex(&format!("{:02x}", marker.wrapping_add(1)).repeat(20))
                .expect("valid tree"),
        )
        .expect("matching Git identities")
    }

    fn review(source: &TestSource) -> TestReview {
        TestReview {
            source: source.clone(),
            projection_identity_matches: true,
            target: 1,
        }
    }

    #[test]
    fn exact_custody_review_bijection_succeeds() {
        let root = source("application", 1);
        let dependency = source("dependency", 2);
        let reviews = vec![review(&dependency), review(&root)];

        let validated =
            validate_review_closure_records(&[root.clone(), dependency.clone()], &reviews)
                .expect("exact closure should validate");

        assert_eq!(validated.len(), 2);
        assert_eq!(validated[0].key(), root.key());
        assert_eq!(validated[1].key(), dependency.key());
    }

    #[test]
    fn duplicate_review_is_rejected() {
        let root = source("application", 1);
        let duplicate = review(&root);

        assert_eq!(
            validate_review_closure_records(&[root.clone()], &[duplicate.clone(), duplicate]),
            Err(ReviewOnlyClosureValidationError::ReviewSet(
                ReviewOnlySetValidationError::DuplicateReview {
                    package: root.key.clone(),
                }
            ))
        );
    }

    #[test]
    fn missing_review_is_rejected() {
        let root = source("application", 1);
        let dependency = source("dependency", 2);

        assert_eq!(
            validate_review_closure_records(&[root.clone(), dependency.clone()], &[review(&root)],),
            Err(ReviewOnlyClosureValidationError::MissingReview {
                package: dependency.key.clone(),
            })
        );
    }

    #[test]
    fn unexpected_review_is_rejected() {
        let root = source("application", 1);
        let dependency = source("dependency", 2);

        assert_eq!(
            validate_review_closure_records(&[root.clone()], &[review(&root), review(&dependency)],),
            Err(ReviewOnlyClosureValidationError::UnexpectedReview {
                package: dependency.key.clone(),
            })
        );
    }

    #[test]
    fn resolution_mismatch_is_rejected() {
        let root = source("application", 1);
        let mut stale = review(&root);
        stale.source.resolution = resolution(3);

        assert_eq!(
            validate_review_closure_records(&[root.clone()], &[stale]),
            Err(ReviewOnlyClosureValidationError::ResolutionMismatch {
                package: root.key.clone(),
            })
        );
    }

    #[test]
    fn projection_identity_mismatch_is_rejected() {
        let root = source("application", 1);
        let mut spoofed = review(&root);
        spoofed.projection_identity_matches = false;

        assert_eq!(
            validate_review_closure_records(&[root.clone()], &[spoofed]),
            Err(ReviewOnlyClosureValidationError::ReviewSet(
                ReviewOnlySetValidationError::ProjectionIdentityMismatch {
                    package: root.key.clone(),
                }
            ))
        );
    }

    #[test]
    fn mixed_targets_are_rejected() {
        let root = source("application", 1);
        let dependency = source("dependency", 2);
        let root_review = review(&root);
        let mut dependency_review = review(&dependency);
        dependency_review.target = 2;

        assert_eq!(
            validate_review_closure_records(
                &[root.clone(), dependency.clone()],
                &[root_review, dependency_review],
            ),
            Err(ReviewOnlyClosureValidationError::ReviewSet(
                ReviewOnlySetValidationError::MixedTarget {
                    first: root.key.clone(),
                    conflicting: dependency.key.clone(),
                }
            ))
        );
    }
}
