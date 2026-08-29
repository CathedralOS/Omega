//! Local reconstruction ledger for ordinary package-review obligations.
//!
//! This mirrors the Terminal obligation-ledger rule at the ordinary package
//! layer: recovered producer rows are inert until the selected local compiler
//! reconstructs the complete current row set from compiler-owned semantic state
//! after successful checking and requires exact equality. The current row
//! vocabulary remains review-only and incomplete for accepted `PackageInstance`
//! evidence; this module does not promote it into a lock or certificate.

use super::{
    DecodedPackageReviewCanonicalRow, PACKAGE_REVIEW_ENCODING_VERSION,
    PACKAGE_REVIEW_ROW_ENCODING_VERSION, PackageReviewCanonicalRow, PackageReviewCanonicalRowKind,
    PackageReviewCanonicalRowRisk,
};
use crate::project_checked_package_review;
use omega_compiler::CheckedCompilation;
use omega_package_compilation::{PackageDependencyBinding, PackageDependencyClosure};
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use sha2::{Digest, Sha256};

const LEDGER_MAGIC: &[u8] = b"OMEGA-ORDINARY-PACKAGE-OBLIGATION-LEDGER\0";
pub const ORDINARY_PACKAGE_OBLIGATION_LEDGER_ENCODING_VERSION: u16 = 1;
pub const ORDINARY_PACKAGE_OBLIGATION_SCHEMA_VERSION: u16 = 1;
const LEDGER_FINGERPRINT_DOMAIN: &[u8] = b"OMEGA-ORDINARY-PACKAGE-OBLIGATION-LEDGER-FINGERPRINT\0";
const MAXIMUM_LEDGER_ENCODING_BYTES: usize = 32 * 1024 * 1024;
const MAXIMUM_LEDGER_TARGET_BYTES: usize = 4 * 1024;
const MAXIMUM_LEDGER_PACKAGES: usize = 65_536;
const MAXIMUM_LEDGER_DEPENDENCIES: usize = 262_144;
const MAXIMUM_LEDGER_ALIAS_BYTES: usize = 1024 * 1024;
const MAXIMUM_LEDGER_ROWS: usize = 65_536;
const MAXIMUM_LEDGER_ROW_BYTES: usize = 4 * 1024 * 1024;
const MAXIMUM_LEDGER_KEY_BYTES: usize = 1024 * 1024;
const MAXIMUM_LEDGER_TOTAL_ROW_BYTES: usize = 16 * 1024 * 1024;

/// Exact semantic vocabulary under which the ordinary obligation question was
/// reconstructed. This is intentionally distinct from both the outer ledger
/// codec and the package-review row encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrdinaryPackageObligationSchemaIdentity {
    version: u16,
}

impl OrdinaryPackageObligationSchemaIdentity {
    pub const fn current() -> Self {
        Self {
            version: ORDINARY_PACKAGE_OBLIGATION_SCHEMA_VERSION,
        }
    }

    pub const fn version(self) -> u16 {
        self.version
    }

    fn decode(version: u16) -> Result<Self, OrdinaryPackageObligationLedgerRecoveryError> {
        if version != ORDINARY_PACKAGE_OBLIGATION_SCHEMA_VERSION {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "unsupported ordinary package obligation schema",
            ));
        }
        Ok(Self { version })
    }
}

/// Collision-resistant identity of one canonical ordinary obligation ledger.
/// It identifies a replay question; it is not a discharge result or package
/// admission.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrdinaryPackageObligationLedgerFingerprint([u8; 32]);

impl OrdinaryPackageObligationLedgerFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for OrdinaryPackageObligationLedgerFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for OrdinaryPackageObligationLedgerFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// One source-handle-free semantic row in the current ordinary package-review
/// vocabulary. Explanatory source coordinates and compiler derivation notes are
/// deliberately separate provenance and do not enter ledger equality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrdinaryPackageObligationRow {
    kind: PackageReviewCanonicalRowKind,
    risk: PackageReviewCanonicalRowRisk,
    key_bytes: Vec<u8>,
    canonical_bytes: Vec<u8>,
}

impl OrdinaryPackageObligationRow {
    pub const fn kind(&self) -> PackageReviewCanonicalRowKind {
        self.kind
    }

