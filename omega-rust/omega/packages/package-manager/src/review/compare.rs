//! Policy comparison between an accepted baseline and a candidate.
//!
//! [`policy`] derives policy row changes, [`locked_policy`] compares a
//! candidate with the accepted lock, and [`risk`] supports triage.

mod locked_policy;
mod policy;
mod risk;

use crate::declarations::BuildDeclarationKind;
use crate::declarations::PackageKey;
pub use locked_policy::{LockedPolicyComparisonError, compare_locked_package_policies};
pub use policy::{
    PackagePolicyChangeError, PackagePolicyChangeFingerprint, PackagePolicyChangeKind,
    PackagePolicyChangeLimits, PackagePolicyChangeSet, PackagePolicyDependencyPath,
    PackagePolicyDependencyPathStep, PackagePolicyPackageChange, PackagePolicyReplacementSite,
    PackagePolicyRowChange, PackagePolicySourceReplacement, compare_package_policy_changes,
};
pub(crate) use risk::{ChangedRowRisk, changed_review_risk};

/// Exact compatibility contract broken by changing the selected project root's
/// authored role without changing its package identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReviewOnlyRootRoleContract {
    DependencyCompatibility,
    ApplicationActivation,
}

impl ReviewOnlyRootRoleContract {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DependencyCompatibility => "dependency-compatibility",
            Self::ApplicationActivation => "application-activation",
        }
    }
}

/// Directional review result for one stable root key whose authored role
/// changed. This is blocking review evidence, not an admission decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewOnlyRootRoleChange {
    root: PackageKey,
    baseline_role: BuildDeclarationKind,
    candidate_role: BuildDeclarationKind,
    broken_contract: ReviewOnlyRootRoleContract,
}

impl ReviewOnlyRootRoleChange {
    pub fn root(&self) -> &PackageKey {
        &self.root
    }

    pub const fn baseline_role(&self) -> BuildDeclarationKind {
        self.baseline_role
    }

    pub const fn candidate_role(&self) -> BuildDeclarationKind {
        self.candidate_role
    }

    pub const fn broken_contract(&self) -> ReviewOnlyRootRoleContract {
        self.broken_contract
    }

    pub const fn is_blocking(&self) -> bool {
        true
    }
}
