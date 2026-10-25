//! Persistent, content-verified cache of compiled package reviews.
//!
//! `compile_dependency_closure` retains each dependency occurrence's compile
//! for the passes of one `update` invocation, but nothing survives the
//! process: the next `update` recompiles the whole dependency closure even
//! when resolution, sources, targets, bindings and build snapshots are all
//! identical. This module retains one review record per dependency
//! occurrence — keyed by SHA-256 over package identity, purpose, the checked
//! context (target plus admitted build profile), the compiler input identity
//! the lock records, the live-inputs digest and the build-snapshot digest —
//! beneath the same resolver lane that already carries the checked-source
//! index.
//!
//! A record never replays a checked verdict by name. The SHA-256 trailer
//! rejects corruption and truncation, and the policy baseline decodes once
//! from its stored canonical bytes, whose digest is the policy commitment. A
//! local writer able to forge a record could equally edit the lock, so a hit
//! does not re-encode the decoded policy to compare it. The carried
//! source-consumption commitment is re-derived over the stored consumed units
//! plus the live inputs before the occurrence counts, because those inputs
//! are live. Stale and foreign records are misses — the worst outcome is a
//! fresh compile.
//!
//! The live compiler source owner is deliberately not persisted: its sealed
//! program storage is not serializable. The cache stores the facts a
//! carried occurrence's `checked` field is actually consumed for — the
//! tolerated flag, the source-consumption commitment inputs, the
//! target-entry candidate and, when a consumer demanded one, the published
//! independent-component description. Publication lowers the dependency to
//! Terminal Psi, so a record does not publish one speculatively: a consumer
//! that later demands a description a record lacks recompiles that
//! dependency, whose replacement record then carries it.
//! Entry candidates remain proposals, not admission authority. The graph root
//! is never cached: its checked compilation is the update's real product.
//!
//! The key binds the build-produced compiler identity and the exact injected
//! toolchain library content, including developer builds. If either identity
//! cannot be established, no record is loaded or stored. Canonical record
//! decoding checks structure independently; it is not proof that a different
//! compiler would issue the same review.
//!
//! `key` derives the record key and the provenance digests a record
//! stores; `record_codec` owns the record byte layout. This module owns the
//! record type, its capture from a fresh review, and the directory store.

mod key;
mod record_codec;

pub(crate) use key::compiled_review_key;

use key::{lineage_digest, resolution_digest};
use package_compilation::{
    AcceptedSemanticBinding, CheckedSourceCacheLimits, ConsumedSourceUnit,
    IndependentComponentDescription, PackageGeneratedSourceBundle,
    PackageSourceConsumptionCommitment,
};
use package_evidence::encoding::PackagePolicyRecoveryLimits;
use package_evidence::record::PackagePolicyBaseline;
use package_source::ImmutableSourceResolution;
use record_codec::{decode_record, encode_record};
use semantic_vocabulary::PackageKeyIdentity;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::review_set::CompilerIssuedPackageReview;
use crate::declarations::{DependencyPurpose, PackageKey};
use package_evidence::record::SemanticBindingReviewCandidate;

const RECORD_EXTENSION: &str = "rec";
const DEFAULT_MAX_RECORDS: usize = 256;
const DEFAULT_MAX_RECORD_BYTES: u64 = 512 * 1024 * 1024;

/// The substitution payload replacing a persisted occurrence's
/// live compiler source: the facts needed by carried dependency occurrences.
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
    policy_bytes: Vec<u8>,
    facts: CachedOccurrenceFacts,
    policy_canonical_bytes: u64,
}

impl CachedCompiledReview {
    /// Rebuild the review the compiler issued plus the carried-compile facts
    /// the cache substitutes for live compiler custody. Live `key` and
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
        let limits = PackagePolicyRecoveryLimits::default();
        let policy = PackagePolicyBaseline::recover_canonical(&self.policy_bytes, limits).ok()?;
        let (policy_rows, _) = policy
            .canonical_rows_with_limits(package_evidence::record::PackagePolicyRowLimits::default())
            .ok()?;
        let policy_commitment = crate::digest_encoder::content_identity(&self.policy_bytes).into();
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
            policy,
            policy_rows,
            policy_commitment,
        };
        Some((review, self.facts))
    }

    pub(crate) fn policy_canonical_bytes(&self) -> Option<usize> {
        usize::try_from(self.policy_canonical_bytes).ok()
    }
}