    pub const fn risk(&self) -> PackageReviewCanonicalRowRisk {
        self.risk
    }

    pub fn key_bytes(&self) -> &[u8] {
        &self.key_bytes
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

/// Complete locally ordered row set for the current ordinary package-review
/// vocabulary under one exact package, target, and dependency closure.
///
/// This is not yet accepted package evidence: exact source/artifact subjects,
/// certificates, transitive open obligations, schema migration, and local
/// admission decisions remain separate unfinished joins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrdinaryPackageObligationLedger {
    schema: OrdinaryPackageObligationSchemaIdentity,
    package: PackageKeyIdentity,
    target: TargetProfile,
    dependency_closure: PackageDependencyClosure,
    rows: Vec<OrdinaryPackageObligationRow>,
}

impl OrdinaryPackageObligationLedger {
    pub const fn schema(&self) -> OrdinaryPackageObligationSchemaIdentity {
        self.schema
    }

    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }

    pub const fn target(&self) -> TargetProfile {
        self.target
    }

    pub const fn dependency_closure(&self) -> &PackageDependencyClosure {
        &self.dependency_closure
    }

    pub fn rows(&self) -> &[OrdinaryPackageObligationRow] {
        &self.rows
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrdinaryPackageObligationLedgerRecoveryError {
    message: &'static str,
}

impl OrdinaryPackageObligationLedgerRecoveryError {
    const fn new(message: &'static str) -> Self {
        Self { message }
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl std::fmt::Display for OrdinaryPackageObligationLedgerRecoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for OrdinaryPackageObligationLedgerRecoveryError {}

/// Build a candidate ledger from compiler-issued in-memory rows.
///
/// `PackageReviewCanonicalRow` has no public constructor, so this route cannot
/// turn caller-authored bytes into compiler issuance. It is used to place the
/// same local-reconstruction gate directly on fresh review publication.
pub fn ordinary_package_obligation_ledger_from_compiler_rows(
    dependency_closure: PackageDependencyClosure,
    rows: &[PackageReviewCanonicalRow],
) -> Result<OrdinaryPackageObligationLedger, OrdinaryPackageObligationLedgerRecoveryError> {
    validate_row_budget(
        rows.iter()
            .map(|row| (row.key_bytes().len(), row.canonical_bytes().len())),
    )?;
    let first = rows.first().ok_or_else(|| {
        OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger has no rows",
        )
    })?;
    let framing = super::recovery::canonical_row_subject_for_ledger(first.canonical_bytes())
        .map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger row has invalid canonical framing",
            )
        })?;
    let package = framing.0;
    let target = framing.1;
    let ledger_rows = rows
        .iter()
        .map(|row| {
            let (row_package, row_target) = super::recovery::canonical_row_subject_for_ledger(
                row.canonical_bytes(),
            )
            .map_err(|_| {
                OrdinaryPackageObligationLedgerRecoveryError::new(
                    "ordinary package obligation ledger row has invalid canonical framing",
                )
            })?;
            if row_package != package {
                return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                    "ordinary package obligation ledger mixes package identities",
                ));
            }
            if row_target != target {
                return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                    "ordinary package obligation ledger mixes targets",
                ));
            }
            row_from_compiler(row)
        })
        .collect::<Result<Vec<_>, _>>()?;
    finish_ledger(
        OrdinaryPackageObligationSchemaIdentity::current(),
        package,
        target,
        dependency_closure,
        ledger_rows,
    )
}

/// Recover a candidate ledger from individually compiler-decoded row
/// envelopes. Canonical decoding establishes framing only; callers must still
/// invoke [`validate_ordinary_package_obligation_ledger`] against the exact
/// checked source subject.
pub fn recover_ordinary_package_obligation_ledger(
    dependency_closure: PackageDependencyClosure,
    rows: &[DecodedPackageReviewCanonicalRow],
) -> Result<OrdinaryPackageObligationLedger, OrdinaryPackageObligationLedgerRecoveryError> {
    validate_row_budget(
        rows.iter()
            .map(|row| (row.key_bytes().len(), row.canonical_bytes().len())),
    )?;
    let first = rows.first().ok_or_else(|| {
        OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger has no rows",
        )
    })?;
    let package = first.package();
    let target = first.target();
    let mut ledger_rows = Vec::new();
    ledger_rows.try_reserve_exact(rows.len()).map_err(|_| {
        OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger row allocation failed",
        )
    })?;
    for row in rows {
        if row.package() != package {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger mixes package identities",
            ));
        }
        if row.target() != target {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger mixes targets",
            ));
        }
        ledger_rows.push(clone_row(
            row.kind(),
            row.risk(),
            row.key_bytes(),
            row.canonical_bytes(),
        )?);
    }
    finish_ledger(
        OrdinaryPackageObligationSchemaIdentity::current(),
        package,
        target,
        dependency_closure,
        ledger_rows,
    )
}

