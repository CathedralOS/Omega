//! Locally reconstructed results for supported ordinary obligation lanes.

use super::{OrdinaryPackageObligationRow, OrdinaryPackageObligationSchemaIdentity};
use crate::record::{
    CheckedPackageCallableReview, PackageReviewContractEntailmentOpenObligation,
    PackageReviewDangerousAuthority, PackageReviewExternalExecutableSupply,
    PackageReviewTerminalAuthorityPermission,
};
use omega_package_compilation::PackageDependencyClosure;
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;

mod reconstruction;

pub use reconstruction::{
    ordinary_package_obligation_results_from_projection,
    reconstruct_ordinary_package_obligation_results, validate_ordinary_package_obligation_results,
};

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
/// The result replaces compiler-private machine handles with the stable
/// reviewed callable identity carried by `obligation` and retains the exact
/// canonical `evidence_row`. It is not an accepted lock or admission decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrdinaryPackageContractEntailmentAssumptionDischarge {
    obligation: PackageReviewContractEntailmentOpenObligation,
    row: OrdinaryPackageObligationRow,
    evidence_row: OrdinaryPackageObligationRow,
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

    pub const fn evidence_row(&self) -> &OrdinaryPackageObligationRow {
        &self.evidence_row
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
