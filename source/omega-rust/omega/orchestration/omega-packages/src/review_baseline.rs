use crate::capability_conflict::compare_review_only_capability_records;
use crate::record_file::{
    RecordFileError, RecordFileLimits, RecordFileRoot, is_portable_record_file_name,
};
use crate::review_closure::{validate_review_only_closure, validate_review_only_records};
use crate::review_evidence::{
    PackageReviewEvidence, ReviewOnlyCanonicalRow, ReviewOnlyCompilerExecutableCommitment,
    ReviewOnlySourceConsumptionCommitment, build_observation_commitment, whole_review_commitment,
};
use crate::source_review::assemble_update_source_review_records;
use crate::source_triage::triage_review_update_records;
use crate::{
    AliasName, CompilerIssuedPackageReviewSet, CompilerReviewTriage, ExternalLocalLineage,
    ExternalSourceContext, GitCommitId, GitTransport, GitTreeId, ImmutableSourceResolution,
    PackageKey, PackageName, PackageSourceCustody, PackageSourceReviewError,
    PackageSourceReviewInput, PackageSourceReviewLimits, ResolvedDependency,
    ResolvedPackageClosure, ResolvedPackageNode, ResolvedPackageSourceClosure,
    ResolvedSourceIdentity, ReviewOnlyCapabilityConflictError, ReviewOnlyCapabilityConflictLimits,
    ReviewOnlyCapabilityConflictSet, SourceContentDigest, SourceLineage, WorkspaceLineageIdentity,
    WorkspaceMemberLineage, WorkspaceMemberPath,
};
use omega_compiler::{
    BuildFilesystemReplayRecordLimits, ReviewOnlyBuildFilesystemReplayRecord,
    capture_verified_build_filesystem_replay_record,
    recover_review_only_build_filesystem_replay_record,
};
use omega_package_review::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRecoveryLimits,
    decode_package_review_canonical_row_with_limits,
    encode_package_review_canonical_row_with_limits,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};

const MAGIC: &[u8] = b"OMEGA-PACKAGE-REVIEW-BASELINE\0";
const CHECKSUM_DOMAIN: &[u8] = b"OMEGA-PACKAGE-REVIEW-BASELINE-CAPSULE\0";
const REPLAY_PARENT_BINDING_DOMAIN: &[u8] = b"OMEGA-PACKAGE-REVIEW-REPLAY-PARENT-BINDING\0";
const VERSION: u16 = 2;
const REVIEW_ONLY_ARTIFACT_CLASS: u8 = 0;
const CHECKSUM_BYTES: usize = 32;
const BASELINE_NAME_MAXIMUM_BYTES: usize = 255;

/// Resource ceilings for a restart-stable review baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewOnlyBaselineLimits {
    maximum_capsule_bytes: usize,
    maximum_packages: usize,
    maximum_dependencies: usize,
    maximum_graph_depth: usize,
    maximum_identity_bytes: usize,
    maximum_target_bytes: usize,
    maximum_rows: usize,
    maximum_row_recovery_bytes: usize,
}

impl ReviewOnlyBaselineLimits {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        maximum_capsule_bytes: usize,
        maximum_packages: usize,
        maximum_dependencies: usize,
        maximum_graph_depth: usize,
        maximum_identity_bytes: usize,
        maximum_target_bytes: usize,
        maximum_rows: usize,
        maximum_row_recovery_bytes: usize,
    ) -> Self {
        Self {
            maximum_capsule_bytes,
            maximum_packages,
            maximum_dependencies,
            maximum_graph_depth,
            maximum_identity_bytes,
            maximum_target_bytes,
            maximum_rows,
            maximum_row_recovery_bytes,
        }
    }

    pub const fn maximum_capsule_bytes(self) -> usize {
        self.maximum_capsule_bytes
    }

    pub const fn maximum_packages(self) -> usize {
        self.maximum_packages
    }

    pub const fn maximum_rows(self) -> usize {
        self.maximum_rows
    }
}

impl Default for ReviewOnlyBaselineLimits {
    fn default() -> Self {
        Self::new(
            64 * 1024 * 1024,
            1_024,
            16_384,
            128,
            4 * 1024,
            256,
            65_536,
            32 * 1024 * 1024,
        )
    }
}

/// A bounded baseline-codec failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewOnlyBaselineError {
    message: &'static str,
}

