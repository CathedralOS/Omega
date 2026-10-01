//! Persistent, content-verified cache of compiled package reviews.
//!
//! `compile_dependency_closure` retains each dependency occurrence's compile
//! for the passes of one `update` invocation, but nothing survives the
//! process: the next `update` recompiles the whole dependency closure even
//! when resolution, sources, targets, bindings and build snapshots are all
//! identical. This module retains one review record per dependency
//! occurrence — keyed by SHA-256 over package identity, purpose, the checked
//! context (target plus admitted build profile), the toolchain stamp, the
//! live-inputs digest and the build-snapshot digest — beneath the same
//! resolver lane that already carries the checked-source index.
//!
//! A record never replays a checked verdict by name: on a lookup hit the
//! retained rows decode strictly, the projection is recovered from its
//! canonical review bytes and is required to re-encode to them byte-for-byte,
//! the policy baseline and obligation ledger round-trip through their own
//! canonical codecs, and the carried source-consumption commitment is
//! re-derived over the stored consumed units plus the live inputs before the
//! occurrence counts. Corruption, staleness, truncation and foreign records
//! are misses, never trusted rows — the worst outcome is a fresh compile.
//!
//! `CheckedCompilation` itself is deliberately not persisted: its sealed
//! program storage is not serializable. The cache stores the facts a
//! carried occurrence's `checked` field is actually consumed for — the
//! tolerated flag, the source-consumption commitment inputs, the
//! published independent-component description and target-entry candidate.
//! Entry candidates remain proposals, not admission authority. The graph root
//! is never cached: its checked compilation is the update's real product.
//!
//! The toolchain stamp binds a record to the binary that minted it
//! (`OMEGA_RELEASE_VERSION`/`OMEGA_RELEASE_COMMIT` at compile time, falling
//! back to the package version and `unrecorded` for dev builds). A dev build
//! that changes review semantics without changing the stamp relies on the
//! record's self-verifying decode; bump `RECORD_VERSION` to invalidate.
//!
//! `key` derives the record key and the provenance digests a record
//! stores; `record_codec` owns the record byte layout. This module owns the
//! record type, its capture from a fresh review, and the directory store.

mod key;
mod record_codec;

pub(crate) use key::compiled_review_key;

use key::{lineage_digest, resolution_digest};
use package_compilation::{
    AcceptedSemanticBinding, ConsumedSourceUnit, IndependentComponentDescription,
    PackageGeneratedSourceBundle, PackageSourceConsumptionCommitment,
};
use package_evidence::encoding::{
    PackagePolicyRecoveryLimits, decode_package_review_canonical_row,
    encode_package_review_canonical_row, recover_package_review_projection,
};
use package_evidence::ledger::{
    decode_ordinary_package_obligation_ledger, encode_ordinary_package_obligation_ledger,
    ordinary_package_obligation_results_from_projection,
};
use package_evidence::record::PackagePolicyBaseline;
use package_source::ImmutableSourceResolution;
use record_codec::{decode_record, encode_record};
use semantic_vocabulary::PackageKeyIdentity;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::review_set::CompilerIssuedPackageReview;
use super::rows::RetainedReviewRows;
use super::semantic_bindings::SemanticBindingReviewCandidate;
use crate::declarations::{DependencyPurpose, PackageKey};

const RECORD_EXTENSION: &str = "rec";
const DEFAULT_MAX_RECORDS: usize = 256;
const DEFAULT_MAX_RECORD_BYTES: u64 = 512 * 1024 * 1024;

/// Capacity contract for one cache directory. A record beyond the byte
/// ceiling, or a directory already holding `max_records` records, declines
/// the store — the compile still returns its fresh review.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompiledReviewCacheLimits {
    pub max_records: usize,
    pub max_record_bytes: u64,
}

impl Default for CompiledReviewCacheLimits {
    fn default() -> Self {
        Self {
            max_records: DEFAULT_MAX_RECORDS,
            max_record_bytes: DEFAULT_MAX_RECORD_BYTES,
        }
    }
}

/// The substitution payload replacing a persisted occurrence's
/// `CheckedCompilation`: the facts needed by carried dependency occurrences.
#[derive(Debug, Clone)]
pub(crate) struct CachedOccurrenceFacts {
    pub tolerated_unsettled_service_fields: bool,
    pub consumed_units: Vec<ConsumedSourceUnit>,
    pub source_consumption_commitment: PackageSourceConsumptionCommitment,
    pub component_description: Option<IndependentComponentDescription>,
    pub target_entry_candidate: Option<SemanticBindingReviewCandidate>,
}

