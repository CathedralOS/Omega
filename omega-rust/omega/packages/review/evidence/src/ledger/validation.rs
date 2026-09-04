use super::construction::ordinary_package_obligation_ledger_from_compiler_rows;
use super::model::OrdinaryPackageObligationLedger;
use crate::project_checked_package_review;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

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

fn ledger_mismatch(
    expected: &OrdinaryPackageObligationLedger,
    candidate: &OrdinaryPackageObligationLedger,
) -> String {
    if expected.schema() != candidate.schema() {
        return "ordinary package obligation ledger schema does not match local reconstruction"
            .to_owned();
    }
    if expected.package() != candidate.package() {
        return "ordinary package obligation ledger package identity does not match local reconstruction"
            .to_owned();
    }
    if expected.target() != candidate.target() {
        return "ordinary package obligation ledger target does not match local reconstruction"
            .to_owned();
    }
    if expected.dependency_closure() != candidate.dependency_closure() {
        return "ordinary package obligation ledger dependency closure does not match local reconstruction"
            .to_owned();
    }
    for (index, (expected_row, candidate_row)) in
        expected.rows().iter().zip(candidate.rows()).enumerate()
    {
        if expected_row != candidate_row {
            return format!(
                "ordinary package obligation ledger row {index} ({:?}) does not match local reconstruction",
                expected_row.kind()
            );
        }
    }
    format!(
        "ordinary package obligation ledger row count {} does not match locally reconstructed count {}",
        candidate.rows().len(),
        expected.rows().len()
    )
}
