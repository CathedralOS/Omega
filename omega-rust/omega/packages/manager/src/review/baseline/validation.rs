//! Canonical graph and resource validation shared by capture and recovery.

use super::encoding::{ensure_bounded_string, replay_parent_binding, validate_package_key_bounds};
use super::{
    ReviewOnlyBaselineCapsule, ReviewOnlyBaselineError, ReviewOnlyBaselineLimits,
    ReviewOnlyBaselinePackage,
};
use crate::declarations::{AliasName, PackageKey};
use crate::resolution::graph::ResolvedPackageClosure;
use crate::review::candidate::ReviewOnlyCanonicalRow;
use crate::review::candidate::validation::validate_review_only_records;
use build_evaluation::{
    BuildFilesystemReplayRecordLimits, recover_review_only_build_filesystem_replay_record,
};
use package_evidence::encoding::PackageReviewCanonicalRowRecoveryLimits;
use package_evidence::record::PackageReviewCanonicalRowKind;
use package_source::ImmutableSourceResolution;
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
    ResolvedPackageClosure::new(graph.root().clone(), graph.root_role(), packages)
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

impl ReviewOnlyBaselineCapsule {
    pub(super) fn validate(
        &self,
        limits: ReviewOnlyBaselineLimits,
    ) -> Result<(), ReviewOnlyBaselineError> {
        if self.packages.is_empty() || self.packages.len() > limits.maximum_packages {
            return Err(ReviewOnlyBaselineError::new(
                "review baseline package count violates its bounds",
            ));
        }
        validate_review_only_records(&self.packages)
            .map_err(|_| ReviewOnlyBaselineError::new("invalid review baseline record set"))?;
        let mut dependencies = 0usize;
        let mut rows = 0usize;
        let mut row_recovery_bytes = 0usize;
        let mut replay_record_bytes = 0usize;
        for package in &self.packages {
            let node = self.graph.package(&package.key).ok_or_else(|| {
                ReviewOnlyBaselineError::new("review baseline graph/review mismatch")
            })?;
            if node.source().resolution() != &package.resolution {
                return Err(ReviewOnlyBaselineError::new(
                    "review baseline graph/review resolution mismatch",
                ));
            }
            ensure_bounded_string(
                &package.target,
                limits.maximum_target_bytes,
                "review baseline target violates its byte bounds",
            )?;
            validate_package_key_bounds(&package.key, limits.maximum_identity_bytes)?;
            dependencies = dependencies.saturating_add(node.dependencies().len());
            rows = rows.saturating_add(package.canonical_rows.len());
            for dependency in node.dependencies() {
                ensure_bounded_string(
                    dependency.alias().as_str(),
                    limits.maximum_identity_bytes,
                    "review baseline dependency alias violates its byte bounds",
                )?;
            }
            for row in &package.canonical_rows {
                let recovery_bytes = row.recovery_bytes().ok_or_else(|| {
                    ReviewOnlyBaselineError::new(
                        "review baseline contains a non-recoverable comparison row",
                    )
                })?;
                row_recovery_bytes = row_recovery_bytes
                    .checked_add(recovery_bytes.len())
                    .ok_or_else(|| {
                        ReviewOnlyBaselineError::new(
                            "review baseline row recovery byte count overflowed",
                        )
                    })?;
            }
            if package.filesystem_replay_record.is_some()
                && package.build_observation_commitment.is_none()
            {
                return Err(ReviewOnlyBaselineError::new(
                    "filesystem replay record has no parent build observation",
                ));
            }
            if let Some(replay) = &package.filesystem_replay_record {
                replay_record_bytes = replay_record_bytes
                    .checked_add(replay.canonical_bytes().len())
                    .ok_or_else(|| {
                        ReviewOnlyBaselineError::new("review baseline replay byte count overflowed")
                    })?;
                let recovered = recover_review_only_build_filesystem_replay_record(
                    replay.canonical_bytes(),
                    replay_record_limits(limits),
                )
                .map_err(|_| {
                    ReviewOnlyBaselineError::new("invalid compiler filesystem replay record")
                })?;
                if recovered.commitment() != replay.commitment() {
                    return Err(ReviewOnlyBaselineError::new(
                        "filesystem replay record commitment mismatch",
                    ));
                }
            }
            let expected_binding = match (
                package.build_observation_commitment,
                package.filesystem_replay_record.as_ref(),
            ) {
                (Some(parent), Some(replay)) => {
                    Some(replay_parent_binding(parent, replay.commitment()))
                }
                (None, None) | (Some(_), None) => None,
                (None, Some(_)) => {
                    return Err(ReviewOnlyBaselineError::new(
                        "filesystem replay record has no parent build observation",
                    ));
                }
            };
            if package.replay_record_parent_binding != expected_binding {
                return Err(ReviewOnlyBaselineError::new(
                    "filesystem replay record parent binding mismatch",
                ));
            }
            validate_rows(&package.canonical_rows)?;
        }
        if self.graph.packages().len() != self.packages.len()
            || dependencies > limits.maximum_dependencies
            || rows > limits.maximum_rows
            || row_recovery_bytes > limits.maximum_row_recovery_bytes
            || replay_record_bytes > limits.maximum_capsule_bytes
            || graph_depth(&self.graph) > limits.maximum_graph_depth
        {
            return Err(ReviewOnlyBaselineError::new(
                "review baseline closure violates its resource bounds",
            ));
        }
        Ok(())
    }
}
