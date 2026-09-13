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
    CheckedPackageReviewProjection, PackageReviewCallableSupply, PackageReviewCanonicalRow,
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
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

    results_from_matching_projection(ledger, projection)
}

// Only the validated join above and the fresh, single-owner reconstruction
// below may bypass canonical-row comparison.
fn results_from_matching_projection(
    ledger: &OrdinaryPackageObligationLedger,
    projection: &CheckedPackageReviewProjection,
) -> Result<OrdinaryPackageObligationResultSet, OrdinaryPackageObligationLedgerRecoveryError> {
    let accepted_callables = projection
        .callables()
        .iter()
        .filter(|callable| callable.supply() == PackageReviewCallableSupply::AdmissionClaim);
    let accepted_rows = ledger.rows_of_kind(PackageReviewCanonicalRowKind::AcceptedClaim);
    if accepted_callables.clone().count() != accepted_rows.len() {
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
    for callable in accepted_callables {
        let key = crate::encoding::nominal_row_key(callable.identity()).map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package accepted-claim key encoding failed",
            )
        })?;
        let position = accepted_rows
            .binary_search_by(|row| row.key_bytes().cmp(&key))
            .map_err(|_| {
                OrdinaryPackageObligationLedgerRecoveryError::new(
                    "ordinary package accepted claim does not rejoin its canonical row",
                )
            })?;
        let row = &accepted_rows[position];
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
    let contract_entailment_rows =
        ledger.rows_of_kind(PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation);
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
    for obligation in contract_entailment_obligations {
        // Typed nominal order is not encoded-key order: canonical strings
        // carry length prefixes. Rejoin the exact key, never zip those orders.
        let key =
            crate::encoding::contract_entailment_obligation_row_key(obligation).map_err(|_| {
                OrdinaryPackageObligationLedgerRecoveryError::new(
                    "ordinary package contract-entailment obligation key encoding failed",
                )
            })?;
        let position = contract_entailment_rows.binary_search_by(|row| row.key_bytes().cmp(&key))
            .map_err(|_| OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package contract-entailment obligation does not rejoin its canonical row",
            ))?;
        let row = &contract_entailment_rows[position];
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
    let external_supply_rows =
        ledger.rows_of_kind(PackageReviewCanonicalRowKind::ExternalExecutableSupply);
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
    for supply in external_supplies {
        let key = crate::encoding::external_executable_supply_row_key(supply).map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package external executable-supply key encoding failed",
            )
        })?;
        let position = external_supply_rows
            .binary_search_by(|row| row.key_bytes().cmp(&key))
            .map_err(|_| {
                OrdinaryPackageObligationLedgerRecoveryError::new(
                    "ordinary package external executable supply does not rejoin its canonical row",
                )
            })?;
        let row = &external_supply_rows[position];
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
    let dangerous_authority_rows =
        ledger.rows_of_kind(PackageReviewCanonicalRowKind::DangerousAuthority);
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
    for authority in dangerous_authorities {
        let key = crate::encoding::nominal_row_key(authority.service()).map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package dangerous-authority key encoding failed",
            )
        })?;
        let position = dangerous_authority_rows
            .binary_search_by(|row| row.key_bytes().cmp(&key))
            .map_err(|_| {
                OrdinaryPackageObligationLedgerRecoveryError::new(
                    "ordinary package dangerous authority does not rejoin its canonical row",
                )
            })?;
        let row = &dangerous_authority_rows[position];
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
    let terminal_authority_permission_rows =
        ledger.rows_of_kind(PackageReviewCanonicalRowKind::TerminalAuthorityPermission);
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

/// Fresh review products constructed together from one checked compilation.
/// These remain review findings, not package acceptance or supplied evidence.
pub struct ReconstructedPackageReview {
    pub projection: CheckedPackageReviewProjection,
    pub canonical_rows: Vec<PackageReviewCanonicalRow>,
    pub ledger: OrdinaryPackageObligationLedger,
    pub results: OrdinaryPackageObligationResultSet,
}

