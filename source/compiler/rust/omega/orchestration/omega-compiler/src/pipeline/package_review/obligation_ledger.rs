//! Local reconstruction ledger for ordinary package-review obligations.
//!
//! This mirrors the Terminal obligation-ledger rule at the ordinary package
//! layer: recovered producer rows are inert until the selected local compiler
//! reconstructs the complete current row set from compiler-owned semantic state
//! after successful checking and requires exact equality. The current row
//! vocabulary remains review-only and incomplete for accepted `PackageInstance`
//! evidence; this module does not promote it into a lock or certificate.

use super::{
    DecodedPackageReviewCanonicalRow, PackageReviewCanonicalRow, PackageReviewCanonicalRowKind,
    PackageReviewCanonicalRowRisk, project_checked_package_review,
};
use crate::pipeline::{CheckedCompilation, PackageDependencyClosure};
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

const MAXIMUM_LEDGER_ROWS: usize = 65_536;
const MAXIMUM_LEDGER_ROW_BYTES: usize = 4 * 1024 * 1024;
const MAXIMUM_LEDGER_KEY_BYTES: usize = 1024 * 1024;
const MAXIMUM_LEDGER_TOTAL_ROW_BYTES: usize = 16 * 1024 * 1024;

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
    package: PackageKeyIdentity,
    target: TargetProfile,
    dependency_closure: PackageDependencyClosure,
    rows: Vec<OrdinaryPackageObligationRow>,
}

impl OrdinaryPackageObligationLedger {
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
    finish_ledger(package, target, dependency_closure, ledger_rows)
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
    finish_ledger(package, target, dependency_closure, ledger_rows)
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
    package: PackageKeyIdentity,
    target: TargetProfile,
    dependency_closure: PackageDependencyClosure,
    rows: Vec<OrdinaryPackageObligationRow>,
) -> Result<OrdinaryPackageObligationLedger, OrdinaryPackageObligationLedgerRecoveryError> {
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