impl ReviewOnlyBaselineError {
    const fn new(message: &'static str) -> Self {
        Self { message }
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for ReviewOnlyBaselineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for ReviewOnlyBaselineError {}

/// One portable direct-child filename beneath an explicit project-owned
/// review-state directory capability. It is routing only and never enters the
/// capsule's semantic identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlyBaselineName(String);

impl ReviewOnlyBaselineName {
    pub fn parse(value: &str) -> Result<Self, ReviewOnlyBaselineNameError> {
        if !is_portable_record_file_name(value, BASELINE_NAME_MAXIMUM_BYTES) {
            return Err(ReviewOnlyBaselineNameError::InvalidName);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }
}

/// Closed rejection for a non-portable review-baseline leaf name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewOnlyBaselineNameError {
    InvalidName,
}

impl fmt::Display for ReviewOnlyBaselineNameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("review-baseline filename is not canonical and portable")
    }
}

impl std::error::Error for ReviewOnlyBaselineNameError {}

/// Explicit directory-capability root for review-only baseline state.
///
/// Trusted command orchestration supplies the project-owned directory. This
/// type never discovers storage from dependency source and cannot promote a
/// recovered capsule into accepted lock or package authority.
#[derive(Debug)]
pub struct ReviewOnlyBaselineDirectory {
    root: RecordFileRoot,
}

impl ReviewOnlyBaselineDirectory {
    /// Bind an already-open project-owned review-state directory.
    ///
    /// `display_path` is diagnostic text only. Every filesystem operation is
    /// performed relative to `directory`.
    pub fn from_capability(
        directory: cap_std::fs::Dir,
        display_path: impl Into<PathBuf>,
    ) -> Result<Self, ReviewOnlyBaselineFileError> {
        let root = RecordFileRoot::from_directory(directory, display_path.into())
            .map_err(map_baseline_file_error)?;
        Ok(Self { root })
    }

    /// Persist a complete capsule as a new immutable review-state file.
    /// Existing destinations are never overwritten.
    pub fn persist_new_capsule(
        &self,
        name: &ReviewOnlyBaselineName,
        capsule: &ReviewOnlyBaselineCapsule,
        limits: ReviewOnlyBaselineLimits,
    ) -> Result<(), ReviewOnlyBaselineFileError> {
        let bytes = capsule
            .encode(limits)
            .map_err(ReviewOnlyBaselineFileError::Capsule)?;
        self.root
            .write_new(
                name.as_path(),
                &bytes,
                RecordFileLimits {
                    maximum_bytes: limits.maximum_capsule_bytes(),
                },
            )
            .map_err(map_baseline_file_error)
    }

    /// Recover one capsule through the retained file handle, then recheck the
    /// exact bytes and direct-child pathname before returning it.
    pub fn recover_capsule(
        &self,
        name: &ReviewOnlyBaselineName,
        limits: ReviewOnlyBaselineLimits,
    ) -> Result<ReviewOnlyBaselineCapsule, ReviewOnlyBaselineFileError> {
        let record_limits = RecordFileLimits {
            maximum_bytes: limits.maximum_capsule_bytes(),
        };
        let mut read = self
            .root
            .read(name.as_path(), record_limits)
            .map_err(map_baseline_file_error)?;
        let capsule = ReviewOnlyBaselineCapsule::decode(read.bytes(), limits)
            .map_err(ReviewOnlyBaselineFileError::Capsule)?;
        read.verify_current(record_limits)
            .map_err(map_baseline_file_error)?;
        Ok(capsule)
    }
}

/// Closed filesystem and capsule-recovery failures for review-only baseline
/// custody. Attacker-controlled record bytes never enter these messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewOnlyBaselineFileError {
    Io { path: PathBuf, message: String },
    InvalidDestination { path: PathBuf },
    NotRegularFile { path: PathBuf },
    DestinationExists { path: PathBuf },
    DirectoryCustodyChanged { path: PathBuf },
    PublishedButUnconfirmed { path: PathBuf, message: String },
    ContentsChanged { path: PathBuf },
    ByteLimitExceeded { actual: u64, maximum: usize },
    LengthOverflow,
    AllocationFailed,
    StageNameSpaceExhausted { directory: PathBuf },
    Capsule(ReviewOnlyBaselineError),
}