/// Encode the complete source-path-free replay question in one bounded,
/// canonical frame. The resulting bytes are not a certificate, discharge
/// result, admission decision, package instance, or lock payload.
pub fn encode_ordinary_package_obligation_ledger(
    ledger: &OrdinaryPackageObligationLedger,
) -> Result<Vec<u8>, OrdinaryPackageObligationLedgerRecoveryError> {
    validate_encoding_budget(ledger)?;
    let mut encoder = LedgerEncoder::bounded(MAXIMUM_LEDGER_ENCODING_BYTES);
    encoder.fixed_bytes(LEDGER_MAGIC);
    encoder.u16(ORDINARY_PACKAGE_OBLIGATION_LEDGER_ENCODING_VERSION);
    encoder.u16(ledger.schema.version());
    encoder.u16(PACKAGE_REVIEW_ENCODING_VERSION);
    encoder.u16(PACKAGE_REVIEW_ROW_ENCODING_VERSION);
    encoder.package_identity(ledger.package);
    encoder.string(ledger.target.target_name())?;
    encoder.package_identity(ledger.dependency_closure.root());
    encoder.usize(ledger.dependency_closure.packages().len())?;
    for package in ledger.dependency_closure.packages() {
        encoder.package_identity(*package);
    }
    encoder.usize(ledger.dependency_closure.dependencies().len())?;
    for dependency in ledger.dependency_closure.dependencies() {
        encoder.package_identity(dependency.requester());
        encoder.string(dependency.alias())?;
        encoder.package_identity(dependency.target());
    }
    encoder.usize(ledger.rows.len())?;
    for row in &ledger.rows {
        encoder.bytes(&row.canonical_bytes)?;
    }
    encoder.finish()
}

fn validate_encoding_budget(
    ledger: &OrdinaryPackageObligationLedger,
) -> Result<(), OrdinaryPackageObligationLedgerRecoveryError> {
    if ledger.target.target_name().len() > MAXIMUM_LEDGER_TARGET_BYTES {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger target exceeds its byte ceiling",
        ));
    }
    if ledger.dependency_closure.packages().len() > MAXIMUM_LEDGER_PACKAGES {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger package count exceeds its ceiling",
        ));
    }
    if ledger.dependency_closure.dependencies().len() > MAXIMUM_LEDGER_DEPENDENCIES {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger dependency count exceeds its ceiling",
        ));
    }
    if ledger
        .dependency_closure
        .dependencies()
        .iter()
        .any(|dependency| dependency.alias().len() > MAXIMUM_LEDGER_ALIAS_BYTES)
    {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger alias exceeds its byte ceiling",
        ));
    }
    validate_row_budget(
        ledger
            .rows
            .iter()
            .map(|row| (row.key_bytes.len(), row.canonical_bytes.len())),
    )
}