/// The decoded contents of one cache record. `key` and `resolution` are not
/// stored as data — the lookup's live package key and custody resolution are
/// the record's values; the stored digests are compared and the record is a
/// miss on any difference.
#[derive(Debug)]
pub(crate) struct CachedCompiledReview {
    package_identity: PackageKeyIdentity,
    name: String,
    lineage: [u8; 32],
    resolution: [u8; 32],
    purpose: DependencyPurpose,
    target: target::TargetProfile,
    build_execution_profile: Option<target::TargetProfile>,
    selected_build_machine_identity: String,
    build_evaluation_usage: Option<build_evaluation::BuildEvaluationUsage>,
    build_observation_summary: Option<build_evaluation::BuildObservationSummary>,
    restricted_build_requests: Vec<build_evaluation::RestrictedBuildRequest>,
    semantic_bindings: Vec<AcceptedSemanticBinding>,
    semantic_binding_candidates: Vec<SemanticBindingReviewCandidate>,
    generated_source_bundle: PackageGeneratedSourceBundle,
    canonical_review_bytes: Vec<u8>,
    canonical_row_bytes: Vec<Vec<u8>>,
    policy_bytes: Vec<u8>,
    obligation_ledger_bytes_payload: Vec<u8>,
    facts: CachedOccurrenceFacts,
    policy_canonical_bytes: u64,
    obligation_ledger_bytes: u64,
}

impl CachedCompiledReview {
    /// Rebuild the review the compiler issued plus the carried-compile facts
    /// the cache substitutes for `CheckedCompilation`. Live `key` and
    /// `resolution` supply the provenance fields; the record's stored
    /// digests must match them — a mismatch means the record was filed under
    /// the wrong identity and is dropped as a miss.
    pub(crate) fn into_review_and_facts(
        self,
        key: &PackageKey,
        resolution: &ImmutableSourceResolution,
    ) -> Option<(CompilerIssuedPackageReview, CachedOccurrenceFacts)> {
        if self.package_identity != key.identity()
            || self.name != key.name().as_str()
            || self.lineage != lineage_digest(key.source_lineage())
            || self.resolution != resolution_digest(resolution)
        {
            return None;
        }
        let canonical_rows = self
            .canonical_row_bytes
            .iter()
            .map(|bytes| {
                decode_package_review_canonical_row(bytes)
                    .ok()
                    .map(|decoded| decoded.into_row())
            })
            .collect::<Option<Vec<_>>>()?;
        let limits = PackagePolicyRecoveryLimits::default();
        let projection = recover_package_review_projection(
            &self.canonical_review_bytes,
            &canonical_rows,
            limits,
        )
        .ok()?;
        let policy = PackagePolicyBaseline::recover_canonical(&self.policy_bytes, limits).ok()?;
        if policy.canonical_bytes().ok()?.as_slice() != self.policy_bytes.as_slice() {
            return None;
        }
        let obligations =
            decode_ordinary_package_obligation_ledger(&self.obligation_ledger_bytes_payload)
                .ok()?;
        if encode_ordinary_package_obligation_ledger(&obligations)
            .ok()?
            .as_slice()
            != self.obligation_ledger_bytes_payload.as_slice()
        {
            return None;
        }
        let obligation_results =
            ordinary_package_obligation_results_from_projection(&obligations, &projection).ok()?;
        let review = CompilerIssuedPackageReview {
            key: key.clone(),
            resolution: resolution.clone(),
            source_consumption_commitment: self.facts.source_consumption_commitment,
            selected_build_machine_identity: self.selected_build_machine_identity,
            build_evaluation_usage: self.build_evaluation_usage,
            build_observation_summary: self.build_observation_summary,
            restricted_build_requests: self.restricted_build_requests,
            semantic_bindings: self.semantic_bindings,
            semantic_binding_candidates: self.semantic_binding_candidates,
            generated_source_bundle: self.generated_source_bundle,
            projection,
            policy,
            canonical_review_bytes: self.canonical_review_bytes,
            canonical_rows: RetainedReviewRows(canonical_rows),
            obligations,
            obligation_results,
        };
        Some((review, self.facts))
    }

    pub(crate) fn policy_canonical_bytes(&self) -> Option<usize> {
        usize::try_from(self.policy_canonical_bytes).ok()
    }

    pub(crate) fn obligation_ledger_bytes(&self) -> Option<usize> {
        usize::try_from(self.obligation_ledger_bytes).ok()
    }
}

/// One cache directory: a flat sequence of `<key-hex>.rec` records whose
/// names are the record keys. Records are immutable, content-named and
/// self-verifying; a full cache declines rather than evicting.
pub(crate) struct CompiledReviewCache {
    directory: PathBuf,
    limits: CompiledReviewCacheLimits,
}

impl CompiledReviewCache {
    /// Open an existing lane directory. A path that cannot be listed or is
    /// not a directory yields `None`, and the caller compiles fresh.
    pub(crate) fn open(directory: &Path) -> Option<Self> {
        let metadata = std::fs::metadata(directory).ok()?;
        metadata.is_dir().then_some(Self {
            directory: directory.to_path_buf(),
            limits: CompiledReviewCacheLimits::default(),
        })
    }