impl fmt::Display for ReviewOnlyBaselineFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => write!(
                formatter,
                "review-baseline file `{}`: {message}",
                path.display()
            ),
            Self::InvalidDestination { path } => write!(
                formatter,
                "review-baseline destination `{}` is invalid",
                path.display()
            ),
            Self::NotRegularFile { path } => write!(
                formatter,
                "review-baseline path `{}` is not a regular confined file",
                path.display()
            ),
            Self::DestinationExists { path } => write!(
                formatter,
                "review-baseline destination `{}` already exists",
                path.display()
            ),
            Self::DirectoryCustodyChanged { path } => write!(
                formatter,
                "review-baseline directory custody changed at `{}`",
                path.display()
            ),
            Self::PublishedButUnconfirmed { path, message } => write!(
                formatter,
                "review-baseline destination `{}` was published but could not be confirmed: {message}",
                path.display()
            ),
            Self::ContentsChanged { path } => write!(
                formatter,
                "review-baseline file `{}` changed while it was being recovered",
                path.display()
            ),
            Self::ByteLimitExceeded { actual, maximum } => write!(
                formatter,
                "review-baseline file uses {actual} bytes; the limit is {maximum}"
            ),
            Self::LengthOverflow => formatter.write_str("review-baseline file length overflow"),
            Self::AllocationFailed => formatter.write_str("review-baseline file allocation failed"),
            Self::StageNameSpaceExhausted { directory } => write!(
                formatter,
                "review-baseline staging names are exhausted beneath `{}`",
                directory.display()
            ),
            Self::Capsule(error) => write!(formatter, "invalid review-baseline capsule: {error}"),
        }
    }
}

impl std::error::Error for ReviewOnlyBaselineFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Capsule(error) => Some(error),
            _ => None,
        }
    }
}

fn map_baseline_file_error(error: RecordFileError) -> ReviewOnlyBaselineFileError {
    match error {
        RecordFileError::Io { path, message } => ReviewOnlyBaselineFileError::Io { path, message },
        RecordFileError::InvalidDestination { path } => {
            ReviewOnlyBaselineFileError::InvalidDestination { path }
        }
        RecordFileError::NotRegularFile { path } => {
            ReviewOnlyBaselineFileError::NotRegularFile { path }
        }
        RecordFileError::DestinationExists { path } => {
            ReviewOnlyBaselineFileError::DestinationExists { path }
        }
        RecordFileError::ParentDirectoryChanged { path } => {
            ReviewOnlyBaselineFileError::DirectoryCustodyChanged { path }
        }
        RecordFileError::PublishedButUnconfirmed { path, message } => {
            ReviewOnlyBaselineFileError::PublishedButUnconfirmed { path, message }
        }
        RecordFileError::ContentsChanged { path } => {
            ReviewOnlyBaselineFileError::ContentsChanged { path }
        }
        RecordFileError::ByteLimitExceeded { actual, maximum } => {
            ReviewOnlyBaselineFileError::ByteLimitExceeded { actual, maximum }
        }
        RecordFileError::LengthOverflow => ReviewOnlyBaselineFileError::LengthOverflow,
        RecordFileError::AllocationFailed => ReviewOnlyBaselineFileError::AllocationFailed,
        RecordFileError::StageNameSpaceExhausted { directory } => {
            ReviewOnlyBaselineFileError::StageNameSpaceExhausted { directory }
        }
    }
}

/// One package's exact comparison evidence recovered from a review-only
/// baseline capsule.
#[derive(Debug, Clone)]
pub struct ReviewOnlyBaselinePackage {
    key: PackageKey,
    resolution: ImmutableSourceResolution,
    target: String,
    compiler_executable_commitment: ReviewOnlyCompilerExecutableCommitment,
    source_consumption_commitment: ReviewOnlySourceConsumptionCommitment,
    build_observation_commitment: Option<[u8; 32]>,
    source_input_replay_record: Option<ReviewOnlyBuildFilesystemReplayRecord>,
    replay_record_parent_binding: Option<[u8; 32]>,
    whole_review_commitment: [u8; 32],
    canonical_rows: Vec<ReviewOnlyCanonicalRow>,
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