/// One cache directory: a flat sequence of `<key-hex>.rec` records whose
/// names are the record keys. Records are immutable, content-named and
/// self-verifying. A full cache evicts its least recently written records:
/// keys bind the compiler identity and the toolchain library content, so
/// every compiler rebuild or library edit strands the records it supersedes,
/// and a cache that declined when full stopped storing for good once those
/// accumulated. Losing a record costs one fresh compile, never a verdict.
pub(crate) struct CompiledReviewCache {
    directory: PathBuf,
    limits: CheckedSourceCacheLimits,
}

impl CompiledReviewCache {
    /// Open an existing lane directory. A path that cannot be listed or is
    /// not a directory yields `None`, and the caller compiles fresh.
    pub(crate) fn open(directory: &Path) -> Option<Self> {
        let metadata = std::fs::metadata(directory).ok()?;
        metadata.is_dir().then_some(Self {
            directory: directory.to_path_buf(),
            limits: CheckedSourceCacheLimits {
                max_records: DEFAULT_MAX_RECORDS,
                max_record_bytes: DEFAULT_MAX_RECORD_BYTES,
            },
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
    /// best-effort: an oversize record declines silently, and a full
    /// directory first evicts its oldest records. `replace` overwrites an
    /// existing record under the same key: a recompile forced by a demanded
    /// component description replaces the record that lacked it.
    pub(crate) fn store(&self, key: &[u8; 32], review: &CachedCompiledReview, replace: bool) {
        let mut payload = Vec::new();
        encode_record(review, &mut payload);
        if payload.len() as u64 > self.limits.max_record_bytes {
            return;
        }
        let mut trailer = Vec::with_capacity(payload.len() + 32);
        trailer.extend_from_slice(&payload);
        let digest: [u8; 32] = crate::digest_encoder::content_identity(&payload).into();
        trailer.extend_from_slice(&digest);
        let path = self.record_path(key);
        let exists = path.exists();
        if exists && !replace {
            return;
        }
        if !exists && !self.make_room() {
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

    /// Leave room for one more record, removing the least recently written
    /// records when the directory is at capacity. Returns whether a record
    /// may be added. A concurrent writer can race this count; the bound is a
    /// size budget, and a removed record is only a later miss.
    fn make_room(&self) -> bool {
        let Ok(entries) = std::fs::read_dir(&self.directory) else {
            return false;
        };
        let mut records = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == RECORD_EXTENSION)
            })
            .map(|entry| {
                let written = entry
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                (written, entry.path())
            })
            .collect::<Vec<_>>();
        let Some(excess) = (records.len() + 1).checked_sub(self.limits.max_records) else {
            return true;
        };
        records.sort();
        for (_, path) in records.iter().take(excess) {
            let _ = std::fs::remove_file(path);
        }
        true
    }
}

/// Capture a completed review and its source-free carry facts into a record.
pub(crate) fn record_compiled_review(
    review: &CompilerIssuedPackageReview,
    facts: CachedOccurrenceFacts,
    policy_canonical_bytes: usize,
) -> Option<CachedCompiledReview> {
    let policy_bytes = review.policy().canonical_bytes().ok()?;
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
        policy_bytes,
        facts,
        policy_canonical_bytes: u64::try_from(policy_canonical_bytes).ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::{CheckedSourceCacheLimits, CompiledReviewCache};
    use std::time::{Duration, SystemTime};

    #[test]
    fn full_cache_evicts_its_oldest_records_before_storing() {
        let directory = std::env::temp_dir().join(format!(
            "omega-review-cache-eviction-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        let written = |name: &str, seconds: u64| {
            let path = directory.join(name);
            std::fs::write(&path, b"record").unwrap();
            std::fs::File::options()
                .write(true)
                .open(&path)
                .unwrap()
                .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
                .unwrap();
        };
        written("oldest.rec", 100);
        written("older.rec", 200);
        written("newest.rec", 300);
        written("staging.tmp", 50);
        let cache = CompiledReviewCache {
            directory: directory.clone(),
            limits: CheckedSourceCacheLimits {
                max_records: 2,
                max_record_bytes: 1024,
            },
        };
        assert!(cache.make_room());
        let mut remaining = std::fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        remaining.sort();
        assert_eq!(remaining, ["newest.rec", "staging.tmp"]);
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
