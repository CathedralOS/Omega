//! Locally reconstructed results for supported ordinary obligation lanes.

use super::{
    OrdinaryPackageObligationLedger, OrdinaryPackageObligationLedgerRecoveryError,
    OrdinaryPackageObligationRow, OrdinaryPackageObligationSchemaIdentity,
    ordinary_package_obligation_ledger_from_compiler_rows,
};
use crate::record::{
    CheckedPackageCallableReview, CheckedPackageReviewProjection, PackageReviewCallableSupply,
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk,
    PackageReviewContractEntailmentOpenObligation, PackageReviewDangerousAuthority,
    PackageReviewExternalExecutableSupply, PackageReviewTerminalAuthorityPermission,
};
use omega_package_compilation::PackageDependencyClosure;
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;

/// Closed result status for the supported ordinary package-obligation lanes.
///
/// An accepted claim has no certificate route. It remains explicitly open
/// until the consuming root supplies its own policy decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OrdinaryPackageObligationStatus {
    OpenRootAdmission,
    OpenLaterDischarge,
    Discharged,
}

/// One exact compiler-retained contract obligation for which no current local
/// proof engine issued a discharge. It remains blocking until a concrete later
/// discharge route rechecks the same canonical obligation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrdinaryPackageContractEntailmentOpenObligation {
    obligation: PackageReviewContractEntailmentOpenObligation,
    row: OrdinaryPackageObligationRow,
}

impl OrdinaryPackageContractEntailmentOpenObligation {
    pub const fn obligation(&self) -> &PackageReviewContractEntailmentOpenObligation {
        &self.obligation
    }

    pub const fn row(&self) -> &OrdinaryPackageObligationRow {
        &self.row
    }

    pub const fn status(&self) -> OrdinaryPackageObligationStatus {
        OrdinaryPackageObligationStatus::OpenLaterDischarge
    }
}

/// One exact contract-entailment obligation discharged by a compiler-owned
/// assumption certificate and independently rechecked against the retained
/// checked program.
///
/// This in-memory result replaces compiler-private machine handles with the
/// stable reviewed callable identity carried by `obligation`. It is not yet a
/// persistable package certificate or accepted-lock row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrdinaryPackageContractEntailmentAssumptionDischarge {
    obligation: PackageReviewContractEntailmentOpenObligation,
    row: OrdinaryPackageObligationRow,
    assumptions: Vec<psi_core::Proposition>,
    goal: psi_core::Proposition,
    selected_assumption_position: u32,
}

impl OrdinaryPackageContractEntailmentAssumptionDischarge {
    pub const fn obligation(&self) -> &PackageReviewContractEntailmentOpenObligation {
        &self.obligation
    }

    pub const fn row(&self) -> &OrdinaryPackageObligationRow {
        &self.row
    }

    pub fn assumptions(&self) -> &[psi_core::Proposition] {
        &self.assumptions
    }

    pub const fn goal(&self) -> &psi_core::Proposition {
        &self.goal
    }

    pub const fn selected_assumption_position(&self) -> u32 {
        self.selected_assumption_position
    }

    pub const fn status(&self) -> OrdinaryPackageObligationStatus {
        OrdinaryPackageObligationStatus::Discharged
    }
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

/// One exact opaque executable-supply disclosure reconstructed from checked
/// compiler state.
///
/// The typed supply retains the callable, requirement application, and
/// external binding. The matching canonical row binds that disclosure to the
/// ordinary obligation schema. Neither field establishes implementation
/// correctness or records an admission decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrdinaryPackageExternalExecutableSupplyObligation {
    supply: PackageReviewExternalExecutableSupply,
    row: OrdinaryPackageObligationRow,
}

impl OrdinaryPackageExternalExecutableSupplyObligation {
    pub const fn supply(&self) -> &PackageReviewExternalExecutableSupply {
        &self.supply
    }

    pub const fn row(&self) -> &OrdinaryPackageObligationRow {
        &self.row
    }

    pub const fn status(&self) -> OrdinaryPackageObligationStatus {
        OrdinaryPackageObligationStatus::OpenRootAdmission
    }
}

/// One exact dangerous authority disclosure reconstructed from checked
/// compiler state.
///
/// This retains the compiler-classified service authority and its canonical
/// row. It does not grant that authority, establish final-artifact use, or
/// record an audit decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrdinaryPackageDangerousAuthorityObligation {
    authority: PackageReviewDangerousAuthority,
    row: OrdinaryPackageObligationRow,
}