/// Decode canonical ledger framing. Decoding establishes shape only; callers
/// must still invoke [`validate_ordinary_package_obligation_ledger`] against
/// the exact locally checked source subject.
pub fn decode_ordinary_package_obligation_ledger(
    bytes: &[u8],
) -> Result<OrdinaryPackageObligationLedger, OrdinaryPackageObligationLedgerRecoveryError> {
    if bytes.len() > MAXIMUM_LEDGER_ENCODING_BYTES {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger encoding exceeds its byte ceiling",
        ));
    }
    let mut decoder = LedgerDecoder::new(bytes);
    decoder.fixed_bytes(LEDGER_MAGIC)?;
    if decoder.u16()? != ORDINARY_PACKAGE_OBLIGATION_LEDGER_ENCODING_VERSION {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "unsupported ordinary package obligation ledger encoding version",
        ));
    }
    let schema = OrdinaryPackageObligationSchemaIdentity::decode(decoder.u16()?)?;
    if decoder.u16()? != PACKAGE_REVIEW_ENCODING_VERSION
        || decoder.u16()? != PACKAGE_REVIEW_ROW_ENCODING_VERSION
    {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger names an unsupported review-row vocabulary",
        ));
    }
    let package = decoder.package_identity()?;
    let target_name = decoder.string(MAXIMUM_LEDGER_TARGET_BYTES)?;
    let target = TargetProfile::from_omega_target_name(Some(&target_name)).map_err(|_| {
        OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger contains a noncanonical target",
        )
    })?;
    if target.target_name() != target_name {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger contains a noncanonical target",
        ));
    }

    let closure_root = decoder.package_identity()?;
    let package_count = decoder.count(
        MAXIMUM_LEDGER_PACKAGES,
        "ordinary package obligation ledger package count exceeds its ceiling",
    )?;
    let mut packages = Vec::new();
    packages.try_reserve_exact(package_count).map_err(|_| {
        OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger package allocation failed",
        )
    })?;
    for _ in 0..package_count {
        packages.push(decoder.package_identity()?);
    }

    let dependency_count = decoder.count(
        MAXIMUM_LEDGER_DEPENDENCIES,
        "ordinary package obligation ledger dependency count exceeds its ceiling",
    )?;
    let mut dependencies = Vec::new();
    dependencies
        .try_reserve_exact(dependency_count)
        .map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger dependency allocation failed",
            )
        })?;
    for _ in 0..dependency_count {
        let requester = decoder.package_identity()?;
        let alias = decoder.string(MAXIMUM_LEDGER_ALIAS_BYTES)?;
        let target = decoder.package_identity()?;
        dependencies.push(PackageDependencyBinding::new(requester, alias, target));
    }
    let dependency_closure =
        PackageDependencyClosure::from_canonical_parts(closure_root, packages, dependencies)
            .map_err(OrdinaryPackageObligationLedgerRecoveryError::new)?;

    let row_count = decoder.count(
        MAXIMUM_LEDGER_ROWS,
        "ordinary package obligation ledger row count exceeds its ceiling",
    )?;
    if row_count == 0 {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger has no rows",
        ));
    }
    let mut rows = Vec::new();
    rows.try_reserve_exact(row_count).map_err(|_| {
        OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger row allocation failed",
        )
    })?;
    let mut total_row_bytes = 0usize;
    for _ in 0..row_count {
        let canonical_bytes = decoder.bytes(MAXIMUM_LEDGER_ROW_BYTES)?;
        let framing =
            super::recovery::canonical_row_framing_for_ledger(canonical_bytes).map_err(|_| {
                OrdinaryPackageObligationLedgerRecoveryError::new(
                    "ordinary package obligation ledger contains an invalid canonical row",
                )
            })?;
        if framing.package != package {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger row has a different package identity",
            ));
        }
        if framing.target != target {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger row has a different target",
            ));
        }
        if framing.key_bytes.len() > MAXIMUM_LEDGER_KEY_BYTES {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger row key exceeds its byte ceiling",
            ));
        }
        total_row_bytes = total_row_bytes
            .checked_add(framing.key_bytes.len())
            .and_then(|total| total.checked_add(canonical_bytes.len()))
            .ok_or_else(|| {
                OrdinaryPackageObligationLedgerRecoveryError::new(
                    "ordinary package obligation ledger row-byte accounting overflowed",
                )
            })?;
        if total_row_bytes > MAXIMUM_LEDGER_TOTAL_ROW_BYTES {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger exceeds its total row-byte ceiling",
            ));
        }
        rows.push(OrdinaryPackageObligationRow {
            kind: framing.kind,
            risk: framing.risk,
            key_bytes: framing.key_bytes,
            canonical_bytes: clone_bytes(canonical_bytes)?,
        });
    }
    decoder.finish()?;

    let ledger = finish_ledger(schema, package, target, dependency_closure, rows)?;
    if encode_ordinary_package_obligation_ledger(&ledger)? != bytes {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger encoding is not canonical",
        ));
    }
    Ok(ledger)
}

