//! In-memory baseline packages and restart-stable capsules.

use super::encoding::{
    Decoder, Encoder, capsule_checksum, clone_baseline_bytes, decode_package_key,
    decode_replay_record_option, decode_resolution, encode_package_key,
    encode_replay_record_option, encode_resolution, ensure_bounded_string, replay_parent_binding,
    validate_recovery_row,
};
use super::validation::{PendingPackage, canonical_graph, row_limits, validate_rows};
use super::{
    CHECKSUM_BYTES, MAGIC, REVIEW_ONLY_ARTIFACT_CLASS, ReviewOnlyBaselineError,
    ReviewOnlyBaselineLimits, VERSION,
};
use crate::declarations::BuildDeclarationKind;
use crate::graph::{
    ResolvedDependency, ResolvedPackageClosure, ResolvedPackageNode, ResolvedPackageSourceClosure,
    ResolvedSourceIdentity,
};
use crate::identity::{AliasName, PackageKey};
use crate::review::CompilerIssuedPackageReviewSet;
use crate::review::records::validation::validate_review_only_closure;
use crate::review::records::{
    PackageReviewEvidence, ReviewOnlyCanonicalRow, ReviewOnlySourceConsumptionCommitment,
    build_observation_commitment, whole_review_commitment,
};
use omega_build_evaluation::{
    BuildFilesystemReplayRecordLimits, ReviewOnlyBuildFilesystemReplayRecord,
    capture_verified_build_filesystem_replay_record,
};
use omega_package_review::encoding::{
    decode_package_review_canonical_row_with_limits,
    encode_package_review_canonical_row_with_limits,
};
use omega_package_source::ImmutableSourceResolution;
use std::collections::BTreeMap;

/// One package's exact comparison evidence recovered from a review-only
/// baseline capsule.
#[derive(Debug, Clone)]
pub struct ReviewOnlyBaselinePackage {
    pub(super) key: PackageKey,
    pub(super) resolution: ImmutableSourceResolution,
    pub(super) target: String,
    source_consumption_commitment: ReviewOnlySourceConsumptionCommitment,
    pub(super) build_observation_commitment: Option<[u8; 32]>,
    pub(super) source_input_replay_record: Option<ReviewOnlyBuildFilesystemReplayRecord>,
    pub(super) replay_record_parent_binding: Option<[u8; 32]>,
    whole_review_commitment: [u8; 32],
    pub(super) canonical_rows: Vec<ReviewOnlyCanonicalRow>,
}

impl ReviewOnlyBaselinePackage {
    pub fn key(&self) -> &PackageKey {
        &self.key
    }

    pub fn resolution(&self) -> &ImmutableSourceResolution {
        &self.resolution
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub const fn source_consumption_commitment(&self) -> ReviewOnlySourceConsumptionCommitment {
        self.source_consumption_commitment
    }

    pub const fn build_observation_commitment(&self) -> Option<[u8; 32]> {
        self.build_observation_commitment
    }

    pub const fn source_input_replay_record(
        &self,
    ) -> Option<&ReviewOnlyBuildFilesystemReplayRecord> {
        self.source_input_replay_record.as_ref()
    }

    pub const fn whole_review_commitment(&self) -> [u8; 32] {
        self.whole_review_commitment
    }

    pub fn canonical_rows(&self) -> &[ReviewOnlyCanonicalRow] {
        &self.canonical_rows
    }
}

impl PackageReviewEvidence for ReviewOnlyBaselinePackage {
    fn key(&self) -> &PackageKey {
        &self.key
    }

    fn resolution(&self) -> &ImmutableSourceResolution {
        &self.resolution
    }

    fn projection_identity_matches(&self) -> bool {
        true
    }

    fn target_name(&self) -> &str {
        &self.target
    }

    fn source_consumption_commitment(&self) -> ReviewOnlySourceConsumptionCommitment {
        self.source_consumption_commitment
    }

    fn build_observation_commitment(&self) -> Option<[u8; 32]> {
        self.build_observation_commitment
    }

    fn whole_review_commitment(&self) -> [u8; 32] {
        self.whole_review_commitment
    }