    fn record_path(&self, key: &[u8; 32]) -> PathBuf {
        let mut name = String::with_capacity(64 + 4);
        use std::fmt::Write;
        for byte in key {
            let _ = write!(name, "{byte:02x}");
        }
        name.push('.');
        name.push_str(RECORD_EXTENSION);
        self.directory.join(name)
    }

    /// Decode the record filed under `key`. Any decode or self-verification
    /// failure removes the record and reports a miss.
    pub(crate) fn lookup(&self, key: &[u8; 32]) -> Option<CachedCompiledReview> {
        let path = self.record_path(key);
        let bytes = std::fs::read(&path).ok()?;
        match decode_record(&bytes) {
            Some(review) => Some(review),
            None => {
                let _ = std::fs::remove_file(&path);
                None
            }
        }
    }

    /// Persist one compiled review record under `key`. Storing is
    /// best-effort: oversize records and full directories decline silently.
    pub(crate) fn store(&self, key: &[u8; 32], review: &CachedCompiledReview) {
        let mut payload = Vec::new();
        encode_record(review, &mut payload);
        if payload.len() as u64 > self.limits.max_record_bytes {
            return;
        }
        let mut trailer = Vec::with_capacity(payload.len() + 32);
        trailer.extend_from_slice(&payload);
        let digest: [u8; 32] = Sha256::digest(&payload).into();
        trailer.extend_from_slice(&digest);
        let path = self.record_path(key);
        if path.exists() {
            return;
        }
        let count = match std::fs::read_dir(&self.directory) {
            Ok(entries) => entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == RECORD_EXTENSION)
                })
                .count(),
            Err(_) => return,
        };
        if count >= self.limits.max_records {
            return;
        }
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let temporary = self.directory.join(format!(
            "{}.{}.tmp",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        if std::fs::write(&temporary, &trailer).is_err() {
            let _ = std::fs::remove_file(&temporary);
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o444));
        }
        if std::fs::rename(&temporary, &path).is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
    }
}

/// Capture one freshly compiled review into a storable record, together with
/// the carry-substitution facts derived from its checked compilation.
pub(crate) fn record_compiled_review(
    review: &CompilerIssuedPackageReview,
    checked: &compiler::CheckedCompilation,
    consumed_units: Vec<ConsumedSourceUnit>,
    component_description: Option<IndependentComponentDescription>,
    policy_canonical_bytes: usize,
    obligation_ledger_bytes: usize,
) -> Option<CachedCompiledReview> {
    let canonical_row_bytes = review
        .canonical_rows()
        .iter()
        .map(|row| encode_package_review_canonical_row(row).ok())
        .collect::<Option<Vec<_>>>()?;
    let policy_bytes = review.policy().canonical_bytes().ok()?;
    let obligation_ledger_bytes_payload =
        encode_ordinary_package_obligation_ledger(review.obligations()).ok()?;
    Some(CachedCompiledReview {
        package_identity: review.key().identity(),
        name: review.key().name().as_str().to_owned(),
        lineage: lineage_digest(review.key().source_lineage()),
        resolution: resolution_digest(review.resolution()),
        purpose: review.checked_context().purpose(),
        target: review.generated_source_bundle().target(),
        build_execution_profile: review.generated_source_bundle().build_execution_profile(),
        selected_build_machine_identity: review.selected_build_machine_identity().to_owned(),
        build_evaluation_usage: review.build_evaluation_usage(),
        build_observation_summary: review.build_observation_summary().cloned(),
        restricted_build_requests: review.restricted_build_requests().to_vec(),
        semantic_bindings: review.semantic_bindings().to_vec(),
        semantic_binding_candidates: review.semantic_binding_candidates().to_vec(),
        generated_source_bundle: review.generated_source_bundle().clone(),
        canonical_review_bytes: review.canonical_review_bytes().to_vec(),
        canonical_row_bytes,
        policy_bytes,
        obligation_ledger_bytes_payload,
        facts: CachedOccurrenceFacts {
            tolerated_unsettled_service_fields: checked
                .execution()
                .tolerated_unsettled_service_fields(),
            consumed_units,
            source_consumption_commitment: review.source_consumption_commitment(),
            component_description,
            target_entry_candidate: super::semantic_bindings::candidate_target_entry_binding(
                checked,
                review.key(),
                review.generated_source_bundle().target(),
            )
            .ok()?,
        },
        policy_canonical_bytes: u64::try_from(policy_canonical_bytes).ok()?,
        obligation_ledger_bytes: u64::try_from(obligation_ledger_bytes).ok()?,
    })
}

#[cfg(test)]
mod tests;