pub fn ordinary_package_obligation_ledger_fingerprint(
    ledger: &OrdinaryPackageObligationLedger,
) -> Result<OrdinaryPackageObligationLedgerFingerprint, OrdinaryPackageObligationLedgerRecoveryError>
{
    let bytes = encode_ordinary_package_obligation_ledger(ledger)?;
    let mut digest = Sha256::new();
    digest.update(LEDGER_FINGERPRINT_DOMAIN);
    digest.update(
        u64::try_from(bytes.len())
            .expect("bounded ordinary package obligation ledger length fits u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
    Ok(OrdinaryPackageObligationLedgerFingerprint(
        digest.finalize().into(),
    ))
}

/// Reconstruct the complete current ordinary package-review question directly
/// from compiler-owned checked semantics.
pub fn reconstruct_ordinary_package_obligation_ledger(
    compilation: &CheckedCompilation,
) -> Result<OrdinaryPackageObligationLedger, Vec<Diagnostic>> {
    let dependency_closure = compilation.dependency_closure().cloned().ok_or_else(|| {
        vec![Diagnostic::error(
            "ordinary package obligation reconstruction requires a package-aware dependency closure",
        )]
    })?;
    let projection = project_checked_package_review(compilation)?;
    let rows = projection.canonical_rows().map_err(|error| {
        vec![Diagnostic::error(format!(
            "ordinary package obligation reconstruction failed to encode canonical rows: {error}"
        ))]
    })?;
    ordinary_package_obligation_ledger_from_compiler_rows(dependency_closure, &rows).map_err(
        |error| {
            vec![Diagnostic::error(format!(
                "ordinary package obligation reconstruction produced an invalid ledger: {error}"
            ))]
        },
    )
}

/// Reconstruct and compare the complete local ordinary package-review
/// question. Recovery or compiler issuance alone never establishes equality to
/// the exact checked source subject.
pub fn validate_ordinary_package_obligation_ledger(
    ledger: &OrdinaryPackageObligationLedger,
    compilation: &CheckedCompilation,
) -> Result<(), Vec<Diagnostic>> {
    let expected = reconstruct_ordinary_package_obligation_ledger(compilation)?;
    if ledger == &expected {
        return Ok(());
    }
    Err(vec![Diagnostic::error(ledger_mismatch(&expected, ledger))])
}

fn row_from_compiler(
    row: &PackageReviewCanonicalRow,
) -> Result<OrdinaryPackageObligationRow, OrdinaryPackageObligationLedgerRecoveryError> {
    clone_row(
        row.kind(),
        row.risk(),
        row.key_bytes(),
        row.canonical_bytes(),
    )
}

fn clone_row(
    kind: PackageReviewCanonicalRowKind,
    risk: PackageReviewCanonicalRowRisk,
    key_bytes: &[u8],
    canonical_bytes: &[u8],
) -> Result<OrdinaryPackageObligationRow, OrdinaryPackageObligationLedgerRecoveryError> {
    Ok(OrdinaryPackageObligationRow {
        kind,
        risk,
        key_bytes: clone_bytes(key_bytes)?,
        canonical_bytes: clone_bytes(canonical_bytes)?,
    })
}

fn clone_bytes(bytes: &[u8]) -> Result<Vec<u8>, OrdinaryPackageObligationLedgerRecoveryError> {
    let mut cloned = Vec::new();
    cloned.try_reserve_exact(bytes.len()).map_err(|_| {
        OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger byte allocation failed",
        )
    })?;
    cloned.extend_from_slice(bytes);
    Ok(cloned)
}

fn validate_row_budget(
    rows: impl Iterator<Item = (usize, usize)>,
) -> Result<(), OrdinaryPackageObligationLedgerRecoveryError> {
    let mut count = 0usize;
    let mut total = 0usize;
    for (key_bytes, canonical_bytes) in rows {
        count = count.checked_add(1).ok_or_else(|| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger row count overflowed",
            )
        })?;
        if count > MAXIMUM_LEDGER_ROWS {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger exceeds its row-count ceiling",
            ));
        }
        if key_bytes > MAXIMUM_LEDGER_KEY_BYTES || canonical_bytes > MAXIMUM_LEDGER_ROW_BYTES {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger row exceeds its byte ceiling",
            ));
        }
        total = total
            .checked_add(key_bytes)
            .and_then(|total| total.checked_add(canonical_bytes))
            .ok_or_else(|| {
                OrdinaryPackageObligationLedgerRecoveryError::new(
                    "ordinary package obligation ledger byte accounting overflowed",
                )
            })?;
        if total > MAXIMUM_LEDGER_TOTAL_ROW_BYTES {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger exceeds its total row-byte ceiling",
            ));
        }
    }
    Ok(())
}