    pub const fn compiler_executable_commitment(&self) -> ReviewOnlyCompilerExecutableCommitment {
        self.compiler_executable_commitment
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

    fn compiler_executable_commitment(&self) -> ReviewOnlyCompilerExecutableCommitment {
        self.compiler_executable_commitment
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
    graph: ResolvedPackageClosure,
    packages: Vec<ReviewOnlyBaselinePackage>,
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
                compiler_executable_commitment: review.compiler_executable_commitment().into(),
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
        let compiler =
            ReviewOnlyCompilerExecutableCommitment::from_recovered_digest(decoder.array_32()?);
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
                    compiler_executable_commitment: compiler,
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
        let graph = ResolvedPackageClosure::new(keys[root_index].clone(), nodes)
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
        encoder.fixed(&first.compiler_executable_commitment.digest());
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

    fn validate(&self, limits: ReviewOnlyBaselineLimits) -> Result<(), ReviewOnlyBaselineError> {
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
            if package.source_input_replay_record.is_some()
                && package.build_observation_commitment.is_none()
            {
                return Err(ReviewOnlyBaselineError::new(
                    "filesystem replay record has no parent build observation",
                ));
            }
            if let Some(replay) = &package.source_input_replay_record {
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
                package.source_input_replay_record.as_ref(),
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

/// Compare a reopened baseline with a live, resolver-bound candidate.
pub fn compare_review_only_capabilities_from_baseline(
    baseline: &ReviewOnlyBaselineCapsule,
    candidate: &CompilerIssuedPackageReviewSet,
    candidate_sources: &ResolvedPackageSourceClosure,
    limits: ReviewOnlyCapabilityConflictLimits,
) -> Result<ReviewOnlyCapabilityConflictSet, ReviewOnlyCapabilityConflictError> {
    compare_review_only_capability_records(
        baseline.packages(),
        candidate,
        candidate_sources,
        limits,
    )
}

pub fn triage_review_update_from_baseline(
    baseline: &ReviewOnlyBaselineCapsule,
    candidate: &CompilerIssuedPackageReviewSet,
    unavailable_baseline_sources: &BTreeSet<PackageKey>,
) -> CompilerReviewTriage {
    triage_review_update_records(baseline.packages(), candidate, unavailable_baseline_sources)
}

pub fn assemble_update_source_review_from_baseline(
    baseline: &ReviewOnlyBaselineCapsule,
    candidate: &CompilerIssuedPackageReviewSet,
    recovered_baseline_sources: &[PackageSourceCustody],
    candidate_sources: &ResolvedPackageSourceClosure,
    limits: PackageSourceReviewLimits,
) -> Result<PackageSourceReviewInput, PackageSourceReviewError> {
    assemble_update_source_review_records(
        baseline.packages(),
        candidate,
        recovered_baseline_sources,
        candidate_sources,
        limits,
    )
}

struct PendingPackage {
    key: PackageKey,
    resolution: ImmutableSourceResolution,
    dependencies: Vec<(AliasName, usize)>,
    review: ReviewOnlyBaselinePackage,
}

fn canonical_graph(
    graph: &ResolvedPackageClosure,
) -> Result<ResolvedPackageClosure, ReviewOnlyBaselineError> {
    let mut packages = graph.packages().to_vec();
    packages.sort_by(|left, right| left.source().key().cmp(right.source().key()));
    ResolvedPackageClosure::new(graph.root().clone(), packages)
        .map_err(|_| ReviewOnlyBaselineError::new("source closure cannot be canonicalized"))
}

fn validate_rows(rows: &[ReviewOnlyCanonicalRow]) -> Result<(), ReviewOnlyBaselineError> {
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

fn graph_depth(graph: &ResolvedPackageClosure) -> usize {
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

fn row_limits(limits: ReviewOnlyBaselineLimits) -> PackageReviewCanonicalRowRecoveryLimits {
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

fn replay_record_limits(limits: ReviewOnlyBaselineLimits) -> BuildFilesystemReplayRecordLimits {
    BuildFilesystemReplayRecordLimits::new(limits.maximum_capsule_bytes, 4_096)
}

fn replay_parent_binding(parent: [u8; 32], replay: [u8; 32]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(REPLAY_PARENT_BINDING_DOMAIN);
    digest.update(parent);
    digest.update(replay);
    digest.finalize().into()
}

fn encode_replay_record_option(
    encoder: &mut Encoder,
    replay: Option<&ReviewOnlyBuildFilesystemReplayRecord>,
) -> Result<(), ReviewOnlyBaselineError> {
    match replay {
        None => encoder.byte(0),
        Some(replay) => {
            encoder.byte(1);
            encoder.bytes(replay.canonical_bytes())?;
        }
    }
    Ok(())
}

fn decode_replay_record_option(
    decoder: &mut Decoder<'_>,
    limits: ReviewOnlyBaselineLimits,
) -> Result<Option<ReviewOnlyBuildFilesystemReplayRecord>, ReviewOnlyBaselineError> {
    match decoder.byte()? {
        0 => Ok(None),
        1 => recover_review_only_build_filesystem_replay_record(
            decoder.bytes(limits.maximum_capsule_bytes)?,
            replay_record_limits(limits),
        )
        .map(Some)
        .map_err(|_| ReviewOnlyBaselineError::new("invalid compiler filesystem replay record")),
        _ => Err(ReviewOnlyBaselineError::new(
            "invalid filesystem-replay-record option tag",
        )),
    }
}

fn capsule_checksum(prefix: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(CHECKSUM_DOMAIN);
    digest.update(
        u64::try_from(prefix.len())
            .expect("bounded capsule length fits u64")
            .to_le_bytes(),
    );
    digest.update(prefix);
    digest.finalize().into()
}

fn clone_baseline_bytes(
    bytes: &[u8],
    allocation_error: &'static str,
) -> Result<Vec<u8>, ReviewOnlyBaselineError> {
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(bytes.len())
        .map_err(|_| ReviewOnlyBaselineError::new(allocation_error))?;
    owned.extend_from_slice(bytes);
    Ok(owned)
}

fn ensure_bounded_string(
    value: &str,
    maximum_bytes: usize,
    error: &'static str,
) -> Result<(), ReviewOnlyBaselineError> {
    if value.is_empty() || value.len() > maximum_bytes {
        Err(ReviewOnlyBaselineError::new(error))
    } else {
        Ok(())
    }
}

fn validate_package_key_bounds(
    key: &PackageKey,
    maximum_identity_bytes: usize,
) -> Result<(), ReviewOnlyBaselineError> {
    let check = |value: &str| {
        ensure_bounded_string(
            value,
            maximum_identity_bytes,
            "review baseline package identity violates its byte bounds",
        )
    };
    check(key.name().as_str())?;
    match key.source_lineage() {
        SourceLineage::GitHub(lineage) => {
            check(lineage.owner())?;
            check(lineage.repository())?;
        }
        SourceLineage::GitLab(lineage) => check(lineage.repository_path())?,
        SourceLineage::Git(lineage) => {
            if let Some(user) = lineage.user() {
                check(user)?;
            }
            check(lineage.host())?;
            check(lineage.repository_path())?;
        }
        SourceLineage::Workspace(lineage) => check(lineage.member_path().as_str())?,
        SourceLineage::ExternalLocal(lineage) => {
            check(lineage.canonical_absolute_path().to_str().ok_or_else(|| {
                ReviewOnlyBaselineError::new("external source path is not UTF-8")
            })?)?
        }
    }
    Ok(())
}

fn validate_recovery_row<'a>(
    row: &'a ReviewOnlyCanonicalRow,
    key: &PackageKey,
    target: &str,
    limits: PackageReviewCanonicalRowRecoveryLimits,
) -> Result<&'a [u8], ReviewOnlyBaselineError> {
    let recovery_bytes = row.recovery_bytes().ok_or_else(|| {
        ReviewOnlyBaselineError::new("review baseline contains a non-recoverable comparison row")
    })?;
    let decoded = decode_package_review_canonical_row_with_limits(recovery_bytes, limits)
        .map_err(|_| ReviewOnlyBaselineError::new("invalid recovered compiler review row"))?;
    if decoded.package() != key.identity()
        || decoded.target().target_name() != target
        || decoded.kind() != row.kind()
        || decoded.risk() != row.risk()
        || decoded.key_bytes() != row.key_bytes()
        || decoded.canonical_bytes() != row.canonical_bytes()
        || decoded.source() != row.source()
    {
        return Err(ReviewOnlyBaselineError::new(
            "recovered compiler review row disagrees with review-only comparison metadata",
        ));
    }
    Ok(recovery_bytes)
}

fn encode_package_key(
    encoder: &mut Encoder,
    key: &PackageKey,
    maximum_identity_bytes: usize,
) -> Result<(), ReviewOnlyBaselineError> {
    validate_package_key_bounds(key, maximum_identity_bytes)?;
    encoder.string(key.name().as_str())?;
    match key.source_lineage() {
        SourceLineage::GitHub(lineage) => {
            encoder.byte(0);
            encoder.string(lineage.owner())?;
            encoder.string(lineage.repository())?;
        }
        SourceLineage::GitLab(lineage) => {
            encoder.byte(1);
            encoder.string(lineage.repository_path())?;
        }
        SourceLineage::Git(lineage) => {
            encoder.byte(2);
            encoder.byte(match lineage.transport() {
                GitTransport::Https => 0,
                GitTransport::SshUrl => 1,
                GitTransport::ScpLike => 2,
            });
            match lineage.user() {
                None => encoder.byte(0),
                Some(user) => {
                    encoder.byte(1);
                    encoder.string(user)?;
                }
            }
            encoder.string(lineage.host())?;
            match lineage.port() {
                None => encoder.byte(0),
                Some(port) => {
                    encoder.byte(1);
                    encoder.u16(port);
                }
            }
            encoder.string(lineage.repository_path())?;
        }
        SourceLineage::Workspace(lineage) => {
            encoder.byte(3);
            encoder.fixed(&decode_hex_32(&lineage.workspace_identity().to_hex())?);
            encoder.string(lineage.member_path().as_str())?;
        }
        SourceLineage::ExternalLocal(lineage) => {
            encoder.byte(4);
            encoder.fixed(&decode_hex_32(&lineage.source_context().to_hex())?);
            encoder.string(lineage.canonical_absolute_path().to_str().ok_or_else(|| {
                ReviewOnlyBaselineError::new("external source path is not UTF-8")
            })?)?;
        }
    }
    Ok(())
}

fn decode_package_key(
    decoder: &mut Decoder<'_>,
    maximum_identity_bytes: usize,
) -> Result<PackageKey, ReviewOnlyBaselineError> {
    let name = PackageName::parse(decoder.string(maximum_identity_bytes)?.to_owned())
        .map_err(|_| ReviewOnlyBaselineError::new("invalid package name in review baseline"))?;
    let lineage = match decoder.byte()? {
        0 => SourceLineage::git(&format!(
            "https://github.com/{}/{}.git",
            decoder.string(maximum_identity_bytes)?,
            decoder.string(maximum_identity_bytes)?
        )),
        1 => SourceLineage::git(&format!(
            "https://gitlab.com/{}.git",
            decoder.string(maximum_identity_bytes)?
        )),
        2 => {
            let transport = match decoder.byte()? {
                0 => GitTransport::Https,
                1 => GitTransport::SshUrl,
                2 => GitTransport::ScpLike,
                _ => return Err(ReviewOnlyBaselineError::new("invalid Git transport tag")),
            };
            let user = match decoder.byte()? {
                0 => None,
                1 => Some(decoder.string(maximum_identity_bytes)?.to_owned()),
                _ => return Err(ReviewOnlyBaselineError::new("invalid Git user option tag")),
            };
            let host = decoder.string(maximum_identity_bytes)?.to_owned();
            let port = match decoder.byte()? {
                0 => None,
                1 => Some(decoder.u16()?),
                _ => return Err(ReviewOnlyBaselineError::new("invalid Git port option tag")),
            };
            let path = decoder.string(maximum_identity_bytes)?.to_owned();
            let locator = generic_git_locator(transport, user.as_deref(), &host, port, &path);
            SourceLineage::git(&locator)
        }
        3 => {
            let workspace = WorkspaceLineageIdentity::parse_hex(&encode_hex(&decoder.array_32()?))
                .map_err(|_| ReviewOnlyBaselineError::new("invalid workspace identity"));
            let member = WorkspaceMemberPath::parse(decoder.string(maximum_identity_bytes)?)
                .map_err(|_| ReviewOnlyBaselineError::new("invalid workspace member path"));
            return Ok(PackageKey::new(
                name,
                SourceLineage::Workspace(WorkspaceMemberLineage::new(workspace?, member?)),
            ));
        }
        4 => {
            let context = ExternalSourceContext::parse_hex(&encode_hex(&decoder.array_32()?));
            let path = decoder.string(maximum_identity_bytes)?.to_owned();
            context
                .and_then(|context| {
                    ExternalLocalLineage::from_recovered_canonical_path(path, context)
                })
                .map(SourceLineage::ExternalLocal)
        }
        _ => return Err(ReviewOnlyBaselineError::new("invalid source-lineage tag")),
    }
    .map_err(|_| ReviewOnlyBaselineError::new("invalid source lineage in review baseline"))?;
    Ok(PackageKey::new(name, lineage))
}

fn generic_git_locator(
    transport: GitTransport,
    user: Option<&str>,
    host: &str,
    port: Option<u16>,
    path: &str,
) -> String {
    let user = user.map(|user| format!("{user}@")).unwrap_or_default();
    match transport {
        GitTransport::Https => format!(
            "https://{user}{host}{}/{path}",
            port.map(|port| format!(":{port}")).unwrap_or_default()
        ),
        GitTransport::SshUrl => format!(
            "ssh://{user}{host}{}/{path}",
            port.map(|port| format!(":{port}")).unwrap_or_default()
        ),
        GitTransport::ScpLike => format!("{user}{host}:{path}"),
    }
}

fn encode_resolution(
    encoder: &mut Encoder,
    resolution: &ImmutableSourceResolution,
) -> Result<(), ReviewOnlyBaselineError> {
    match resolution {
        ImmutableSourceResolution::Git {
            commit,
            tree,
            content,
        } => {
            encoder.byte(0);
            encoder.string(&commit.to_hex())?;
            encoder.string(&tree.to_hex())?;
            encoder.fixed(&decode_hex_32(&content.to_hex())?);
        }
        ImmutableSourceResolution::Workspace { content } => {
            encoder.byte(1);
            encoder.fixed(&decode_hex_32(&content.to_hex())?);
        }
        ImmutableSourceResolution::ExternalLocal { content } => {
            encoder.byte(2);
            encoder.fixed(&decode_hex_32(&content.to_hex())?);
        }
    }
    Ok(())
}

fn decode_resolution(
    decoder: &mut Decoder<'_>,
) -> Result<ImmutableSourceResolution, ReviewOnlyBaselineError> {
    let content = |decoder: &mut Decoder<'_>| {
        SourceContentDigest::parse_hex(&encode_hex(&decoder.array_32()?))
            .map_err(|_| ReviewOnlyBaselineError::new("invalid source content digest"))
    };
    match decoder.byte()? {
        0 => ImmutableSourceResolution::git(
            GitCommitId::parse_hex(decoder.string(64)?)
                .map_err(|_| ReviewOnlyBaselineError::new("invalid Git commit ID"))?,
            GitTreeId::parse_hex(decoder.string(64)?)
                .map_err(|_| ReviewOnlyBaselineError::new("invalid Git tree ID"))?,
            content(decoder)?,
        )
        .map_err(|_| ReviewOnlyBaselineError::new("invalid Git source resolution")),
        1 => Ok(ImmutableSourceResolution::workspace(content(decoder)?)),
        2 => Ok(ImmutableSourceResolution::external_local(content(decoder)?)),
        _ => Err(ReviewOnlyBaselineError::new(
            "invalid immutable-resolution tag",
        )),
    }
}

fn decode_hex_32(value: &str) -> Result<[u8; 32], ReviewOnlyBaselineError> {
    let bytes = decode_hex(value)
        .ok_or_else(|| ReviewOnlyBaselineError::new("invalid 32-byte hexadecimal value"))?;
    bytes
        .try_into()
        .map_err(|_| ReviewOnlyBaselineError::new("invalid 32-byte hexadecimal value"))
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = char::from(pair[0]).to_digit(16)?;
            let low = char::from(pair[1]).to_digit(16)?;
            Some(((high << 4) | low) as u8)
        })
        .collect()
}

fn encode_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

struct Encoder {
    bytes: Vec<u8>,
    maximum_bytes: usize,
    exceeded: bool,
}

impl Encoder {
    fn bounded(maximum_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum_bytes,
            exceeded: false,
        }
    }

    fn append(&mut self, bytes: &[u8]) {
        if self.exceeded
            || self
                .bytes
                .len()
                .checked_add(bytes.len())
                .is_none_or(|length| length > self.maximum_bytes)
        {
            self.exceeded = true;
            return;
        }
        if self.bytes.try_reserve(bytes.len()).is_err() {
            self.exceeded = true;
            return;
        }
        self.bytes.extend_from_slice(bytes);
    }

    fn fixed(&mut self, bytes: &[u8]) {
        self.append(bytes);
    }

    fn byte(&mut self, value: u8) {
        self.append(&[value]);
    }

    fn u16(&mut self, value: u16) {
        self.append(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.append(&value.to_le_bytes());
    }

    fn usize(&mut self, value: usize) -> Result<(), ReviewOnlyBaselineError> {
        self.append(
            &u64::try_from(value)
                .map_err(|_| ReviewOnlyBaselineError::new("baseline length exceeds u64"))?
                .to_le_bytes(),
        );
        Ok(())
    }

    fn bytes(&mut self, bytes: &[u8]) -> Result<(), ReviewOnlyBaselineError> {
        self.usize(bytes.len())?;
        self.append(bytes);
        Ok(())
    }

    fn string(&mut self, value: &str) -> Result<(), ReviewOnlyBaselineError> {
        self.bytes(value.as_bytes())
    }

    fn finish(self) -> Result<Vec<u8>, ReviewOnlyBaselineError> {
        if self.exceeded {
            Err(ReviewOnlyBaselineError::new(
                "review baseline encoding exceeds its byte ceiling",
            ))
        } else {
            Ok(self.bytes)
        }
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ReviewOnlyBaselineError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| ReviewOnlyBaselineError::new("baseline length overflow"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| ReviewOnlyBaselineError::new("truncated review baseline capsule"))?;
        self.offset = end;
        Ok(value)
    }

    fn fixed(&mut self, expected: &[u8]) -> Result<(), ReviewOnlyBaselineError> {
        if self.take(expected.len())? == expected {
            Ok(())
        } else {
            Err(ReviewOnlyBaselineError::new(
                "invalid review baseline capsule magic",
            ))
        }
    }

    fn byte(&mut self) -> Result<u8, ReviewOnlyBaselineError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, ReviewOnlyBaselineError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("exact u16 width"),
        ))
    }

