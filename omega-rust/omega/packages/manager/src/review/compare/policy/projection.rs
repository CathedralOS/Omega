use super::{PackagePolicyChangeError as Error, limits::Budget};
use crate::declarations::PackageKey;
use crate::lock::{PackageAcceptanceRow, PackageOccurrenceRoster};
use crate::resolution::graph::{CanonicalSourceClosureSubject, ExactTargetPackageSourceClosure};
use crate::review::{CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet};
use package_evidence::record::PackagePolicyBaseline;

pub(super) fn candidate<'a>(
    candidate: &'a CompilerIssuedPackageReviewSet,
    sources: &ExactTargetPackageSourceClosure<'_>,
    source: &CanonicalSourceClosureSubject,
    budget: &mut Budget,
) -> Result<Vec<&'a CompilerIssuedPackageReview>, Error> {
    let count = candidate.reviews().len();
    if count
        > budget
            .limits
            .maximum_packages
            .saturating_mul(crate::declarations::DependencyPurpose::ALL.len())
        || sources.source_closure().custodies().len() > budget.limits.maximum_packages
    {
        return Err(Error::LimitExceeded {
            resource: "candidate packages",
            maximum: budget.limits.maximum_packages,
        });
    }
    budget.slots::<&CompilerIssuedPackageReview>(count)?;
    budget.slots::<&crate::resolution::source::PackageSourceCustody>(
        sources.source_closure().custodies().len(),
    )?;
    let mut reviews = Vec::new();
    reviews
        .try_reserve_exact(count)
        .map_err(|_| Error::AllocationFailed)?;
    reviews.extend(candidate.reviews());
    reviews.sort_unstable_by(|left, right| {
        (left.key(), left.checked_context().purpose())
            .cmp(&(right.key(), right.checked_context().purpose()))
    });
    let mut custodies = Vec::new();
    custodies
        .try_reserve_exact(sources.source_closure().custodies().len())
        .map_err(|_| Error::AllocationFailed)?;
    custodies.extend(sources.source_closure().custodies());
    custodies.sort_unstable_by(|left, right| left.key().cmp(right.key()));
    if let Some(pair) = reviews.windows(2).find(|pair| {
        pair[0].key() == pair[1].key()
            && pair[0].checked_context().purpose() == pair[1].checked_context().purpose()
    }) {
        return Err(invalid(
            pair[0].key(),
            "duplicate checked occurrence review",
        ));
    }
    let roster = PackageOccurrenceRoster::derive(source)?;
    if count != roster.occurrence_count() {
        return Err(Error::CandidateReview {
            package: None,
            reason: "checked review occurrences do not cover the source graph",
        });
    }
    let execution_profile = reviews
        .first()
        .and_then(|review| review.checked_context().build_execution_profile());
    for review in &reviews {
        budget.context(review.canonical_review_bytes().len())?;
        budget.context(review.selected_build_machine_identity().len())?;
        if review.projection().package() != review.key().identity()
            || review.policy().package() != review.key().identity()
        {
            return Err(Error::CandidateReview {
                package: Some(Box::new(review.key().clone())),
                reason: "normalized policy owner differs",
            });
        }
        let context = review.checked_context();
        if !roster.is_occurrence(review.key(), context.purpose()) {
            return Err(invalid(
                review.key(),
                "checked review purpose differs from its source occurrence",
            ));
        }
        let expected_target = if context.purpose().is_product() {
            Some(sources.target_profile())
        } else {
            execution_profile
        };
        if Some(context.target()) != expected_target
            || review.projection().target() != context.target()
            || review.policy().target() != context.target()
        {
            return Err(Error::TargetMismatch);
        }
        if context.build_execution_profile() != execution_profile {
            return Err(invalid(
                review.key(),
                "checked reviews use different build execution profiles",
            ));
        }
    }
    for custody in &custodies {
        let start = reviews.partition_point(|review| review.key() < custody.key());
        let selected = &reviews[start..]
            [..reviews[start..].partition_point(|review| review.key() == custody.key())];
        if selected.is_empty() {
            return Err(invalid(custody.key(), "missing review"));
        }
        if selected
            .iter()
            .any(|review| review.resolution() != custody.resolution())
        {
            return Err(invalid(
                custody.key(),
                "immutable source resolution differs",
            ));
        }
    }
    for review in &reviews {
        if custodies
            .binary_search_by(|custody| custody.key().cmp(review.key()))
            .is_err()
        {
            return Err(invalid(review.key(), "unexpected review"));
        }
    }
    Ok(reviews)
}

pub(super) fn rows(
    package: &PackageKey,
    policy: &PackagePolicyBaseline,
    budget: &mut Budget,
) -> Result<Vec<PackageAcceptanceRow>, Error> {
    let (acceptance, usage) =
        crate::lock::PackagePolicyAcceptance::project(policy, budget.row_limits()).map_err(
            |error| Error::Projection {
                package: Box::new(package.clone()),
                error,
            },
        )?;
    budget.projected(usage)?;
    Ok(acceptance.into_rows())
}

fn invalid(package: &PackageKey, reason: &'static str) -> Error {
    Error::CandidateReview {
        package: Some(Box::new(package.clone())),
        reason,
    }
}