fn finish_ledger(
    schema: OrdinaryPackageObligationSchemaIdentity,
    package: PackageKeyIdentity,
    target: TargetProfile,
    dependency_closure: PackageDependencyClosure,
    rows: Vec<OrdinaryPackageObligationRow>,
) -> Result<OrdinaryPackageObligationLedger, OrdinaryPackageObligationLedgerRecoveryError> {
    if schema != OrdinaryPackageObligationSchemaIdentity::current() {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "unsupported ordinary package obligation schema",
        ));
    }
    if dependency_closure.root() != package {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger dependency closure has a different root package",
        ));
    }
    if rows
        .windows(2)
        .any(|pair| row_coordinate(&pair[0]) >= row_coordinate(&pair[1]))
    {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger rows are not in strict canonical order",
        ));
    }
    if !rows
        .iter()
        .any(|row| row.kind == PackageReviewCanonicalRowKind::ProjectionHeader)
    {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger has no projection header",
        ));
    }
    if !rows
        .iter()
        .any(|row| row.kind == PackageReviewCanonicalRowKind::SelectedProviderSet)
    {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger has no selected-provider set",
        ));
    }
    Ok(OrdinaryPackageObligationLedger {
        schema,
        package,
        target,
        dependency_closure,
        rows,
    })
}

fn row_coordinate(row: &OrdinaryPackageObligationRow) -> (PackageReviewCanonicalRowKind, &[u8]) {
    (row.kind, &row.key_bytes)
}

fn ledger_mismatch(
    expected: &OrdinaryPackageObligationLedger,
    candidate: &OrdinaryPackageObligationLedger,
) -> String {
    if expected.schema != candidate.schema {
        return "ordinary package obligation ledger schema does not match local reconstruction"
            .to_owned();
    }
    if expected.package != candidate.package {
        return "ordinary package obligation ledger package identity does not match local reconstruction"
            .to_owned();
    }
    if expected.target != candidate.target {
        return "ordinary package obligation ledger target does not match local reconstruction"
            .to_owned();
    }
    if expected.dependency_closure != candidate.dependency_closure {
        return "ordinary package obligation ledger dependency closure does not match local reconstruction"
            .to_owned();
    }
    for (index, (expected_row, candidate_row)) in
        expected.rows.iter().zip(&candidate.rows).enumerate()
    {
        if expected_row != candidate_row {
            return format!(
                "ordinary package obligation ledger row {index} ({:?}) does not match local reconstruction",
                expected_row.kind
            );
        }
    }
    format!(
        "ordinary package obligation ledger row count {} does not match locally reconstructed count {}",
        candidate.rows.len(),
        expected.rows.len()
    )
}

struct LedgerEncoder {
    output: Vec<u8>,
    maximum_bytes: usize,
    exceeded: bool,
}

impl LedgerEncoder {
    fn bounded(maximum_bytes: usize) -> Self {
        Self {
            output: Vec::new(),
            maximum_bytes,
            exceeded: false,
        }
    }

    fn append(&mut self, bytes: &[u8]) {
        if self.exceeded {
            return;
        }
        let Some(required) = self.output.len().checked_add(bytes.len()) else {
            self.exceeded = true;
            return;
        };
        if required > self.maximum_bytes || self.output.try_reserve(bytes.len()).is_err() {
            self.exceeded = true;
            return;
        }
        self.output.extend_from_slice(bytes);
    }

