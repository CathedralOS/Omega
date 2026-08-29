use super::limits::{
    MAXIMUM_LEDGER_ALIAS_BYTES, MAXIMUM_LEDGER_DEPENDENCIES, MAXIMUM_LEDGER_KEY_BYTES,
    MAXIMUM_LEDGER_PACKAGES, MAXIMUM_LEDGER_ROW_BYTES, MAXIMUM_LEDGER_ROWS,
    MAXIMUM_LEDGER_TARGET_BYTES, MAXIMUM_LEDGER_TOTAL_ROW_BYTES,
};
use super::model::{
    OrdinaryPackageObligationLedger, OrdinaryPackageObligationLedgerRecoveryError,
    OrdinaryPackageObligationRow, OrdinaryPackageObligationSchemaIdentity,
};
use crate::encoding::{DecodedPackageReviewCanonicalRow, canonical_row_subject_for_ledger};
use crate::evidence::{
    PackageReviewCanonicalRow, PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
};
use omega_package_compilation::PackageDependencyClosure;
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;

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
    let (package, target) =
        canonical_row_subject_for_ledger(first.canonical_bytes()).map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger row has invalid canonical framing",
            )
        })?;
    let ledger_rows = rows
        .iter()
        .map(|row| {
            let (row_package, row_target) = canonical_row_subject_for_ledger(row.canonical_bytes())
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
/// invoke [`super::validate_ordinary_package_obligation_ledger`] against the
/// exact checked source subject.
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

pub(super) fn validate_encoding_budget(
    ledger: &OrdinaryPackageObligationLedger,
) -> Result<(), OrdinaryPackageObligationLedgerRecoveryError> {
    if ledger.target().target_name().len() > MAXIMUM_LEDGER_TARGET_BYTES {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger target exceeds its byte ceiling",
        ));
    }
    if ledger.dependency_closure().packages().len() > MAXIMUM_LEDGER_PACKAGES {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger package count exceeds its ceiling",
        ));
    }
    if ledger.dependency_closure().dependencies().len() > MAXIMUM_LEDGER_DEPENDENCIES {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger dependency count exceeds its ceiling",
        ));
    }
    if ledger
        .dependency_closure()
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
            .rows()
            .iter()
            .map(|row| (row.key_bytes().len(), row.canonical_bytes().len())),
    )
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
    Ok(OrdinaryPackageObligationRow::from_parts(
        kind,
        risk,
        clone_bytes(key_bytes)?,
        clone_bytes(canonical_bytes)?,
    ))
}

pub(super) fn clone_bytes(
    bytes: &[u8],
) -> Result<Vec<u8>, OrdinaryPackageObligationLedgerRecoveryError> {
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

pub(super) fn finish_ledger(
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
        .any(|row| row.kind() == PackageReviewCanonicalRowKind::ProjectionHeader)
    {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger has no projection header",
        ));
    }
    if !rows
        .iter()
        .any(|row| row.kind() == PackageReviewCanonicalRowKind::SelectedProviderSet)
    {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger has no selected-provider set",
        ));
    }
    Ok(OrdinaryPackageObligationLedger::from_parts(
        schema,
        package,
        target,
        dependency_closure,
        rows,
    ))
}

fn row_coordinate(row: &OrdinaryPackageObligationRow) -> (PackageReviewCanonicalRowKind, &[u8]) {
    (row.kind(), row.key_bytes())
}
