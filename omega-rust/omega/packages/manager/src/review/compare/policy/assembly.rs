use super::{
    PackagePolicyChangeError, PackagePolicyChangeFingerprint, PackagePolicyPackageChange,
    fingerprints, limits::Budget, merge, paths, projection,
};
use crate::declarations::DependencyPurpose;
use crate::lock::{PackageLockTarget, PackageOccurrenceRoster};
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
    // The recorded baseline and the candidate each join to their own source
    // graph's occurrence roster; a package's authorized purposes can change
    // without any consent row moving.
    let old_roster = accepted
        .map(|old| PackageOccurrenceRoster::derive(old.source()))
        .transpose()?;
    let new_roster = PackageOccurrenceRoster::derive(source)?;
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
    let (mut old_occurrence_index, mut new_occurrence_index) = (0, 0);
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
        let baseline_occurrence_purposes = old.and_then(|_| {
            old_roster
                .as_ref()
                .expect("old package implies baseline roster")
                .purposes(key)
        });
        let candidate_occurrence_purposes = new.and_then(|_| new_roster.purposes(key));
        let old_count = baseline_occurrence_purposes.map_or(0, <[_]>::len);
        let new_count = candidate_occurrence_purposes.map_or(0, <[_]>::len);
        let baselines = accepted.map_or(&[][..], |accepted| {
            &accepted.occurrences()[old_occurrence_index..old_occurrence_index + old_count]
        });
        let candidate_reviews = &reviews[new_occurrence_index..new_occurrence_index + new_count];
        old_occurrence_index += old_count;
        new_occurrence_index += new_count;
        budget.slots::<crate::lock::PackageCheckedContext>(old_count + new_count)?;
        let mut baseline_occurrence_contexts = Vec::new();
        baseline_occurrence_contexts
            .try_reserve_exact(old_count)
            .map_err(|_| PackagePolicyChangeError::AllocationFailed)?;
        baseline_occurrence_contexts.extend(baselines.iter().map(|value| value.context()));
        let mut candidate_occurrence_contexts = Vec::new();
        candidate_occurrence_contexts
            .try_reserve_exact(new_count)
            .map_err(|_| PackagePolicyChangeError::AllocationFailed)?;
        candidate_occurrence_contexts.extend(
            candidate_reviews
                .iter()
                .map(|value| value.checked_context()),
        );
        let occurrence_contexts_changed =
            baseline_occurrence_contexts != candidate_occurrence_contexts;
        let mut rows = Vec::new();
        let mut audit_present = false;
        // At most two roles per package. Keep a single package audit summary,
        // but compare each independently checked occurrence; adding a build
        // role must neither reuse nor invalidate the product role's consent.
        for purpose in DependencyPurpose::ALL {
            let baseline = baselines
                .iter()
                .find(|value| value.context().purpose() == purpose);
            let review = candidate_reviews
                .iter()
                .copied()
                .find(|value| value.checked_context().purpose() == purpose);
            if baseline.is_none() && review.is_none() {
                continue;
            }
            let retained = baseline.map_or(&[][..], |value| value.acceptance().rows());
            budget.slots::<crate::lock::PackageAcceptanceRow>(retained.len())?;
            for row in retained {
                budget.context(row.canonical_text().len())?;
            }
            let old_rows = retained.to_vec();
            let new_rows = review
                .map(|value| projection::rows(key, value.policy(), budget))
                .transpose()?
                .unwrap_or_default();
            let baseline_context = baseline.map(|value| value.context());
            let candidate_context = review.map(CompilerIssuedPackageReview::checked_context);
            fingerprints::package_context(
                context,
                key,
                baseline_context,
                &old_rows,
                review,
                &new_rows,
            );
            audit_present |= !new_rows.is_empty()
                || review.is_some_and(|review| {
                    !review.policy().slack_uses().is_empty()
                        || !review.policy().representation().demands().is_empty()
                });
            merge::append_rows(
                old_rows,
                new_rows,
                old.is_some(),
                baseline_context,
                candidate_context,
                budget,
                &mut rows,
            )?;
        }
        let baseline_path = old
            .map(|_| {
                old_paths
                    .as_ref()
                    .expect("old package has paths")
                    .path(key, budget)
            })
            .transpose()?;
        let candidate_path = new.map(|_| new_paths.path(key, budget)).transpose()?;
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
            || (accepted.is_some()
                && (source_changed || source_association_changed || occurrence_contexts_changed));
        packages.push(PackagePolicyPackageChange {
            key: key.clone(),
            baseline_resolution: old.map(|value| value.resolution().clone()),
            candidate_resolution: new.map(|value| value.resolution().clone()),
            baseline_path,
            candidate_path,
            restricted_build_requests: candidate_reviews
                .iter()
                .flat_map(|review| review.restricted_build_requests().iter().cloned())
                .collect(),
            baseline_occurrence_contexts,
            candidate_occurrence_contexts,
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