    fn finish(self) -> Result<Vec<u8>, OrdinaryPackageObligationLedgerRecoveryError> {
        if self.exceeded {
            Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger encoding exceeds its byte ceiling",
            ))
        } else {
            Ok(self.output)
        }
    }

    fn fixed_bytes(&mut self, bytes: &[u8]) {
        self.append(bytes);
    }

    fn u16(&mut self, value: u16) {
        self.append(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.append(&value.to_le_bytes());
    }

    fn usize(&mut self, value: usize) -> Result<(), OrdinaryPackageObligationLedgerRecoveryError> {
        self.u64(u64::try_from(value).map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger value exceeds the portable encoding range",
            )
        })?);
        Ok(())
    }

    fn bytes(&mut self, bytes: &[u8]) -> Result<(), OrdinaryPackageObligationLedgerRecoveryError> {
        self.usize(bytes.len())?;
        self.append(bytes);
        Ok(())
    }

    fn string(&mut self, value: &str) -> Result<(), OrdinaryPackageObligationLedgerRecoveryError> {
        self.bytes(value.as_bytes())
    }

    fn package_identity(&mut self, identity: PackageKeyIdentity) {
        self.append(&identity.digest());
    }
}

struct LedgerDecoder<'bytes> {
    bytes: &'bytes [u8],
    position: usize,
}

impl<'bytes> LedgerDecoder<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(
        &mut self,
        count: usize,
    ) -> Result<&'bytes [u8], OrdinaryPackageObligationLedgerRecoveryError> {
        let end = self.position.checked_add(count).ok_or_else(|| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger length frame overflowed",
            )
        })?;
        let value = self.bytes.get(self.position..end).ok_or_else(|| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger encoding is truncated",
            )
        })?;
        self.position = end;
        Ok(value)
    }

    fn fixed_bytes(
        &mut self,
        expected: &[u8],
    ) -> Result<(), OrdinaryPackageObligationLedgerRecoveryError> {
        if self.take(expected.len())? != expected {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger has invalid framing magic",
            ));
        }
        Ok(())
    }

    fn u16(&mut self) -> Result<u16, OrdinaryPackageObligationLedgerRecoveryError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().expect(
            "ordinary package obligation ledger u16 width is fixed",
        )))
    }

    fn u64(&mut self) -> Result<u64, OrdinaryPackageObligationLedgerRecoveryError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().expect(
            "ordinary package obligation ledger u64 width is fixed",
        )))
    }

    fn count(
        &mut self,
        maximum: usize,
        exceeded_message: &'static str,
    ) -> Result<usize, OrdinaryPackageObligationLedgerRecoveryError> {
        let count = usize::try_from(self.u64()?)
            .map_err(|_| OrdinaryPackageObligationLedgerRecoveryError::new(exceeded_message))?;
        if count > maximum {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                exceeded_message,
            ));
        }
        Ok(count)
    }

    fn bytes(
        &mut self,
        maximum: usize,
    ) -> Result<&'bytes [u8], OrdinaryPackageObligationLedgerRecoveryError> {
        let length = self.count(
            maximum,
            "ordinary package obligation ledger byte field exceeds its ceiling",
        )?;
        self.take(length)
    }

    fn string(
        &mut self,
        maximum: usize,
    ) -> Result<String, OrdinaryPackageObligationLedgerRecoveryError> {
        let bytes = self.bytes(maximum)?;
        let value = std::str::from_utf8(bytes).map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger contains invalid UTF-8",
            )
        })?;
        let mut output = String::new();
        output.try_reserve_exact(value.len()).map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger string allocation failed",
            )
        })?;
        output.push_str(value);
        Ok(output)
    }

    fn package_identity(
        &mut self,
    ) -> Result<PackageKeyIdentity, OrdinaryPackageObligationLedgerRecoveryError> {
        let digest: [u8; 32] = self
            .take(32)?
            .try_into()
            .expect("ordinary package obligation ledger package identity width is fixed");
        PackageKeyIdentity::from_digest(digest).ok_or_else(|| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger contains an invalid package identity",
            )
        })
    }

    fn finish(self) -> Result<(), OrdinaryPackageObligationLedgerRecoveryError> {
        if self.position != self.bytes.len() {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger encoding has trailing bytes",
            ));
        }
        Ok(())
    }
}
