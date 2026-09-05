use super::{PackagePolicyChangeError as Error, limits::Budget};
use crate::declarations::PackageKey;
use crate::resolution::graph::ExactTargetPackageSourceClosure;
use crate::review::{CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet};
use omega_package_evidence::record::{PackagePolicyBaseline, PackagePolicyRow};

pub(super) fn candidate<'a>(
    candidate: &'a CompilerIssuedPackageReviewSet,
    sources: &ExactTargetPackageSourceClosure<'_>,
    budget: &mut Budget,
) -> Result<Vec<&'a CompilerIssuedPackageReview>, Error> {
    let count = candidate.reviews().len();
    if count > budget.limits.maximum_packages
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
    reviews.sort_unstable_by(|left, right| left.key().cmp(right.key()));
    let mut custodies = Vec::new();
    custodies
        .try_reserve_exact(sources.source_closure().custodies().len())
        .map_err(|_| Error::AllocationFailed)?;
    custodies.extend(sources.source_closure().custodies());
    custodies.sort_unstable_by(|left, right| left.key().cmp(right.key()));
    if let Some(pair) = reviews
        .windows(2)
        .find(|pair| pair[0].key() == pair[1].key())
    {
        return Err(invalid(pair[0].key(), "duplicate review"));
    }
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
        if review.projection().target() != sources.target_profile()
            || review.policy().target() != sources.target_profile()
        {
            return Err(Error::TargetMismatch);
        }
    }
    for custody in &custodies {
        let index = reviews
            .binary_search_by(|review| review.key().cmp(custody.key()))
            .map_err(|_| invalid(custody.key(), "missing review"))?;
        if reviews[index].resolution() != custody.resolution() {
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
) -> Result<Vec<PackagePolicyRow>, Error> {
    let (rows, usage) = policy
        .canonical_rows_with_limits(budget.row_limits())
        .map_err(|error| Error::Projection {
            package: Box::new(package.clone()),
            error,
        })?;
    budget.projected(usage)?;
    Ok(rows)
}

fn invalid(package: &PackageKey, reason: &'static str) -> Error {
    Error::CandidateReview {
        package: Some(Box::new(package.clone())),
        reason,
    }
}
