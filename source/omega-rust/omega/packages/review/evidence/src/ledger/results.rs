//! Locally reconstructed results for the first ordinary obligation lane.

use super::{
    OrdinaryPackageObligationLedger, OrdinaryPackageObligationLedgerRecoveryError,
    OrdinaryPackageObligationRow, OrdinaryPackageObligationSchemaIdentity,
    ordinary_package_obligation_ledger_from_compiler_rows,
};
use crate::record::{
    CheckedPackageCallableReview, CheckedPackageReviewProjection, PackageReviewCallableSupply,
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
};
use omega_package_compilation::PackageDependencyClosure;
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;

/// Closed result vocabulary for the first ordinary package-obligation lane.
///
/// An accepted claim has no certificate route. It remains explicitly open
/// until the consuming root supplies its own policy decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OrdinaryPackageObligationStatus {
    OpenRootAdmission,
}

/// One exact bodyless package claim reconstructed from checked compiler state.
///
/// The typed callable retains the formal contract and signature. The matching
/// canonical row binds it to the ordinary obligation schema. Neither field is
/// a certificate or an admission decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrdinaryPackageAcceptedClaimObligation {
    callable: CheckedPackageCallableReview,
    row: OrdinaryPackageObligationRow,
}

impl OrdinaryPackageAcceptedClaimObligation {
    pub const fn callable(&self) -> &CheckedPackageCallableReview {
        &self.callable
    }

    pub const fn row(&self) -> &OrdinaryPackageObligationRow {
        &self.row
    }

    pub const fn status(&self) -> OrdinaryPackageObligationStatus {
        OrdinaryPackageObligationStatus::OpenRootAdmission
    }
}

/// Locally reconstructed ordinary results for one exact package subject.
///
/// This is intentionally in-memory and contains only explicit open accepted
/// claims. It cannot issue a `PackageInstance`, accepted lock row, or producer
/// admission decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrdinaryPackageObligationResultSet {
    schema: OrdinaryPackageObligationSchemaIdentity,
    package: PackageKeyIdentity,
    target: TargetProfile,
    dependency_closure: PackageDependencyClosure,
    open_accepted_claims: Vec<OrdinaryPackageAcceptedClaimObligation>,
}

impl OrdinaryPackageObligationResultSet {
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

    pub fn open_accepted_claims(&self) -> &[OrdinaryPackageAcceptedClaimObligation] {
        &self.open_accepted_claims
    }
}

/// Join one exact locally reconstructed ledger to its typed compiler
/// projection and expose accepted claims only as open obligations.
pub fn ordinary_package_obligation_results_from_projection(
    ledger: &OrdinaryPackageObligationLedger,
    projection: &CheckedPackageReviewProjection,
) -> Result<OrdinaryPackageObligationResultSet, OrdinaryPackageObligationLedgerRecoveryError> {
    if ledger.package() != projection.package() {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation result package does not match its projection",
        ));
    }
    if ledger.target() != projection.target() {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation result target does not match its projection",
        ));
    }

    let projected_rows = projection.canonical_rows().map_err(|_| {
        OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation result could not reconstruct canonical rows",
        )
    })?;
    if projected_rows.len() != ledger.rows().len()
        || !projected_rows
            .iter()
            .zip(ledger.rows())
            .all(|(projected, retained)| {
                projected.kind() == retained.kind()
                    && projected.risk() == retained.risk()
                    && projected.key_bytes() == retained.key_bytes()
                    && projected.canonical_bytes() == retained.canonical_bytes()
            })
    {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation result rows do not match local reconstruction",
        ));
    }

    let accepted_callables = projection
        .callables()
        .iter()
        .filter(|callable| callable.supply() == PackageReviewCallableSupply::AdmissionClaim)
        .collect::<Vec<_>>();
    let accepted_rows = ledger
        .rows()
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::AcceptedClaim)
        .collect::<Vec<_>>();
    if accepted_callables.len() != accepted_rows.len() {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package accepted claims are not bijective with their canonical rows",
        ));
    }

    let mut open_accepted_claims = Vec::new();
    open_accepted_claims
        .try_reserve_exact(accepted_rows.len())
        .map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package accepted-claim result allocation failed",
            )
        })?;
    for (callable, row) in accepted_callables.into_iter().zip(accepted_rows) {
        if row.risk() != PackageReviewCanonicalRowRisk::Blocking {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package accepted claim is not blocking",
            ));
        }
        open_accepted_claims.push(OrdinaryPackageAcceptedClaimObligation {
            callable: callable.clone(),
            row: row.clone(),
        });
    }

    Ok(OrdinaryPackageObligationResultSet {
        schema: ledger.schema(),
        package: ledger.package(),
        target: ledger.target(),
        dependency_closure: ledger.dependency_closure().clone(),
        open_accepted_claims,
    })
}

/// Reconstruct the result set from one checked package compilation.
pub fn reconstruct_ordinary_package_obligation_results(
    compilation: &omega_compiler::CheckedCompilation,
) -> Result<OrdinaryPackageObligationResultSet, Vec<psi_diagnostics::Diagnostic>> {
    let projection = crate::project_checked_package_review(compilation)?;
    let canonical_rows = projection.canonical_rows().map_err(|error| {
        vec![psi_diagnostics::Diagnostic::error(format!(
            "ordinary package obligation result reconstruction failed to encode canonical rows: {error}"
        ))]
    })?;
    let dependency_closure = compilation.dependency_closure().cloned().ok_or_else(|| {
        vec![psi_diagnostics::Diagnostic::error(
            "ordinary package obligation result reconstruction requires package dependency closure",
        )]
    })?;
    let ledger = ordinary_package_obligation_ledger_from_compiler_rows(
        dependency_closure,
        &canonical_rows,
    )
    .map_err(|error| {
        vec![psi_diagnostics::Diagnostic::error(format!(
            "ordinary package obligation result reconstruction produced an invalid ledger: {error}"
        ))]
    })?;
    ordinary_package_obligation_results_from_projection(&ledger, &projection).map_err(|error| {
        vec![psi_diagnostics::Diagnostic::error(format!(
            "ordinary package obligation result reconstruction failed: {error}"
        ))]
    })
}

/// Require exact equality to a fresh local reconstruction.
pub fn validate_ordinary_package_obligation_results(
    results: &OrdinaryPackageObligationResultSet,
    compilation: &omega_compiler::CheckedCompilation,
) -> Result<(), Vec<psi_diagnostics::Diagnostic>> {
    let expected = reconstruct_ordinary_package_obligation_results(compilation)?;
    if results == &expected {
        return Ok(());
    }
    Err(vec![psi_diagnostics::Diagnostic::error(
        "ordinary package obligation results do not match local reconstruction",
    )])
}
