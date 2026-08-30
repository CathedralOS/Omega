//! Strict recovery and immediate reconstruction of a baseline capsule.

use super::{ReviewOnlyBaselineCapsule, ReviewOnlyBaselinePackage};
use crate::declarations::{AliasName, BuildDeclarationKind};
use crate::resolution::graph::{
    ResolvedDependency, ResolvedPackageClosure, ResolvedPackageNode, ResolvedSourceIdentity,
};
use crate::review::baseline::encoding::{
    Decoder, capsule_checksum, clone_baseline_bytes, decode_package_key,
    decode_replay_record_option, decode_resolution, replay_parent_binding,
};
use crate::review::baseline::validation::{PendingPackage, row_limits, validate_rows};
use crate::review::baseline::{
    CHECKSUM_BYTES, MAGIC, REVIEW_ONLY_ARTIFACT_CLASS, ReviewOnlyBaselineError,
    ReviewOnlyBaselineLimits, VERSION,
};
use crate::review::candidate::{ReviewOnlyCanonicalRow, ReviewOnlySourceConsumptionCommitment};
use omega_package_evidence::encoding::decode_package_review_canonical_row_with_limits;

impl ReviewOnlyBaselineCapsule {
    pub fn decode(
        bytes: &[u8],
        limits: ReviewOnlyBaselineLimits,
    ) -> Result<Self, ReviewOnlyBaselineError> {
        if bytes.len() > limits.maximum_capsule_bytes || bytes.len() < CHECKSUM_BYTES {
            return Err(ReviewOnlyBaselineError::new(
                "review baseline capsule violates its byte bounds",
            ));
        }
        let prefix_length = bytes.len() - CHECKSUM_BYTES;
        let (prefix, checksum) = bytes.split_at(prefix_length);
        if capsule_checksum(prefix) != checksum {
            return Err(ReviewOnlyBaselineError::new(
                "review baseline capsule checksum mismatch",
            ));
        }

        let mut decoder = Decoder::new(prefix);
        decoder.fixed(MAGIC)?;
        if decoder.u16()? != VERSION
            || decoder.byte()? != REVIEW_ONLY_ARTIFACT_CLASS
            || decoder.byte()? != 0
        {
            return Err(ReviewOnlyBaselineError::new(
                "unsupported review baseline capsule header",
            ));
        }
        let target = decoder.string(limits.maximum_target_bytes)?.to_owned();
        if target.is_empty() {
            return Err(ReviewOnlyBaselineError::new(
                "review baseline target must not be empty",
            ));
        }
        let package_count = decoder.usize()?;
        if package_count == 0 || package_count > limits.maximum_packages {
            return Err(ReviewOnlyBaselineError::new(
                "review baseline package count violates its bounds",
            ));
        }
        let root_index = decoder.u32()? as usize;
        if root_index >= package_count {
            return Err(ReviewOnlyBaselineError::new(
                "review baseline root index is out of range",
            ));
        }
        let root_role = decode_root_role(&mut decoder)?;

        let mut pending = Vec::new();
        pending
            .try_reserve_exact(package_count)
            .map_err(|_| ReviewOnlyBaselineError::new("baseline package allocation failed"))?;
        let mut total_dependencies = 0usize;
        let mut total_rows = 0usize;
        let mut total_row_bytes = 0usize;
        for _ in 0..package_count {
            let record = decoder.bytes(limits.maximum_capsule_bytes)?;
            let mut record = Decoder::new(record);
            let key = decode_package_key(&mut record, limits.maximum_identity_bytes)?;
            let resolution = decode_resolution(&mut record)?;
            ResolvedSourceIdentity::new(key.clone(), resolution.clone()).map_err(|_| {
                ReviewOnlyBaselineError::new("baseline resolution does not match source lineage")
            })?;
            let source_consumption_commitment =
                ReviewOnlySourceConsumptionCommitment::from_recovered_digest(record.array_32()?);
            let whole_review_commitment = record.array_32()?;
            let build_observation_commitment = match record.byte()? {
                0 => None,
                1 => Some(record.array_32()?),
                _ => {
                    return Err(ReviewOnlyBaselineError::new(
                        "invalid build-observation option tag",
                    ));
                }
            };
            let source_input_replay_record = decode_replay_record_option(&mut record, limits)?;
            let replay_record_parent_binding = match (
                build_observation_commitment,
                source_input_replay_record.as_ref(),
            ) {
                (Some(parent), Some(replay)) => {
                    let recovered = record.array_32()?;
                    if recovered != replay_parent_binding(parent, replay.commitment()) {
                        return Err(ReviewOnlyBaselineError::new(
                            "filesystem replay record parent binding mismatch",
                        ));
                    }
                    Some(recovered)
                }
                (None, None) | (Some(_), None) => None,
                (None, Some(_)) => {
                    return Err(ReviewOnlyBaselineError::new(
                        "filesystem replay record has no parent build observation",
                    ));
                }
            };
            let dependency_count = record.usize()?;
            total_dependencies = total_dependencies.saturating_add(dependency_count);
            if total_dependencies > limits.maximum_dependencies {
                return Err(ReviewOnlyBaselineError::new(
                    "review baseline dependency count exceeds its ceiling",
                ));
            }
            let mut dependencies = Vec::new();
            dependencies
                .try_reserve_exact(dependency_count)
                .map_err(|_| {
                    ReviewOnlyBaselineError::new("baseline dependency allocation failed")
                })?;
            for _ in 0..dependency_count {
                dependencies.push((
                    AliasName::parse(record.string(limits.maximum_identity_bytes)?.to_owned())
                        .map_err(|_| ReviewOnlyBaselineError::new("invalid dependency alias"))?,
                    record.u32()? as usize,
                ));
            }
            let row_count = record.usize()?;
            total_rows = total_rows.saturating_add(row_count);
            if total_rows > limits.maximum_rows {
                return Err(ReviewOnlyBaselineError::new(
                    "review baseline row count exceeds its ceiling",
                ));
            }
            let mut rows = Vec::new();
            rows.try_reserve_exact(row_count)
                .map_err(|_| ReviewOnlyBaselineError::new("baseline row allocation failed"))?;
            for _ in 0..row_count {
                let row_bytes = record.bytes(limits.maximum_row_recovery_bytes)?;
                total_row_bytes = total_row_bytes.saturating_add(row_bytes.len());
                if total_row_bytes > limits.maximum_row_recovery_bytes {
                    return Err(ReviewOnlyBaselineError::new(
                        "review baseline row bytes exceed their aggregate ceiling",
                    ));
                }
                let recovery_bytes = clone_baseline_bytes(
                    row_bytes,
                    "review baseline row recovery allocation failed",
                )?;
                let decoded = decode_package_review_canonical_row_with_limits(
                    &recovery_bytes,
                    row_limits(limits),
                )
                .map_err(|_| ReviewOnlyBaselineError::new("invalid compiler review row"))?;
                if decoded.package() != key.identity() || decoded.target().target_name() != target {
                    return Err(ReviewOnlyBaselineError::new(
                        "review baseline row package or target mismatch",
                    ));
                }
                rows.push(ReviewOnlyCanonicalRow::from_recovered(
                    &decoded,
                    recovery_bytes,
                ));
            }
            record.finish()?;
            validate_rows(&rows)?;
            let review_key = key.clone();
            let review_resolution = resolution.clone();
            pending.push(PendingPackage {
                key,
                resolution,
                dependencies,
                review: ReviewOnlyBaselinePackage {
                    key: review_key,
                    resolution: review_resolution,
                    target: target.clone(),
                    source_consumption_commitment,
                    build_observation_commitment,
                    source_input_replay_record,
                    replay_record_parent_binding,
                    whole_review_commitment,
                    canonical_rows: rows,
                },
            });
        }
        decoder.finish()?;

        for pair in pending.windows(2) {
            if pair[0].key >= pair[1].key {
                return Err(ReviewOnlyBaselineError::new(
                    "review baseline packages are not in canonical order",
                ));
            }
        }
        let keys = pending
            .iter()
            .map(|package| package.key.clone())
            .collect::<Vec<_>>();
        let mut nodes = Vec::new();
        let mut packages = Vec::new();
        nodes
            .try_reserve_exact(package_count)
            .map_err(|_| ReviewOnlyBaselineError::new("baseline graph allocation failed"))?;
        packages
            .try_reserve_exact(package_count)
            .map_err(|_| ReviewOnlyBaselineError::new("baseline package allocation failed"))?;
        for package in pending {
            let mut dependencies = Vec::new();
            dependencies
                .try_reserve_exact(package.dependencies.len())
                .map_err(|_| ReviewOnlyBaselineError::new("baseline graph allocation failed"))?;
            for (alias, target_index) in package.dependencies {
                let target_key = keys.get(target_index).ok_or_else(|| {
                    ReviewOnlyBaselineError::new("baseline dependency index is out of range")
                })?;
                dependencies.push(ResolvedDependency::new(alias, target_key.clone()));
            }
            let source =
                ResolvedSourceIdentity::new(package.key.clone(), package.resolution.clone())
                    .map_err(|_| {
                        ReviewOnlyBaselineError::new("invalid baseline source identity")
                    })?;
            nodes.push(ResolvedPackageNode::new(source, dependencies));
            packages.push(package.review);
        }
        let graph = ResolvedPackageClosure::new(keys[root_index].clone(), root_role, nodes)
            .map_err(|_| ReviewOnlyBaselineError::new("invalid review baseline source graph"))?;
        let capsule = Self { graph, packages };
        capsule.validate(limits)?;
        if capsule.encode(limits)?.as_slice() != bytes {
            return Err(ReviewOnlyBaselineError::new(
                "review baseline capsule is not canonically encoded",
            ));
        }
        Ok(capsule)
    }
}

fn decode_root_role(
    decoder: &mut Decoder<'_>,
) -> Result<BuildDeclarationKind, ReviewOnlyBaselineError> {
    match decoder.byte()? {
        0 => Ok(BuildDeclarationKind::Package),
        1 => Ok(BuildDeclarationKind::Application),
        2 => Err(ReviewOnlyBaselineError::new(
            "review baseline root cannot have workspace role",
        )),
        _ => Err(ReviewOnlyBaselineError::new(
            "invalid review baseline root-role tag",
        )),
    }
}
