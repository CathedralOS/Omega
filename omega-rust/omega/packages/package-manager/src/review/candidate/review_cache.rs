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

use checked_interpreter::observation_codec::{ObservationCursor, push_framed};
use effects::provider_plan::{
    BoundaryCallingPlanCommitment, ProviderPlanDigest, ServiceEntryAuthorityFlow,
    ServiceEntryClaim, ServiceMethod, ServiceProgressEstablishmentRoute,
    ServiceProgressEstablishmentRouteKind, ServiceProgressPremise, ServiceProgressSubject,
    ServiceResultClaim, ServiceSchema, ServiceSchemaDigest,
};
use effects::{
    ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
};
use language_semantics::{CarryPolicy, DomainPredicateBody};
use package_compilation::build_execution::retained_review_codec;
use package_compilation::{
    AcceptedSemanticBinding, AcceptedSemanticBindingRole, BuildDeclarationKind, ConsumedSourceUnit,
    ConsumedSourceUnitKind, IndependentComponentDescription, PackageCompilationInputs,
    PackageDependencyBinding, PackageDependencyClosure, PackageGeneratedSourceBundle,
    PackageSourceConsumptionCommitment, review_cache_inputs_digest, review_cache_snapshot_digest,
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
use package_source::{ImmutableSourceResolution, SourceLineage};
use semantic_vocabulary::PackageKeyIdentity;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::review_set::CompilerIssuedPackageReview;
use super::rows::RetainedReviewRows;
use super::semantic_bindings::SemanticBindingReviewCandidate;
use crate::declarations::{DependencyPurpose, PackageKey};

const RECORD_MAGIC: &[u8; 8] = b"OMGCREV\x01";
const RECORD_KEY_DOMAIN: &[u8] = b"OMEGA-COMPILED-REVIEW-CACHE-KEY-V2\0";
const RECORD_EXTENSION: &str = "rec";
const RECORD_VERSION: u16 = 2;
const DEFAULT_MAX_RECORDS: usize = 256;
const DEFAULT_MAX_RECORD_BYTES: u64 = 512 * 1024 * 1024;
const RECORD_SEQUENCES_LIMIT: u64 = 1 << 20;

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

/// The compiler toolchain stamp folded into every record key. Release builds
/// stamp their exact version and commit; dev builds stamp the package
/// version plus `unrecorded` — records remain self-verifying either way.
fn stamp_string() -> String {
    let version =
        option_env!("OMEGA_RELEASE_VERSION").unwrap_or(concat!(env!("CARGO_PKG_VERSION"), "-dev"));
    let commit = option_env!("OMEGA_RELEASE_COMMIT").unwrap_or("unrecorded");
    format!("{version}|{commit}")
}

/// The SHA-256 key over everything a persisted review depends on: package
/// identity, occurrence purpose, the checked context (target plus admitted
/// build profile), the checked entry path, the toolchain that minted the
/// record, and the content digests of the complete compilation inputs and
/// the build snapshot.
pub(crate) fn compiled_review_key(
    package: PackageKeyIdentity,
    purpose: DependencyPurpose,
    checked_target: target::TargetProfile,
    build_execution_profile: Option<target::TargetProfile>,
    entry: &Path,
    inputs: &PackageCompilationInputs,
    snapshot: &build_evaluation::BuildSnapshotRequest,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(RECORD_KEY_DOMAIN);
    digest.update(package.digest());
    digest.update([purpose_tag(purpose)]);
    hash_bytes(&mut digest, checked_target.target_name().as_bytes());
    match build_execution_profile {
        Some(profile) => {
            digest.update([1]);
            hash_bytes(&mut digest, profile.target_name().as_bytes());
        }
        None => digest.update([0]),
    }
    hash_bytes(&mut digest, entry.as_os_str().as_encoded_bytes());
    hash_bytes(&mut digest, stamp_string().as_bytes());
    digest.update(review_cache_inputs_digest(inputs));
    digest.update(review_cache_snapshot_digest(snapshot));
    digest.finalize().into()
}

fn hash_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn lineage_digest(lineage: &SourceLineage) -> [u8; 32] {
    let mut digest = Sha256::new();
    lineage.hash_canonical(&mut digest);
    digest.finalize().into()
}

fn resolution_digest(resolution: &ImmutableSourceResolution) -> [u8; 32] {
    let mut digest = Sha256::new();
    match resolution {
        ImmutableSourceResolution::Git {
            commit,
            tree,
            content,
        } => {
            digest.update([0]);
            hash_bytes(&mut digest, commit.to_hex().as_bytes());
            hash_bytes(&mut digest, tree.to_hex().as_bytes());
            digest.update(content.digest());
        }
        ImmutableSourceResolution::Workspace { content } => {
            digest.update([1]);
            digest.update(content.digest());
        }
        ImmutableSourceResolution::ExternalLocal { content } => {
            digest.update([2]);
            digest.update(content.digest());
        }
    }
    digest.finalize().into()
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
            tolerated_unsettled_service_fields: checked.tolerated_unsettled_service_fields(),
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

// ---------------------------------------------------------------------------
// Record codec. Sequences are u64 length-prefixed; byte strings are framed;
// every enum is a checked tag. Unknown tags, trailing bytes and length
// overflows are all decode failures.

fn cursor_bool(cursor: &mut ObservationCursor<'_>) -> Option<bool> {
    match cursor.u8()? {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

fn cursor_digest(cursor: &mut ObservationCursor<'_>) -> Option<[u8; 32]> {
    cursor.bytes(32)?.try_into().ok()
}

fn cursor_string<'a>(cursor: &mut ObservationCursor<'a>) -> Option<&'a str> {
    std::str::from_utf8(cursor.framed()?).ok()
}

fn cursor_vec<'a, T>(
    cursor: &mut ObservationCursor<'a>,
    mut decode: impl FnMut(&mut ObservationCursor<'_>) -> Option<T>,
) -> Option<Vec<T>> {
    let count = cursor.u64()?;
    if count > RECORD_SEQUENCES_LIMIT {
        return None;
    }
    let mut values = Vec::with_capacity(usize::try_from(count).ok()?.min(1 << 20));
    for _ in 0..count {
        values.push(decode(cursor)?);
    }
    Some(values)
}

fn encode_string(value: &str, out: &mut Vec<u8>) {
    push_framed(out, value.as_bytes());
}

fn encode_optional<T>(value: Option<&T>, out: &mut Vec<u8>, encode: impl Fn(&T, &mut Vec<u8>)) {
    match value {
        Some(value) => {
            out.push(1);
            encode(value, out);
        }
        None => out.push(0),
    }
}

fn recover_optional<'a, T>(
    cursor: &mut ObservationCursor<'a>,
    decode: impl Fn(&mut ObservationCursor<'a>) -> Option<T>,
) -> Option<Option<T>> {
    match cursor.u8()? {
        0 => Some(None),
        1 => decode(cursor).map(Some),
        _ => None,
    }
}

fn purpose_tag(purpose: DependencyPurpose) -> u8 {
    match purpose {
        DependencyPurpose::Product => 0,
        DependencyPurpose::Build => 1,
    }
}

fn recover_purpose(cursor: &mut ObservationCursor<'_>) -> Option<DependencyPurpose> {
    match cursor.u8()? {
        0 => Some(DependencyPurpose::Product),
        1 => Some(DependencyPurpose::Build),
        _ => None,
    }
}

fn declaration_kind_tag(kind: BuildDeclarationKind) -> u8 {
    match kind {
        BuildDeclarationKind::Package => 0,
        BuildDeclarationKind::Application => 1,
        BuildDeclarationKind::Workspace => 2,
    }
}

fn recover_declaration_kind(cursor: &mut ObservationCursor<'_>) -> Option<BuildDeclarationKind> {
    match cursor.u8()? {
        0 => Some(BuildDeclarationKind::Package),
        1 => Some(BuildDeclarationKind::Application),
        2 => Some(BuildDeclarationKind::Workspace),
        _ => None,
    }
}

fn encode_target(profile: target::TargetProfile, out: &mut Vec<u8>) {
    encode_string(profile.target_name(), out);
}

fn recover_target(cursor: &mut ObservationCursor<'_>) -> Option<target::TargetProfile> {
    let name = cursor_string(cursor)?;
    target::TargetProfile::ALL
        .iter()
        .copied()
        .find(|profile| profile.target_name() == name)
}

fn encode_identity(identity: PackageKeyIdentity, out: &mut Vec<u8>) {
    out.extend_from_slice(&identity.digest());
}

fn recover_identity(cursor: &mut ObservationCursor<'_>) -> Option<PackageKeyIdentity> {
    PackageKeyIdentity::from_digest(cursor_digest(cursor)?)
}

fn encode_role(role: AcceptedSemanticBindingRole, out: &mut Vec<u8>) {
    out.push(match role {
        AcceptedSemanticBindingRole::ConsoleExitProcessI32 => 0,
        AcceptedSemanticBindingRole::ProcessExitExitProcessI32 => 1,
        AcceptedSemanticBindingRole::FilesystemHostService => 2,
        AcceptedSemanticBindingRole::TimeHostService => 3,
        AcceptedSemanticBindingRole::UefiX64ProgramEntry => 4,
        AcceptedSemanticBindingRole::MacosArm64ProgramEntry => 5,
        AcceptedSemanticBindingRole::MacosX64ProgramEntry => 6,
        AcceptedSemanticBindingRole::LinuxX86_64ProgramEntry => 7,
        AcceptedSemanticBindingRole::LinuxArm64ProgramEntry => 8,
        AcceptedSemanticBindingRole::WindowsX64ProgramEntry => 9,
    });
}

fn recover_role(cursor: &mut ObservationCursor<'_>) -> Option<AcceptedSemanticBindingRole> {
    Some(match cursor.u8()? {
        0 => AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        1 => AcceptedSemanticBindingRole::ProcessExitExitProcessI32,
        2 => AcceptedSemanticBindingRole::FilesystemHostService,
        3 => AcceptedSemanticBindingRole::TimeHostService,
        4 => AcceptedSemanticBindingRole::UefiX64ProgramEntry,
        5 => AcceptedSemanticBindingRole::MacosArm64ProgramEntry,
        6 => AcceptedSemanticBindingRole::MacosX64ProgramEntry,
        7 => AcceptedSemanticBindingRole::LinuxX86_64ProgramEntry,
        8 => AcceptedSemanticBindingRole::LinuxArm64ProgramEntry,
        9 => AcceptedSemanticBindingRole::WindowsX64ProgramEntry,
        _ => return None,
    })
}

fn encode_disposition(disposition: &TerminalAuthorityDisposition, out: &mut Vec<u8>) {
    out.extend_from_slice(&(disposition.classes().len() as u64).to_le_bytes());
    for class in disposition.classes() {
        out.push(match class {
            TerminalAuthorityClass::FilesystemContentRead => 0,
            TerminalAuthorityClass::FilesystemContentWrite => 1,
            TerminalAuthorityClass::FilesystemMetadataQuery => 2,
            TerminalAuthorityClass::DirectoryEnumeration => 3,
            TerminalAuthorityClass::FilesystemNamespaceMutation => 4,
            TerminalAuthorityClass::FilesystemMetadataMutation => 5,
            TerminalAuthorityClass::ProcessOutput => 6,
            TerminalAuthorityClass::ProcessTermination => 7,
            TerminalAuthorityClass::MachineControl => 8,
            TerminalAuthorityClass::PortIo => 9,
            TerminalAuthorityClass::InterruptControl => 10,
            TerminalAuthorityClass::InterruptEntry => 11,
            TerminalAuthorityClass::RootMemoryAccess => 12,
            TerminalAuthorityClass::ProcessInput => 13,
        });
    }
}

fn recover_disposition(cursor: &mut ObservationCursor<'_>) -> Option<TerminalAuthorityDisposition> {
    let classes = cursor_vec(cursor, |cursor| {
        Some(match cursor.u8()? {
            0 => TerminalAuthorityClass::FilesystemContentRead,
            1 => TerminalAuthorityClass::FilesystemContentWrite,
            2 => TerminalAuthorityClass::FilesystemMetadataQuery,
            3 => TerminalAuthorityClass::DirectoryEnumeration,
            4 => TerminalAuthorityClass::FilesystemNamespaceMutation,
            5 => TerminalAuthorityClass::FilesystemMetadataMutation,
            6 => TerminalAuthorityClass::ProcessOutput,
            7 => TerminalAuthorityClass::ProcessTermination,
            8 => TerminalAuthorityClass::MachineControl,
            9 => TerminalAuthorityClass::PortIo,
            10 => TerminalAuthorityClass::InterruptControl,
            11 => TerminalAuthorityClass::InterruptEntry,
            12 => TerminalAuthorityClass::RootMemoryAccess,
            13 => TerminalAuthorityClass::ProcessInput,
            _ => return None,
        })
    })?;
    TerminalAuthorityDisposition::try_from_canonical_classes(classes).ok()
}

fn encode_permission(permission: &ServiceTerminalAuthorityPermission, out: &mut Vec<u8>) {
    out.extend_from_slice(permission.service_schema().as_bytes());
    encode_string(permission.requirement_identity(), out);
    encode_disposition(permission.permitted(), out);
}

fn recover_permission(
    cursor: &mut ObservationCursor<'_>,
) -> Option<ServiceTerminalAuthorityPermission> {
    Some(ServiceTerminalAuthorityPermission::new(
        ServiceSchemaDigest::from_digest(cursor_digest(cursor)?),
        cursor_string(cursor)?,
        recover_disposition(cursor)?,
    ))
}

fn encode_binding(binding: &AcceptedSemanticBinding, out: &mut Vec<u8>) {
    encode_role(binding.role(), out);
    encode_identity(binding.package(), out);
    encode_string(binding.declaration_path(), out);
    out.extend_from_slice(binding.normalized_schema_digest().as_bytes());
    encode_optional(
        binding.selected_provider_plan_digest().as_ref(),
        out,
        |digest, out| out.extend_from_slice(digest.as_bytes()),
    );
    out.extend_from_slice(&(binding.terminal_authority_permissions().len() as u64).to_le_bytes());
    for permission in binding.terminal_authority_permissions() {
        encode_permission(permission, out);
    }
}

fn recover_binding(cursor: &mut ObservationCursor<'_>) -> Option<AcceptedSemanticBinding> {
    let role = recover_role(cursor)?;
    let package = recover_identity(cursor)?;
    let declaration_path = cursor_string(cursor)?;
    let schema = ServiceSchemaDigest::from_digest(cursor_digest(cursor)?);
    let plan_digest = recover_optional(cursor, |cursor| {
        Some(ProviderPlanDigest::from_digest(cursor_digest(cursor)?))
    })?;
    let binding = match plan_digest {
        Some(digest) => {
            AcceptedSemanticBinding::new(role, package, declaration_path, schema, digest).ok()?
        }
        None => {
            AcceptedSemanticBinding::new_service(role, package, declaration_path, schema).ok()?
        }
    };
    let permissions = cursor_vec(cursor, recover_permission)?;
    binding
        .with_terminal_authority_permissions(permissions)
        .ok()
}

fn encode_carry(carry: &CarryPolicy, out: &mut Vec<u8>) {
    out.push(match carry.suspension {
        language_semantics::CarrySuspension::Forbidden => 0,
        language_semantics::CarrySuspension::Allowed => 1,
    });
    out.push(match carry.cpu {
        language_semantics::CarryCpu::Origin => 0,
        language_semantics::CarryCpu::Any => 1,
    });
    out.push(match carry.host_thread {
        language_semantics::CarryHostThread::Origin => 0,
        language_semantics::CarryHostThread::Any => 1,
    });
    out.push(match carry.address {
        language_semantics::CarryAddress::Stable => 0,
        language_semantics::CarryAddress::Movable => 1,
    });
}

fn recover_carry(cursor: &mut ObservationCursor<'_>) -> Option<CarryPolicy> {
    let suspension = match cursor.u8()? {
        0 => language_semantics::CarrySuspension::Forbidden,
        1 => language_semantics::CarrySuspension::Allowed,
        _ => return None,
    };
    let cpu = match cursor.u8()? {
        0 => language_semantics::CarryCpu::Origin,
        1 => language_semantics::CarryCpu::Any,
        _ => return None,
    };
    let host_thread = match cursor.u8()? {
        0 => language_semantics::CarryHostThread::Origin,
        1 => language_semantics::CarryHostThread::Any,
        _ => return None,
    };
    let address = match cursor.u8()? {
        0 => language_semantics::CarryAddress::Stable,
        1 => language_semantics::CarryAddress::Movable,
        _ => return None,
    };
    Some(CarryPolicy {
        suspension,
        cpu,
        host_thread,
        address,
    })
}

fn encode_predicate_body(body: &DomainPredicateBody, out: &mut Vec<u8>) {
    out.push(match body {
        DomainPredicateBody::Bodyless => 0,
        DomainPredicateBody::Present => 1,
    });
}

fn recover_predicate_body(cursor: &mut ObservationCursor<'_>) -> Option<DomainPredicateBody> {
    Some(match cursor.u8()? {
        0 => DomainPredicateBody::Bodyless,
        1 => DomainPredicateBody::Present,
        _ => return None,
    })
}

fn encode_service_method(method: &ServiceMethod, out: &mut Vec<u8>) {
    encode_string(&method.name, out);
    encode_string(&method.requirement_owner, out);
    encode_optional(
        method.requirement_owner_package_identity.as_ref(),
        out,
        |identity, out| encode_identity(*identity, out),
    );
    encode_string(&method.requirement_identity, out);
    out.extend_from_slice(&(method.parameter_count as u64).to_le_bytes());
    out.extend_from_slice(&(method.parameter_type_identities.len() as u64).to_le_bytes());
    for identity in &method.parameter_type_identities {
        encode_string(identity, out);
    }
    out.extend_from_slice(&(method.entry_claims.len() as u64).to_le_bytes());
    for claim in &method.entry_claims {
        out.extend_from_slice(&(claim.parameter_index as u64).to_le_bytes());
        encode_string(&claim.carrier_identity, out);
        encode_string(&claim.domain, out);
        encode_predicate_body(&claim.predicate_body, out);
        encode_carry(&claim.effective_carry, out);
        encode_string(claim.authority_flow.as_str(), out);
    }
    out.push(u8::from(method.has_result));
    encode_optional(
        method.result_type_identity.as_ref(),
        out,
        |identity, out| encode_string(identity, out),
    );
    out.extend_from_slice(&(method.result_claims.len() as u64).to_le_bytes());
    for claim in &method.result_claims {
        encode_string(&claim.domain, out);
        encode_carry(&claim.effective_carry, out);
    }
    out.extend_from_slice(&(method.service_reach.len() as u64).to_le_bytes());
    for reach in &method.service_reach {
        encode_string(reach, out);
    }
    out.extend_from_slice(&(method.synchronous_invocations.len() as u64).to_le_bytes());
    for invocation in &method.synchronous_invocations {
        encode_string(invocation, out);
    }
    out.push(u8::from(method.may_suspend));
    out.push(u8::from(method.may_block));
    out.push(u8::from(method.terminates_guarantee));
    out.extend_from_slice(&(method.termination_premises.len() as u64).to_le_bytes());
    for premise in &method.termination_premises {
        encode_string(&premise.profile, out);
        match premise.subject {
            ServiceProgressSubject::ProviderReceiver => out.push(0),
            ServiceProgressSubject::Parameter(index) => {
                out.push(1);
                out.extend_from_slice(&(index as u64).to_le_bytes());
            }
        }
        out.extend_from_slice(&(premise.subject_projections.len() as u64).to_le_bytes());
        for projection in &premise.subject_projections {
            encode_string(projection, out);
        }
        out.extend_from_slice(&(premise.establishment_routes.len() as u64).to_le_bytes());
        for route in &premise.establishment_routes {
            out.push(match route.kind {
                ServiceProgressEstablishmentRouteKind::CheckedRequirement => 0,
                ServiceProgressEstablishmentRouteKind::BoundaryRequirement => 1,
            });
            encode_string(&route.requirement_identity, out);
        }
    }
    encode_optional(
        method.calling_plan_report_fingerprint.as_ref(),
        out,
        |value, out| out.extend_from_slice(&value.to_le_bytes()),
    );
    encode_optional(
        method.calling_plan_commitment.as_ref(),
        out,
        |commitment, out| push_framed(out, &commitment.as_bytes()),
    );
}

fn recover_service_method(cursor: &mut ObservationCursor<'_>) -> Option<ServiceMethod> {
    let name = cursor_string(cursor)?.to_owned();
    let requirement_owner = cursor_string(cursor)?.to_owned();
    let requirement_owner_package_identity = recover_optional(cursor, recover_identity)?;
    let requirement_identity = cursor_string(cursor)?.to_owned();
    let parameter_count = usize::try_from(cursor.u64()?).ok()?;
    let parameter_type_identities =
        cursor_vec(cursor, |cursor| cursor_string(cursor).map(str::to_owned))?;
    let entry_claims = cursor_vec(cursor, |cursor| {
        let parameter_index = usize::try_from(cursor.u64()?).ok()?;
        let carrier_identity = cursor_string(cursor)?.to_owned();
        let domain = cursor_string(cursor)?.to_owned();
        let predicate_body = recover_predicate_body(cursor)?;
        let effective_carry = recover_carry(cursor)?;
        let authority_flow = match cursor_string(cursor)? {
            "accepts" => ServiceEntryAuthorityFlow::Accepts,
            _ => return None,
        };
        Some(ServiceEntryClaim {
            parameter_index,
            carrier_identity,
            domain,
            predicate_body,
            effective_carry,
            authority_flow,
        })
    })?;
    let has_result = cursor_bool(cursor)?;
    let result_type_identity =
        recover_optional(cursor, |cursor| cursor_string(cursor).map(str::to_owned))?;
    let result_claims = cursor_vec(cursor, |cursor| {
        Some(ServiceResultClaim {
            domain: cursor_string(cursor)?.to_owned(),
            effective_carry: recover_carry(cursor)?,
        })
    })?;
    let service_reach = cursor_vec(cursor, |cursor| cursor_string(cursor).map(str::to_owned))?;
    let synchronous_invocations =
        cursor_vec(cursor, |cursor| cursor_string(cursor).map(str::to_owned))?;
    let may_suspend = cursor_bool(cursor)?;
    let may_block = cursor_bool(cursor)?;
    let terminates_guarantee = cursor_bool(cursor)?;
    let termination_premises = cursor_vec(cursor, |cursor| {
        let profile = cursor_string(cursor)?.to_owned();
        let subject = match cursor.u8()? {
            0 => ServiceProgressSubject::ProviderReceiver,
            1 => ServiceProgressSubject::Parameter(usize::try_from(cursor.u64()?).ok()?),
            _ => return None,
        };
        let subject_projections =
            cursor_vec(cursor, |cursor| cursor_string(cursor).map(str::to_owned))?;
        let establishment_routes = cursor_vec(cursor, |cursor| {
            let kind = match cursor.u8()? {
                0 => ServiceProgressEstablishmentRouteKind::CheckedRequirement,
                1 => ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                _ => return None,
            };
            Some(ServiceProgressEstablishmentRoute {
                kind,
                requirement_identity: cursor_string(cursor)?.to_owned(),
            })
        })?;
        Some(ServiceProgressPremise {
            profile,
            subject,
            subject_projections,
            establishment_routes,
        })
    })?;
    let calling_plan_report_fingerprint = recover_optional(cursor, |cursor| {
        Some(u64::from_le_bytes(cursor.bytes(8)?.try_into().ok()?))
    })?;
    let calling_plan_commitment = recover_optional(cursor, |cursor| {
        Some(BoundaryCallingPlanCommitment::from_digest(
            cursor.framed()?.try_into().ok()?,
        ))
    })?;
    match (calling_plan_report_fingerprint, calling_plan_commitment) {
        (Some(_), None) | (None, Some(_)) => return None,
        _ => {}
    }
    Some(ServiceMethod {
        name,
        requirement_owner,
        requirement_owner_package_identity,
        requirement_identity,
        parameter_count,
        parameter_type_identities,
        entry_claims,
        has_result,
        result_type_identity,
        result_claims,
        service_reach,
        synchronous_invocations,
        may_suspend,
        may_block,
        terminates_guarantee,
        termination_premises,
        calling_plan_report_fingerprint,
        calling_plan_commitment,
    })
}

fn encode_service_schema(schema: &ServiceSchema, out: &mut Vec<u8>) {
    encode_string(&schema.trait_name, out);
    encode_optional(
        schema.trait_package_identity.as_ref(),
        out,
        |identity, out| encode_identity(*identity, out),
    );
    out.extend_from_slice(&(schema.methods.len() as u64).to_le_bytes());
    for method in &schema.methods {
        encode_service_method(method, out);
    }
}

fn recover_service_schema(cursor: &mut ObservationCursor<'_>) -> Option<ServiceSchema> {
    Some(ServiceSchema {
        trait_name: cursor_string(cursor)?.to_owned(),
        trait_package_identity: recover_optional(cursor, recover_identity)?,
        methods: cursor_vec(cursor, recover_service_method)?,
    })
}

fn encode_candidate(candidate: &SemanticBindingReviewCandidate, out: &mut Vec<u8>) {
    encode_binding(candidate.binding(), out);
    encode_service_schema(candidate.service_schema(), out);
}

fn recover_candidate(cursor: &mut ObservationCursor<'_>) -> Option<SemanticBindingReviewCandidate> {
    let binding = recover_binding(cursor)?;
    let service_schema = recover_service_schema(cursor)?;
    // The constructor's digest invariant is a decode check here: a record
    // whose schema does not normalize to the binding's declared digest is
    // corrupt, not a different valid record.
    if binding.normalized_schema_digest()
        != package_compilation::accepted_service_schema_digest(binding.role(), &service_schema)
    {
        return None;
    }
    Some(SemanticBindingReviewCandidate::new(binding, service_schema))
}

fn encode_dependency_binding(binding: &PackageDependencyBinding, out: &mut Vec<u8>) {
    encode_identity(binding.requester(), out);
    encode_string(binding.alias(), out);
    encode_identity(binding.target(), out);
    out.push(purpose_tag(binding.purpose()));
}

fn recover_dependency_binding(
    cursor: &mut ObservationCursor<'_>,
) -> Option<PackageDependencyBinding> {
    Some(PackageDependencyBinding::for_purpose(
        recover_identity(cursor)?,
        cursor_string(cursor)?,
        recover_identity(cursor)?,
        recover_purpose(cursor)?,
    ))
}

fn encode_generated_source_bundle(bundle: &PackageGeneratedSourceBundle, out: &mut Vec<u8>) {
    encode_identity(bundle.package(), out);
    out.push(purpose_tag(bundle.purpose()));
    encode_target(bundle.target(), out);
    encode_optional(
        bundle.build_execution_profile().as_ref(),
        out,
        |profile, out| encode_target(*profile, out),
    );
    let closure = bundle.dependency_closure();
    encode_identity(closure.root(), out);
    out.push(declaration_kind_tag(closure.root_role()));
    out.extend_from_slice(&(closure.packages().len() as u64).to_le_bytes());
    for package in closure.packages() {
        encode_identity(*package, out);
    }
    out.extend_from_slice(&(closure.dependencies().len() as u64).to_le_bytes());
    for binding in closure.dependencies() {
        encode_dependency_binding(binding, out);
    }
    out.extend_from_slice(&bundle.source_consumption_commitment().digest());
    out.extend_from_slice(&(bundle.sources().len() as u64).to_le_bytes());
    for source in bundle.sources() {
        push_framed(out, source.relative_path());
        push_framed(out, source.bytes());
    }
}

fn recover_generated_source_bundle(
    cursor: &mut ObservationCursor<'_>,
) -> Option<PackageGeneratedSourceBundle> {
    let package = recover_identity(cursor)?;
    let purpose = recover_purpose(cursor)?;
    let target = recover_target(cursor)?;
    let build_execution_profile = recover_optional(cursor, recover_target)?;
    let root = recover_identity(cursor)?;
    let root_role = recover_declaration_kind(cursor)?;
    let packages = cursor_vec(cursor, recover_identity)?;
    let dependencies = cursor_vec(cursor, recover_dependency_binding)?;
    let dependency_closure =
        PackageDependencyClosure::from_canonical_parts(root, root_role, packages, dependencies)
            .ok()?;
    let commitment =
        PackageSourceConsumptionCommitment::from_retained_digest(cursor_digest(cursor)?);
    let sources = cursor_vec(cursor, |cursor| {
        let relative_path = cursor.framed()?.to_vec();
        let bytes = cursor.framed()?;
        Some(build_output::PackageGeneratedSource::from_retained_parts(
            relative_path,
            bytes.into(),
        ))
    })?;
    Some(PackageGeneratedSourceBundle::from_checked(
        package,
        purpose,
        target,
        build_execution_profile,
        dependency_closure,
        commitment,
        sources,
    ))
}

fn encode_consumed_unit(unit: &ConsumedSourceUnit, out: &mut Vec<u8>) {
    match unit.kind() {
        ConsumedSourceUnitKind::PackageAuthored => out.push(0),
        ConsumedSourceUnitKind::PackageGenerated(scope) => {
            out.push(1);
            out.push(match scope {
                source::DependencyScope::Product => 0,
                source::DependencyScope::Build => 1,
            });
        }
        ConsumedSourceUnitKind::ToolchainVirtual => out.push(2),
        ConsumedSourceUnitKind::ToolchainOwned => out.push(3),
    }
    encode_optional(unit.package().as_ref(), out, |identity, out| {
        encode_identity(*identity, out)
    });
    encode_optional(
        unit.toolchain_namespace().as_ref(),
        out,
        |namespace, out| encode_string(namespace, out),
    );
    out.extend_from_slice(&(unit.relative_path().len() as u64).to_le_bytes());
    for component in unit.relative_path() {
        encode_string(component, out);
    }
    out.extend_from_slice(&unit.byte_count().to_le_bytes());
    out.extend_from_slice(&unit.content_digest());
}

fn recover_consumed_unit(cursor: &mut ObservationCursor<'_>) -> Option<ConsumedSourceUnit> {
    let kind = match cursor.u8()? {
        0 => ConsumedSourceUnitKind::PackageAuthored,
        1 => ConsumedSourceUnitKind::PackageGenerated(match cursor.u8()? {
            0 => source::DependencyScope::Product,
            1 => source::DependencyScope::Build,
            _ => return None,
        }),
        2 => ConsumedSourceUnitKind::ToolchainVirtual,
        3 => ConsumedSourceUnitKind::ToolchainOwned,
        _ => return None,
    };
    let package = recover_optional(cursor, recover_identity)?;
    let toolchain_namespace =
        recover_optional(cursor, |cursor| cursor_string(cursor).map(str::to_owned))?;
    let relative_path = cursor_vec(cursor, |cursor| cursor_string(cursor).map(str::to_owned))?;
    let byte_count = cursor.u64()?;
    let content_digest = cursor_digest(cursor)?;
    ConsumedSourceUnit::from_retained(
        kind,
        package,
        toolchain_namespace,
        relative_path,
        byte_count,
        content_digest,
    )
    .ok()
}

fn encode_component_description(description: &IndependentComponentDescription, out: &mut Vec<u8>) {
    encode_identity(description.package(), out);
    let subject = description.expected_subject();
    out.extend_from_slice(&subject.vocabulary_marker.get().to_le_bytes());
    out.extend_from_slice(subject.program_fingerprint.as_bytes());
    push_framed(out, description.description());
}

fn recover_component_description(
    cursor: &mut ObservationCursor<'_>,
) -> Option<IndependentComponentDescription> {
    let package = recover_identity(cursor)?;
    let vocabulary_marker = terminal_psi::VocabularyMarker::new(cursor.u16()?)?;
    let program_fingerprint = terminal_psi::SemanticFingerprint::from_bytes(cursor_digest(cursor)?);
    let description = cursor.framed()?.to_vec();
    Some(IndependentComponentDescription::new(
        package,
        terminal_psi::TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint,
        },
        description,
    ))
}

/// Encode one record's payload.
fn encode_record(review: &CachedCompiledReview, out: &mut Vec<u8>) {
    out.extend_from_slice(RECORD_MAGIC);
    out.extend_from_slice(&RECORD_VERSION.to_le_bytes());
    out.extend_from_slice(&review.package_identity.digest());
    encode_string(&review.name, out);
    out.extend_from_slice(&review.lineage);
    out.extend_from_slice(&review.resolution);
    out.push(purpose_tag(review.purpose));
    encode_target(review.target, out);
    encode_optional(
        review.build_execution_profile.as_ref(),
        out,
        |profile, out| encode_target(*profile, out),
    );
    encode_string(&review.selected_build_machine_identity, out);
    encode_optional(review.build_evaluation_usage.as_ref(), out, |usage, out| {
        let mut framed = Vec::new();
        retained_review_codec::encode_build_evaluation_usage(usage, &mut framed);
        push_framed(out, &framed);
    });
    encode_optional(
        review.build_observation_summary.as_ref(),
        out,
        |summary, out| {
            let mut framed = Vec::new();
            retained_review_codec::encode_observation_summary(summary, &mut framed);
            push_framed(out, &framed);
        },
    );
    out.extend_from_slice(&(review.restricted_build_requests.len() as u64).to_le_bytes());
    for request in &review.restricted_build_requests {
        let mut framed = Vec::new();
        retained_review_codec::encode_restricted_build_request(request, &mut framed);
        push_framed(out, &framed);
    }
    out.extend_from_slice(&(review.semantic_bindings.len() as u64).to_le_bytes());
    for binding in &review.semantic_bindings {
        encode_binding(binding, out);
    }
    out.extend_from_slice(&(review.semantic_binding_candidates.len() as u64).to_le_bytes());
    for candidate in &review.semantic_binding_candidates {
        encode_candidate(candidate, out);
    }
    encode_generated_source_bundle(&review.generated_source_bundle, out);
    push_framed(out, &review.canonical_review_bytes);
    out.extend_from_slice(&(review.canonical_row_bytes.len() as u64).to_le_bytes());
    for row in &review.canonical_row_bytes {
        push_framed(out, row);
    }
    push_framed(out, &review.policy_bytes);
    push_framed(out, &review.obligation_ledger_bytes_payload);
    out.push(u8::from(review.facts.tolerated_unsettled_service_fields));
    out.extend_from_slice(&(review.facts.consumed_units.len() as u64).to_le_bytes());
    for unit in &review.facts.consumed_units {
        encode_consumed_unit(unit, out);
    }
    out.extend_from_slice(&review.facts.source_consumption_commitment.digest());
    encode_optional(
        review.facts.component_description.as_ref(),
        out,
        encode_component_description,
    );
    encode_optional(
        review.facts.target_entry_candidate.as_ref(),
        out,
        encode_candidate,
    );
    out.extend_from_slice(&review.policy_canonical_bytes.to_le_bytes());
    out.extend_from_slice(&review.obligation_ledger_bytes.to_le_bytes());
}

fn decode_record(bytes: &[u8]) -> Option<CachedCompiledReview> {
    if bytes.len() as u64 > DEFAULT_MAX_RECORD_BYTES + 64 {
        return None;
    }
    if bytes.len() < 8 + 2 + 32 {
        return None;
    }
    let (payload, digest) = bytes.split_at(bytes.len() - 32);
    let expected: [u8; 32] = Sha256::digest(payload).into();
    if expected != digest {
        return None;
    }
    let mut cursor = ObservationCursor::new(payload);
    if cursor.bytes(8)? != RECORD_MAGIC {
        return None;
    }
    if cursor.u16()? != RECORD_VERSION {
        return None;
    }
    let package_identity = recover_identity(&mut cursor)?;
    let name = cursor_string(&mut cursor)?.to_owned();
    let lineage = cursor_digest(&mut cursor)?;
    let resolution = cursor_digest(&mut cursor)?;
    let purpose = recover_purpose(&mut cursor)?;
    let target = recover_target(&mut cursor)?;
    let build_execution_profile = recover_optional(&mut cursor, recover_target)?;
    let selected_build_machine_identity = cursor_string(&mut cursor)?.to_owned();
    let build_evaluation_usage = recover_optional(&mut cursor, |cursor| {
        retained_review_codec::recover_build_evaluation_usage(&mut ObservationCursor::new(
            cursor.framed()?,
        ))
    })?;
    let build_observation_summary = recover_optional(&mut cursor, |cursor| {
        retained_review_codec::recover_observation_summary(&mut ObservationCursor::new(
            cursor.framed()?,
        ))
    })?;
    let restricted_build_requests = cursor_vec(&mut cursor, |cursor| {
        retained_review_codec::recover_restricted_build_request(&mut ObservationCursor::new(
            cursor.framed()?,
        ))
    })?;
    let semantic_bindings = cursor_vec(&mut cursor, recover_binding)?;
    let semantic_binding_candidates = cursor_vec(&mut cursor, recover_candidate)?;
    let generated_source_bundle = recover_generated_source_bundle(&mut cursor)?;
    let canonical_review_bytes = cursor.framed()?.to_vec();
    let canonical_row_bytes = cursor_vec(&mut cursor, |cursor| Some(cursor.framed()?.to_vec()))?;
    let policy_bytes = cursor.framed()?.to_vec();
    let obligation_ledger_bytes_payload = cursor.framed()?.to_vec();
    let tolerated_unsettled_service_fields = cursor_bool(&mut cursor)?;
    let consumed_units = cursor_vec(&mut cursor, recover_consumed_unit)?;
    let source_consumption_commitment =
        PackageSourceConsumptionCommitment::from_retained_digest(cursor_digest(&mut cursor)?);
    let component_description = recover_optional(&mut cursor, recover_component_description)?;
    let target_entry_candidate = recover_optional(&mut cursor, recover_candidate)?;
    let policy_canonical_bytes = cursor.u64()?;
    let obligation_ledger_bytes = cursor.u64()?;
    if !cursor.exhausted() {
        return None;
    }
    Some(CachedCompiledReview {
        package_identity,
        name,
        lineage,
        resolution,
        purpose,
        target,
        build_execution_profile,
        selected_build_machine_identity,
        build_evaluation_usage,
        build_observation_summary,
        restricted_build_requests,
        semantic_bindings,
        semantic_binding_candidates,
        generated_source_bundle,
        canonical_review_bytes,
        canonical_row_bytes,
        policy_bytes,
        obligation_ledger_bytes_payload,
        facts: CachedOccurrenceFacts {
            tolerated_unsettled_service_fields,
            consumed_units,
            source_consumption_commitment,
            component_description,
            target_entry_candidate,
        },
        policy_canonical_bytes,
        obligation_ledger_bytes,
    })
}

#[cfg(test)]
mod tests;
