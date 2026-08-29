//! Canonical graph and resource validation shared by capture and recovery.

use super::{ReviewOnlyBaselineError, ReviewOnlyBaselineLimits, ReviewOnlyBaselinePackage};
use crate::resolution::ResolvedPackageClosure;
use crate::review::records::ReviewOnlyCanonicalRow;
use omega_build_evaluation::BuildFilesystemReplayRecordLimits;
use omega_package_review::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRecoveryLimits,
};
use omega_package_source::{AliasName, ImmutableSourceResolution, PackageKey};
use std::collections::{BTreeMap, VecDeque};

pub(super) struct PendingPackage {
    pub(super) key: PackageKey,
    pub(super) resolution: ImmutableSourceResolution,
    pub(super) dependencies: Vec<(AliasName, usize)>,
    pub(super) review: ReviewOnlyBaselinePackage,
}

pub(super) fn canonical_graph(
    graph: &ResolvedPackageClosure,
) -> Result<ResolvedPackageClosure, ReviewOnlyBaselineError> {
    let mut packages = graph.packages().to_vec();
    packages.sort_by(|left, right| left.source().key().cmp(right.source().key()));
    ResolvedPackageClosure::new(graph.root().clone(), packages)
        .map_err(|_| ReviewOnlyBaselineError::new("source closure cannot be canonicalized"))
}

pub(super) fn validate_rows(
    rows: &[ReviewOnlyCanonicalRow],
) -> Result<(), ReviewOnlyBaselineError> {
    if rows.is_empty() {
        return Err(ReviewOnlyBaselineError::new(
            "review baseline package has no canonical rows",
        ));
    }
    for pair in rows.windows(2) {
        if (pair[0].kind(), pair[0].key_bytes()) >= (pair[1].kind(), pair[1].key_bytes()) {
            return Err(ReviewOnlyBaselineError::new(
                "review baseline rows are not in strict canonical order",
            ));
        }
    }
    if rows
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::ProjectionHeader)
        .count()
        != 1
        || rows
            .iter()
            .filter(|row| row.kind() == PackageReviewCanonicalRowKind::SelectedProviderSet)
            .count()
            != 1
    {
        return Err(ReviewOnlyBaselineError::new(
            "review baseline is missing a singleton compiler row",
        ));
    }
    Ok(())
}

pub(super) fn graph_depth(graph: &ResolvedPackageClosure) -> usize {
    let mut incoming = graph
        .packages()
        .iter()
        .map(|package| (package.source().key().clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    for package in graph.packages() {
        for dependency in package.dependencies() {
            if let Some(count) = incoming.get_mut(dependency.target()) {
                *count = count.saturating_add(1);
            }
        }
    }
    let mut pending = incoming
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(key, _)| key.clone())
        .collect::<VecDeque<_>>();
    let mut depths = BTreeMap::from([(graph.root().clone(), 1usize)]);
    let mut maximum = 1usize;
    while let Some(key) = pending.pop_front() {
        let depth = depths.get(&key).copied().unwrap_or(1);
        let Some(node) = graph.package(&key) else {
            continue;
        };
        for dependency in node.dependencies() {
            let dependency_depth = depth.saturating_add(1);
            depths
                .entry(dependency.target().clone())
                .and_modify(|known| *known = (*known).max(dependency_depth))
                .or_insert(dependency_depth);
            maximum = maximum.max(dependency_depth);
            if let Some(count) = incoming.get_mut(dependency.target()) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    pending.push_back(dependency.target().clone());
                }
            }
        }
    }
    maximum
}

pub(super) fn row_limits(
    limits: ReviewOnlyBaselineLimits,
) -> PackageReviewCanonicalRowRecoveryLimits {
    PackageReviewCanonicalRowRecoveryLimits::new(
        limits.maximum_row_recovery_bytes,
        4 * 1024 * 1024,
        limits.maximum_target_bytes,
        1024 * 1024,
        4 * 1024 * 1024,
        131_072,
        1024 * 1024,
        8 * 1024 * 1024,
        16,
    )
}

pub(super) fn replay_record_limits(
    limits: ReviewOnlyBaselineLimits,
) -> BuildFilesystemReplayRecordLimits {
    BuildFilesystemReplayRecordLimits::new(limits.maximum_capsule_bytes, 4_096)
}