/// Reconstruct the result set from one checked package compilation.
pub fn reconstruct_ordinary_package_obligation_results(
    compilation: &compiler::CheckedCompilation,
) -> Result<OrdinaryPackageObligationResultSet, Vec<diagnostics::Diagnostic>> {
    reconstruct_package_review(compilation).map(|review| review.results)
}

/// Project, encode, and construct the ledger once, then join its results and
/// recheck compiler-issued discharge certificates. Callers retain these fresh
/// products instead of reconstructing the same compilation for each output.
///
/// This takes checked semantics, never caller-supplied projections or rows.
/// Validation of supplied evidence still independently reconstructs locally.
pub fn reconstruct_package_review(
    compilation: &compiler::CheckedCompilation,
) -> Result<ReconstructedPackageReview, Vec<diagnostics::Diagnostic>> {
    let projection = crate::project_checked_package_review(compilation)?;
    let canonical_rows = projection.canonical_rows().map_err(|error| {
        vec![diagnostics::Diagnostic::error(format!(
            "ordinary package obligation result reconstruction failed to encode canonical rows: {error}"
        ))]
    })?;
    let dependency_closure = compilation.dependency_closure().cloned().ok_or_else(|| {
        vec![diagnostics::Diagnostic::error(
            "ordinary package obligation result reconstruction requires package dependency closure",
        )]
    })?;
    let ledger = ordinary_package_obligation_ledger_from_compiler_rows(
        dependency_closure,
        &canonical_rows,
    )
    .map_err(|error| {
        vec![diagnostics::Diagnostic::error(format!(
            "ordinary package obligation result reconstruction produced an invalid ledger: {error}"
        ))]
    })?;
    let mut results = results_from_matching_projection(&ledger, &projection).map_err(|error| {
        vec![diagnostics::Diagnostic::error(format!(
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
        vec![diagnostics::Diagnostic::error(format!(
            "ordinary package contract-entailment discharge reconstruction failed: {error}"
        ))]
    })?;
    Ok(ReconstructedPackageReview {
        projection,
        canonical_rows,
        ledger,
        results,
    })
}

fn apply_contract_entailment_assumption_discharges(
    compilation: &compiler::CheckedCompilation,
    projection: &CheckedPackageReviewProjection,
    ledger: &OrdinaryPackageObligationLedger,
    results: &mut OrdinaryPackageObligationResultSet,
) -> Result<(), OrdinaryPackageObligationLedgerRecoveryError> {
    let package = results.package;
    let evidence_rows =
        ledger.rows_of_kind(PackageReviewCanonicalRowKind::ContractEntailmentAssumptionDischarge);
    if evidence_rows.len() != projection.contract_entailment_assumption_discharges().len() {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package contract-entailment discharges are not bijective with their canonical rows",
        ));
    }
    if evidence_rows.is_empty() {
        return Ok(());
    }
    // Nominal projection is compiler work with owned strings. Perform it once
    // per certificate, not once for every candidate discharge comparison.
    let mut certificates = compilation
        .facts
        .proof
        .contract_entailment_assumption_discharges
        .iter()
        .filter(|certificate| {
            compilation
                .symbols
                .symbol_package_identity(certificate.machine_symbol())
                == Some(package)
        })
        .filter_map(|certificate| {
            crate::capture::nominal_identity(compilation, certificate.machine_symbol())
                .ok()
                .map(|callable| (callable, certificate))
        })
        .collect::<Vec<_>>();
    certificates.sort_unstable_by(|(left_callable, left), (right_callable, right)| {
        (
            left_callable,
            left.contract_position(),
            left.fact_position(),
            left.machine_contract_commitment().as_bytes(),
        )
            .cmp(&(
                right_callable,
                right.contract_position(),
                right.fact_position(),
                right.machine_contract_commitment().as_bytes(),
            ))
    });
    // Fresh result construction preserves the projection's canonical
    // obligation order; borrow it throughout validation before partitioning.
    let obligations = &results.open_contract_entailment_obligations;
    let mut selected = Vec::with_capacity(evidence_rows.len());
    for discharge in projection.contract_entailment_assumption_discharges() {
        let key = (
            discharge.obligation.callable(),
            discharge.obligation.contract_position(),
            discharge.obligation.fact_position(),
            discharge.obligation.machine_contract_commitment(),
        );
        let start = certificates.partition_point(|(callable, certificate)| {
            (
                callable,
                certificate.contract_position(),
                certificate.fact_position(),
                certificate.machine_contract_commitment().as_bytes(),
            ) < key
        });
        let mut matching_certificates = certificates[start..]
            .iter()
            .take_while(|(callable, certificate)| {
                (
                    callable,
                    certificate.contract_position(),
                    certificate.fact_position(),
                    certificate.machine_contract_commitment().as_bytes(),
                ) == key
            })
            .map(|(_, certificate)| *certificate)
            .filter(|certificate| {
                certificate.assumptions() == discharge.assumptions()
                    && certificate.goal() == discharge.goal()
                    && certificate.selected_assumption_position()
                        == discharge.selected_assumption_position()
            });
        let certificate = matching_certificates.next();
        let Some(certificate) = certificate.filter(|_| matching_certificates.next().is_none())
        else {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "persisted contract-entailment assumption discharge does not rejoin exactly one compiler certificate",
            ));
        };
        typed_trees_to_checked_trees::recheck_contract_entailment_assumption_discharge(
            &compilation.typed,
            &compilation.facts.contract_plans,
            certificate,
        )
        .map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "compiler-owned contract-entailment assumption certificate failed local recheck",
            )
        })?;
        let start = obligations.partition_point(|open| open.obligation() < discharge.obligation());
        let end = start
            + obligations[start..]
                .partition_point(|open| open.obligation() == discharge.obligation());
        let [open] = &obligations[start..end] else {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "contract-entailment assumption certificate does not rejoin exactly one open obligation",
            ));
        };
        // Both row families encode the same exact obligation coordinate.
        // Reuse the open row's checked key instead of encoding it again.
        let evidence_position = evidence_rows.binary_search_by(|row| row.key_bytes().cmp(open.row().key_bytes()))
            .map_err(|_| OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package contract-entailment discharge does not rejoin its canonical row",
            ))?;
        let evidence_row = &evidence_rows[evidence_position];
        if evidence_row.risk() != PackageReviewCanonicalRowRisk::Blocking {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package contract-entailment discharge evidence is not blocking",
            ));
        }
        selected.push((start, discharge, evidence_row));
    }
    selected.sort_unstable_by_key(|(position, _, _)| *position);
    if selected.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "contract-entailment assumption certificate does not rejoin exactly one open obligation",
        ));
    }
    // Commit the checked selection in one stable partition. Open results keep
    // their original order, without shifting the tail for each discharge.
    let mut selected = selected.into_iter().peekable();
    let mut remaining =
        Vec::with_capacity(results.open_contract_entailment_obligations.len() - selected.len());
    for (position, open) in std::mem::take(&mut results.open_contract_entailment_obligations)
        .into_iter()
        .enumerate()
    {
        let Some((_, discharge, evidence_row)) =
            selected.next_if(|(selected_position, _, _)| *selected_position == position)
        else {
            remaining.push(open);
            continue;
        };
        results.contract_entailment_assumption_discharges.push(
            OrdinaryPackageContractEntailmentAssumptionDischarge {
                obligation: open.obligation,
                row: open.row,
                evidence_row: evidence_row.clone(),
                assumptions: discharge.assumptions().to_vec(),
                goal: discharge.goal().clone(),
                selected_assumption_position: discharge.selected_assumption_position(),
            },
        );
    }
    results.open_contract_entailment_obligations = remaining;
    results
        .contract_entailment_assumption_discharges
        .sort_by(|left, right| left.obligation.cmp(&right.obligation));
    Ok(())
}

/// Require exact equality to a fresh local reconstruction.
pub fn validate_ordinary_package_obligation_results(
    results: &OrdinaryPackageObligationResultSet,
    compilation: &compiler::CheckedCompilation,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let expected = reconstruct_ordinary_package_obligation_results(compilation)?;
    if results == &expected {
        return Ok(());
    }
    Err(vec![diagnostics::Diagnostic::error(
        "ordinary package obligation results do not match local reconstruction",
    )])
}
