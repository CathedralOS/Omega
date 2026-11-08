//! Projection of crash predicates and permission-frontier claims.

use crate::record::PackageReviewCrashPredicate;

pub(super) fn project_crash_predicate(
    predicate: &checked_trees::CrashPredicateIdentity,
) -> PackageReviewCrashPredicate {
    PackageReviewCrashPredicate {
        canonical_bytes: predicate.canonical_bytes().to_vec(),
    }
}