    fn canonical_rows(&self) -> &[ReviewOnlyCanonicalRow] {
        &self.canonical_rows
    }
}

/// A restart-stable source graph and normalized review baseline.
///
/// This is intentionally review-only. It cannot construct `PackageInstance`,
/// authorize a conflict, mutate a project, or stand in for `omega.lock`.
#[derive(Debug, Clone)]
pub struct ReviewOnlyBaselineCapsule {
    pub(super) graph: ResolvedPackageClosure,
    pub(super) packages: Vec<ReviewOnlyBaselinePackage>,
}

impl ReviewOnlyBaselineCapsule {
    pub fn capture(
        sources: &ResolvedPackageSourceClosure,
        reviews: &CompilerIssuedPackageReviewSet,
        limits: ReviewOnlyBaselineLimits,
    ) -> Result<Self, ReviewOnlyBaselineError> {
        let validated = validate_review_only_closure(sources, reviews).map_err(|_| {
            ReviewOnlyBaselineError::new("cannot capture an invalid review-only source closure")
        })?;
        let mut packages = Vec::new();
        packages
            .try_reserve_exact(reviews.reviews().len())
            .map_err(|_| ReviewOnlyBaselineError::new("baseline package allocation failed"))?;
        let row_limits = row_limits(limits);
        let mut replay_record_bytes = 0usize;
        for review in validated.into_reviews_by_key() {
            let mut rows = Vec::new();
            rows.try_reserve_exact(review.canonical_rows().len())
                .map_err(|_| ReviewOnlyBaselineError::new("baseline row allocation failed"))?;
            for row in review.canonical_rows() {
                let encoded = encode_package_review_canonical_row_with_limits(row, row_limits)
                    .map_err(|_| {
                        ReviewOnlyBaselineError::new("compiler row cannot enter review baseline")
                    })?;
                let decoded = decode_package_review_canonical_row_with_limits(&encoded, row_limits)
                    .map_err(|_| {
                        ReviewOnlyBaselineError::new("compiler row recovery self-check failed")
                    })?;
                if decoded.package() != review.key().identity()
                    || decoded.target().target_name() != review.projection().target().target_name()
                {
                    return Err(ReviewOnlyBaselineError::new(
                        "compiler row package or target disagrees with its review",
                    ));
                }
                rows.push(ReviewOnlyCanonicalRow::from_recovered(&decoded, encoded));
            }
            let build_observation_commitment = review
                .build_observation_summary()
                .map(build_observation_commitment);
            let remaining_replay_bytes = limits
                .maximum_capsule_bytes
                .checked_sub(replay_record_bytes)
                .ok_or_else(|| {
                    ReviewOnlyBaselineError::new(
                        "review baseline replay records exceed their aggregate ceiling",
                    )
                })?;
            let source_input_replay_record = review
                .build_observation_summary()
                .map(|summary| {
                    capture_verified_build_filesystem_replay_record(
                        summary,
                        BuildFilesystemReplayRecordLimits::new(remaining_replay_bytes, 4_096),
                    )
                    .map_err(|_| {
                        ReviewOnlyBaselineError::new(
                            "compiler replay record cannot enter review baseline",
                        )
                    })
                })
                .transpose()?
                .flatten();
            if let Some(record) = &source_input_replay_record {
                replay_record_bytes = replay_record_bytes
                    .checked_add(record.canonical_bytes().len())
                    .filter(|bytes| *bytes <= limits.maximum_capsule_bytes)
                    .ok_or_else(|| {
                        ReviewOnlyBaselineError::new(
                            "review baseline replay records exceed their aggregate ceiling",
                        )
                    })?;
            }
            let replay_record_parent_binding = match (
                build_observation_commitment,
                source_input_replay_record.as_ref(),
            ) {
                (Some(parent), Some(record)) => {
                    Some(replay_parent_binding(parent, record.commitment()))
                }
                (None, None) | (Some(_), None) => None,
                (None, Some(_)) => {
                    return Err(ReviewOnlyBaselineError::new(
                        "filesystem replay record has no parent build observation",
                    ));
                }
            };
            packages.push(ReviewOnlyBaselinePackage {
                key: review.key().clone(),
                resolution: review.resolution().clone(),
                target: review.projection().target().target_name().to_owned(),
                source_consumption_commitment: review.source_consumption_commitment().into(),
                build_observation_commitment,
                source_input_replay_record,
                replay_record_parent_binding,
                whole_review_commitment: whole_review_commitment(review.canonical_review_bytes()),
                canonical_rows: rows,
            });
        }
        let graph = canonical_graph(sources.graph())?;
        let capsule = Self { graph, packages };
        capsule.validate(limits)?;
        let _ = capsule.encode(limits)?;
        Ok(capsule)
    }

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