    fn u32(&mut self) -> Result<u32, ReviewOnlyBaselineError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("exact u32 width"),
        ))
    }

    fn usize(&mut self) -> Result<usize, ReviewOnlyBaselineError> {
        usize::try_from(u64::from_le_bytes(
            self.take(8)?.try_into().expect("exact u64 width"),
        ))
        .map_err(|_| ReviewOnlyBaselineError::new("baseline length exceeds usize"))
    }

    fn bytes(&mut self, maximum: usize) -> Result<&'a [u8], ReviewOnlyBaselineError> {
        let length = self.usize()?;
        if length > maximum {
            return Err(ReviewOnlyBaselineError::new(
                "review baseline field exceeds its byte ceiling",
            ));
        }
        self.take(length)
    }

    fn string(&mut self, maximum: usize) -> Result<&'a str, ReviewOnlyBaselineError> {
        std::str::from_utf8(self.bytes(maximum)?)
            .map_err(|_| ReviewOnlyBaselineError::new("review baseline string is not UTF-8"))
    }

    fn array_32(&mut self) -> Result<[u8; 32], ReviewOnlyBaselineError> {
        Ok(self.take(32)?.try_into().expect("exact digest width"))
    }

    fn finish(self) -> Result<(), ReviewOnlyBaselineError> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(ReviewOnlyBaselineError::new(
                "review baseline capsule has trailing bytes",
            ))
        }
    }
}