/// One exact consumer-supplied terminal-authority permission reconstructed
/// from checked compiler state.
///
/// The row is an open root-admission obligation: package review records the
/// grant but neither proves exercise nor accepts any physical terminal leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrdinaryPackageTerminalAuthorityPermissionObligation {
    permission: PackageReviewTerminalAuthorityPermission,
    row: OrdinaryPackageObligationRow,
}

impl OrdinaryPackageTerminalAuthorityPermissionObligation {
    pub const fn permission(&self) -> &PackageReviewTerminalAuthorityPermission {
        &self.permission
    }

    pub const fn row(&self) -> &OrdinaryPackageObligationRow {
        &self.row
    }

    pub const fn status(&self) -> OrdinaryPackageObligationStatus {
        OrdinaryPackageObligationStatus::OpenRootAdmission
    }
}

impl OrdinaryPackageDangerousAuthorityObligation {
    pub const fn authority(&self) -> &PackageReviewDangerousAuthority {
        &self.authority
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
/// This is intentionally in-memory and contains only supported explicit open
/// obligations. It cannot issue a `PackageInstance`, accepted lock row, or
/// producer admission decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrdinaryPackageObligationResultSet {
    schema: OrdinaryPackageObligationSchemaIdentity,
    package: PackageKeyIdentity,
    target: TargetProfile,
    dependency_closure: PackageDependencyClosure,
    open_accepted_claims: Vec<OrdinaryPackageAcceptedClaimObligation>,
    contract_entailment_assumption_discharges:
        Vec<OrdinaryPackageContractEntailmentAssumptionDischarge>,
    open_contract_entailment_obligations: Vec<OrdinaryPackageContractEntailmentOpenObligation>,
    open_external_executable_supplies: Vec<OrdinaryPackageExternalExecutableSupplyObligation>,
    open_dangerous_authorities: Vec<OrdinaryPackageDangerousAuthorityObligation>,
    open_terminal_authority_permissions: Vec<OrdinaryPackageTerminalAuthorityPermissionObligation>,
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

    pub fn open_contract_entailment_obligations(
        &self,
    ) -> &[OrdinaryPackageContractEntailmentOpenObligation] {
        &self.open_contract_entailment_obligations
    }

    pub fn contract_entailment_assumption_discharges(
        &self,
    ) -> &[OrdinaryPackageContractEntailmentAssumptionDischarge] {
        &self.contract_entailment_assumption_discharges
    }

    pub fn open_external_executable_supplies(
        &self,
    ) -> &[OrdinaryPackageExternalExecutableSupplyObligation] {
        &self.open_external_executable_supplies
    }

    pub fn open_dangerous_authorities(&self) -> &[OrdinaryPackageDangerousAuthorityObligation] {
        &self.open_dangerous_authorities
    }

    pub fn open_terminal_authority_permissions(
        &self,
    ) -> &[OrdinaryPackageTerminalAuthorityPermissionObligation] {
        &self.open_terminal_authority_permissions
    }
}

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
    apply_contract_entailment_assumption_discharges(compilation, &mut results).map_err(
        |error| {
            vec![psi_diagnostics::Diagnostic::error(format!(
                "ordinary package contract-entailment discharge reconstruction failed: {error}"
            ))]
        },
    )?;
    Ok(results)
}

fn apply_contract_entailment_assumption_discharges(
    compilation: &omega_compiler::CheckedCompilation,
    results: &mut OrdinaryPackageObligationResultSet,
) -> Result<(), OrdinaryPackageObligationLedgerRecoveryError> {
    let package = results.package;
    for certificate in &compilation
        .facts
        .proof
        .contract_entailment_assumption_discharges
    {
        if compilation
            .symbols
            .symbol_package_identity(certificate.machine_symbol())
            != Some(package)
        {
            continue;
        }
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
        let callable = crate::capture::nominal_identity(compilation, certificate.machine_symbol())
            .map_err(|_| {
                OrdinaryPackageObligationLedgerRecoveryError::new(
                    "contract-entailment assumption certificate has no stable callable identity",
                )
            })?;
        let commitment = certificate.machine_contract_commitment().as_bytes();
        let matching_positions = results
            .open_contract_entailment_obligations
            .iter()
            .enumerate()
            .filter_map(|(position, open)| {
                let obligation = open.obligation();
                (obligation.callable() == &callable
                    && obligation.contract_position() == certificate.contract_position()
                    && obligation.fact_position() == certificate.fact_position()
                    && obligation.machine_contract_commitment() == commitment)
                    .then_some(position)
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
                assumptions: certificate.assumptions().to_vec(),
                goal: certificate.goal().clone(),
                selected_assumption_position: certificate.selected_assumption_position(),
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
