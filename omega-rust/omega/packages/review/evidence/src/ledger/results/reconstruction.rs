//! Reconstruction and independent validation of ordinary obligation results.

use super::{
    OrdinaryPackageAcceptedClaimObligation, OrdinaryPackageContractEntailmentAssumptionDischarge,
    OrdinaryPackageContractEntailmentOpenObligation, OrdinaryPackageDangerousAuthorityObligation,
    OrdinaryPackageExternalExecutableSupplyObligation, OrdinaryPackageObligationResultSet,
    OrdinaryPackageTerminalAuthorityPermissionObligation,
};
use crate::ledger::{
    OrdinaryPackageObligationLedger, OrdinaryPackageObligationLedgerRecoveryError,
    ordinary_package_obligation_ledger_from_compiler_rows,
};
use crate::record::{
    CheckedPackageReviewProjection, PackageReviewCallableSupply, PackageReviewCanonicalRowKind,
    PackageReviewCanonicalRowRisk,
};

/// Join one exact locally reconstructed ledger to its typed compiler
/// projection and expose supported opaque claims as open obligations.
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

    let contract_entailment_obligations = projection.contract_entailment_open_obligations();
    let contract_entailment_rows = ledger
        .rows()
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation)
        .collect::<Vec<_>>();
    if contract_entailment_obligations.len() != contract_entailment_rows.len() {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package contract-entailment obligations are not bijective with their canonical rows",
        ));
    }
    let mut open_contract_entailment_obligations = Vec::new();
    open_contract_entailment_obligations
        .try_reserve_exact(contract_entailment_rows.len())
        .map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package contract-entailment result allocation failed",
            )
        })?;
    for (obligation, row) in contract_entailment_obligations
        .iter()
        .zip(contract_entailment_rows)
    {
        if row.risk() != PackageReviewCanonicalRowRisk::Blocking {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package contract-entailment obligation is not blocking",
            ));
        }
        open_contract_entailment_obligations.push(
            OrdinaryPackageContractEntailmentOpenObligation {
                obligation: obligation.clone(),
                row: row.clone(),
            },
        );
    }

    let external_supplies = projection.external_executable_supply();
    let external_supply_rows = ledger
        .rows()
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::ExternalExecutableSupply)
        .collect::<Vec<_>>();
    if external_supplies.len() != external_supply_rows.len() {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package external executable supplies are not bijective with their canonical rows",
        ));
    }

    let mut open_external_executable_supplies = Vec::new();
    open_external_executable_supplies
        .try_reserve_exact(external_supply_rows.len())
        .map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package external executable-supply result allocation failed",
            )
        })?;
    for (supply, row) in external_supplies.iter().zip(external_supply_rows) {
        if row.risk() != PackageReviewCanonicalRowRisk::OpaqueBlocking {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package external executable supply is not opaque blocking",
            ));
        }
        open_external_executable_supplies.push(OrdinaryPackageExternalExecutableSupplyObligation {
            supply: supply.clone(),
            row: row.clone(),
        });
    }

    let dangerous_authorities = projection.dangerous_authorities();
    let dangerous_authority_rows = ledger
        .rows()
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::DangerousAuthority)
        .collect::<Vec<_>>();
    if dangerous_authorities.len() != dangerous_authority_rows.len() {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package dangerous authorities are not bijective with their canonical rows",
        ));
    }

    let mut open_dangerous_authorities = Vec::new();
    open_dangerous_authorities
        .try_reserve_exact(dangerous_authority_rows.len())
        .map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package dangerous-authority result allocation failed",
            )
        })?;
    for (authority, row) in dangerous_authorities.iter().zip(dangerous_authority_rows) {
        if row.risk() != PackageReviewCanonicalRowRisk::Blocking {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package dangerous authority is not blocking",
            ));
        }
        open_dangerous_authorities.push(OrdinaryPackageDangerousAuthorityObligation {
            authority: authority.clone(),
            row: row.clone(),
        });
    }

    let terminal_authority_permissions = projection.terminal_authority_permissions();
    let terminal_authority_permission_rows = ledger
        .rows()
        .iter()
        .filter(|row| row.kind() == PackageReviewCanonicalRowKind::TerminalAuthorityPermission)
        .collect::<Vec<_>>();
    if terminal_authority_permissions.len() != terminal_authority_permission_rows.len() {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package terminal-authority permissions are not bijective with their canonical rows",
        ));
    }

    let mut open_terminal_authority_permissions = Vec::new();
    open_terminal_authority_permissions
        .try_reserve_exact(terminal_authority_permission_rows.len())
        .map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package terminal-authority-permission result allocation failed",
            )
        })?;
    for (permission, row) in terminal_authority_permissions
        .iter()
        .zip(terminal_authority_permission_rows)
    {
        if row.risk() != PackageReviewCanonicalRowRisk::Blocking {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package terminal-authority permission is not blocking",
            ));
        }
        open_terminal_authority_permissions.push(
            OrdinaryPackageTerminalAuthorityPermissionObligation {
                permission: permission.clone(),
                row: row.clone(),
            },
        );
    }

    Ok(OrdinaryPackageObligationResultSet {
        schema: ledger.schema(),
        package: ledger.package(),
        target: ledger.target(),
        dependency_closure: ledger.dependency_closure().clone(),
        open_accepted_claims,
        contract_entailment_assumption_discharges: Vec::new(),
        open_contract_entailment_obligations,
        open_external_executable_supplies,
        open_dangerous_authorities,
        open_terminal_authority_permissions,
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
    let mut results = ordinary_package_obligation_results_from_projection(&ledger, &projection)
        .map_err(|error| {
            vec![psi_diagnostics::Diagnostic::error(format!(
                "ordinary package obligation result reconstruction failed: {error}"
            ))]
        })?;
    apply_contract_entailment_assumption_discharges(
        compilation,
        &projection,
        &ledger,
        &mut results,
    )
    .map_err(|error| {
        vec![psi_diagnostics::Diagnostic::error(format!(
            "ordinary package contract-entailment discharge reconstruction failed: {error}"
        ))]
    })?;
    Ok(results)
}

fn apply_contract_entailment_assumption_discharges(
    compilation: &omega_compiler::CheckedCompilation,
    projection: &CheckedPackageReviewProjection,
    ledger: &OrdinaryPackageObligationLedger,
    results: &mut OrdinaryPackageObligationResultSet,
) -> Result<(), OrdinaryPackageObligationLedgerRecoveryError> {
    let package = results.package;
    let evidence_rows = ledger
        .rows()
        .iter()
        .filter(|row| {
            row.kind() == PackageReviewCanonicalRowKind::ContractEntailmentAssumptionDischarge
        })
        .collect::<Vec<_>>();
    if evidence_rows.len() != projection.contract_entailment_assumption_discharges().len() {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package contract-entailment discharges are not bijective with their canonical rows",
        ));
    }
    for (discharge, evidence_row) in projection
        .contract_entailment_assumption_discharges()
        .iter()
        .zip(evidence_rows)
    {
        if evidence_row.risk() != PackageReviewCanonicalRowRisk::Blocking {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package contract-entailment discharge evidence is not blocking",
            ));
        }
        let matching_certificates = compilation
            .facts
            .proof
            .contract_entailment_assumption_discharges
            .iter()
            .filter(|certificate| {
                compilation
                    .symbols
                    .symbol_package_identity(certificate.machine_symbol())
                    == Some(package)
                    && certificate.contract_position() == discharge.obligation.contract_position()
                    && certificate.fact_position() == discharge.obligation.fact_position()
                    && certificate.machine_contract_commitment().as_bytes()
                        == discharge.obligation.machine_contract_commitment()
                    && certificate.assumptions() == discharge.assumptions()
                    && certificate.goal() == discharge.goal()
                    && certificate.selected_assumption_position()
                        == discharge.selected_assumption_position()
                    && crate::capture::nominal_identity(compilation, certificate.machine_symbol())
                        .is_ok_and(|callable| callable == *discharge.obligation.callable())
            })
            .collect::<Vec<_>>();
        let [certificate] = matching_certificates.as_slice() else {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "persisted contract-entailment assumption discharge does not rejoin exactly one compiler certificate",
            ));
        };
        psi_typed_trees_to_checked_trees::recheck_contract_entailment_assumption_discharge(
            &compilation.typed,
            &compilation.facts.contract_plans,
            certificate,
        )
        .map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "compiler-owned contract-entailment assumption certificate failed local recheck",
            )
        })?;
        let matching_positions = results
            .open_contract_entailment_obligations
            .iter()
            .enumerate()
            .filter_map(|(position, open)| {
                let obligation = open.obligation();
                (obligation == discharge.obligation()).then_some(position)
            })
            .collect::<Vec<_>>();
        let [position] = matching_positions.as_slice() else {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "contract-entailment assumption certificate does not rejoin exactly one open obligation",
            ));
        };
        let open = results
            .open_contract_entailment_obligations
            .remove(*position);
        results.contract_entailment_assumption_discharges.push(
            OrdinaryPackageContractEntailmentAssumptionDischarge {
                obligation: open.obligation,
                row: open.row,
                evidence_row: (*evidence_row).clone(),
                assumptions: discharge.assumptions().to_vec(),
                goal: discharge.goal().clone(),
                selected_assumption_position: discharge.selected_assumption_position(),
            },
        );
    }
    results
        .contract_entailment_assumption_discharges
        .sort_by(|left, right| left.obligation.cmp(&right.obligation));
    Ok(())
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