#[cfg(test)]
mod replay_record_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn replay_record_option_framing_round_trips_compiler_bytes() {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let project = std::env::temp_dir().join(format!(
            "omega-review-baseline-replay-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&project);
        std::fs::create_dir_all(&project).expect("create replay framing fixture");
        std::fs::write(
            project.join("build.omg"),
            r#"use omega::language::std::filesystem_host;

target windows_x64 { }

data ReplayProbe {
    filesystem: FilesystemHost;
    status: i32;
    bytes: [u8; 144];
}

machine ReplayProbe::build(&mut self, builder: &mut Build)
reaches FilesystemHost
{
    let source: &[u8] in Path = builder.source.resolve("main.omg");
    self.status = self.filesystem.read_metadata(source, &mut self.bytes);
}
"#,
        )
        .expect("write replay framing build");
        std::fs::write(project.join("main.omg"), "data Main { value: u8; }\n")
            .expect("write replay framing source");
        let compilation =
            omega_compiler::compile_to_checked(&project.join("main.omg"), Some("windows_x64"))
                .expect("compile replay framing fixture");
        let summary = compilation
            .build_observation_summary()
            .expect("filesystem build publishes observations");
        assert!(summary.source_inputs_replay_verified());
        let limits = ReviewOnlyBaselineLimits::default();
        let replay =
            capture_verified_build_filesystem_replay_record(summary, replay_record_limits(limits))
                .expect("capture replay record")
                .expect("verified replay record");

        let mut encoder = Encoder::bounded(limits.maximum_capsule_bytes);
        encode_replay_record_option(&mut encoder, Some(&replay)).expect("frame replay option");
        let framed = encoder.finish().expect("finish replay option");
        let mut decoder = Decoder::new(&framed);
        let recovered = decode_replay_record_option(&mut decoder, limits)
            .expect("recover framed replay option")
            .expect("recovered replay option is present");
        decoder.finish().expect("replay option consumes its frame");
        assert_eq!(recovered, replay);

        let parent = [7; 32];
        assert_eq!(
            replay_parent_binding(parent, recovered.commitment()),
            replay_parent_binding(parent, replay.commitment())
        );
        assert_ne!(
            replay_parent_binding(parent, recovered.commitment()),
            replay_parent_binding([8; 32], recovered.commitment())
        );

        assert_eq!(
            decode_replay_record_option(&mut Decoder::new(&[0]), limits)
                .expect("absent replay option")
                .as_ref(),
            None
        );
        assert!(decode_replay_record_option(&mut Decoder::new(&[2]), limits).is_err());
        let _ = std::fs::remove_dir_all(project);
    }
}
