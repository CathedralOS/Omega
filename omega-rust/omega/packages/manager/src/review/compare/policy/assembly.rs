use super::{
    PackagePolicyChangeError, PackagePolicyChangeFingerprint, PackagePolicyPackageChange,
    fingerprints, limits::Budget, merge, paths, projection,
};
use crate::lock::PackageLockTarget;
use crate::resolution::graph::CanonicalSourceClosureSubject;
use crate::review::CompilerIssuedPackageReview;
use sha2::Sha256;

pub(super) fn packages(
    accepted: Option<&PackageLockTarget>,
    source: &CanonicalSourceClosureSubject,
    reviews: &[&CompilerIssuedPackageReview],
    budget: &mut Budget,
    context: &mut Sha256,
) -> Result<Vec<PackagePolicyPackageChange>, PackagePolicyChangeError> {
    let old_paths = accepted
        .map(|old| paths::Paths::new(old.source(), budget))
        .transpose()?;
    let new_paths = paths::Paths::new(source, budget)?;
    let old_sources = accepted.map_or(&[][..], |old| old.source().packages());
    let new_sources = source.packages();
    let shared_count = old_sources
        .iter()
        .filter(|old| {
            new_sources
                .binary_search_by(|new| new.key().cmp(old.key()))
                .is_ok()
        })
        .count();
    let count = old_sources
        .len()
        .checked_add(new_sources.len())
        .and_then(|count| count.checked_sub(shared_count))
        .ok_or(PackagePolicyChangeError::AllocationFailed)?;
    budget.package_slots(count)?;
    let mut packages = Vec::new();
    packages
        .try_reserve_exact(count)
        .map_err(|_| PackagePolicyChangeError::AllocationFailed)?;
    let (mut old_index, mut new_index) = (0, 0);
    while old_index < old_sources.len() || new_index < new_sources.len() {
        let old = old_sources.get(old_index);
        let new = new_sources.get(new_index);
        let ordering = match (old, new) {
            (Some(old), Some(new)) => old.key().cmp(new.key()),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => break,
        };
        let old = if ordering.is_gt() { None } else { old };
        let new = if ordering.is_lt() { None } else { new };
        let key = old.or(new).expect("package union has one side").key();
        let baseline =
            old.map(|_| &accepted.expect("old package has baseline").baselines()[old_index]);
        let review = new.map(|_| reviews[new_index]);
        let old_rows = baseline
            .map(|value| projection::rows(key, value, budget))
            .transpose()?
            .unwrap_or_default();
        let new_rows = review
            .map(|value| projection::rows(key, value.policy(), budget))
            .transpose()?
            .unwrap_or_default();
        fingerprints::package_context(
            context,
            key,
            baseline.is_some(),
            &old_rows,
            review,
            &new_rows,
        );
        let baseline_path = old
            .map(|_| {
                old_paths
                    .as_ref()
                    .expect("old package has paths")
                    .path(key, budget)
            })
            .transpose()?;
        let candidate_path = new.map(|_| new_paths.path(key, budget)).transpose()?;
        let audit_present = new_rows
            .iter()
            .any(|row| row.audit_recommended_when_present());
        let rows = merge::rows(old_rows, new_rows, old.is_some(), budget)?;
        if baseline
            .zip(review)
            .is_some_and(|(old, new)| (old == new.policy()) != rows.is_empty())
        {
            return Err(PackagePolicyChangeError::IncompleteRowProjection {
                package: Box::new(key.clone()),
            });
        }
        budget.key(key)?;
        let source_changed =
            old.map(|value| value.resolution()) != new.map(|value| value.resolution());
        let source_association_changed = baseline_path != candidate_path
            || accepted.is_some_and(|old| {
                old.source().package_navigation(key) != source.package_navigation(key)
                    || old.source().package_dependency_projection(key)
                        != source.package_dependency_projection(key)
            });
        let audit_recommended = audit_present
            || rows.iter().any(|row| row.audit_recommended)
            || (accepted.is_some() && (source_changed || source_association_changed));
        packages.push(PackagePolicyPackageChange {
            key: key.clone(),
            baseline_resolution: old.map(|value| value.resolution().clone()),
            candidate_resolution: new.map(|value| value.resolution().clone()),
            baseline_path,
            candidate_path,
            source_changed,
            source_association_changed,
            audit_recommended,
            rows,
            fingerprint: PackagePolicyChangeFingerprint([0; 32]),
        });
        if old.is_some() {
            old_index += 1;
        }
        if new.is_some() {
            new_index += 1;
        }
    }
    Ok(packages)
}