    pub fn encode(
        &self,
        limits: ReviewOnlyBaselineLimits,
    ) -> Result<Vec<u8>, ReviewOnlyBaselineError> {
        self.validate(limits)?;
        let mut encoder = Encoder::bounded(
            limits
                .maximum_capsule_bytes
                .checked_sub(CHECKSUM_BYTES)
                .ok_or_else(|| ReviewOnlyBaselineError::new("capsule ceiling is too small"))?,
        );
        encoder.fixed(MAGIC);
        encoder.u16(VERSION);
        encoder.byte(REVIEW_ONLY_ARTIFACT_CLASS);
        encoder.byte(0);
        let first = self
            .packages
            .first()
            .ok_or_else(|| ReviewOnlyBaselineError::new("review baseline is empty"))?;
        ensure_bounded_string(
            &first.target,
            limits.maximum_target_bytes,
            "review baseline target violates its byte bounds",
        )?;
        encoder.string(&first.target)?;
        encoder.usize(self.packages.len())?;
        let indices = self
            .packages
            .iter()
            .enumerate()
            .map(|(index, package)| (package.key.clone(), index))
            .collect::<BTreeMap<_, _>>();
        encoder.u32(
            u32::try_from(*indices.get(self.graph.root()).ok_or_else(|| {
                ReviewOnlyBaselineError::new("review baseline root has no package record")
            })?)
            .map_err(|_| ReviewOnlyBaselineError::new("baseline root index exceeds u32"))?,
        );
        encode_root_role(&mut encoder, self.graph.root_role());
        for package in &self.packages {
            let node = self.graph.package(&package.key).ok_or_else(|| {
                ReviewOnlyBaselineError::new("review baseline package has no graph node")
            })?;
            let mut record = Encoder::bounded(limits.maximum_capsule_bytes);
            encode_package_key(&mut record, &package.key, limits.maximum_identity_bytes)?;
            encode_resolution(&mut record, &package.resolution)?;
            record.fixed(&package.source_consumption_commitment.digest());
            record.fixed(&package.whole_review_commitment);
            match package.build_observation_commitment {
                None => record.byte(0),
                Some(commitment) => {
                    record.byte(1);
                    record.fixed(&commitment);
                }
            }
            encode_replay_record_option(&mut record, package.source_input_replay_record.as_ref())?;
            if let Some(binding) = package.replay_record_parent_binding {
                record.fixed(&binding);
            }
            record.usize(node.dependencies().len())?;
            for dependency in node.dependencies() {
                ensure_bounded_string(
                    dependency.alias().as_str(),
                    limits.maximum_identity_bytes,
                    "review baseline dependency alias violates its byte bounds",
                )?;
                record.string(dependency.alias().as_str())?;
                record.u32(
                    u32::try_from(*indices.get(dependency.target()).ok_or_else(|| {
                        ReviewOnlyBaselineError::new(
                            "review baseline dependency has no package record",
                        )
                    })?)
                    .map_err(|_| {
                        ReviewOnlyBaselineError::new("baseline dependency index exceeds u32")
                    })?,
                );
            }
            record.usize(package.canonical_rows.len())?;
            for row in &package.canonical_rows {
                let recovery_bytes =
                    validate_recovery_row(row, &package.key, &package.target, row_limits(limits))?;
                record.bytes(recovery_bytes)?;
            }
            encoder.bytes(&record.finish()?)?;
        }
        let mut bytes = encoder.finish()?;
        let checksum = capsule_checksum(&bytes);
        bytes
            .try_reserve_exact(CHECKSUM_BYTES)
            .map_err(|_| ReviewOnlyBaselineError::new("capsule checksum allocation failed"))?;
        bytes.extend_from_slice(&checksum);
        Ok(bytes)
    }

    pub fn graph(&self) -> &ResolvedPackageClosure {
        &self.graph
    }

    pub fn packages(&self) -> &[ReviewOnlyBaselinePackage] {
        &self.packages
    }
}

fn encode_root_role(encoder: &mut Encoder, role: BuildDeclarationKind) {
    encoder.byte(match role {
        BuildDeclarationKind::Package => 0,
        BuildDeclarationKind::Application => 1,
        BuildDeclarationKind::Workspace => 2,
    });
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
