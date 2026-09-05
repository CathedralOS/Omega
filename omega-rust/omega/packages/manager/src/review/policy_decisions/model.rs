use crate::declarations::BuildDeclarationKind;
use crate::review::{
    PackagePolicyChangeFingerprint, ReviewOnlyRootPolicyDisposition, ReviewOnlyRootRoleContract,
};
use omega_package_evidence::record::PackagePolicyRowKind;
use psi_core::PackageKeyIdentity;

/// Exact semantic subject of a policy obligation. Root-role obligations are
/// independent of row changes and retain their directional compatibility loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackagePolicyDecisionSubject {
    Row {
        kind: PackagePolicyRowKind,
        fingerprint: PackagePolicyChangeFingerprint,
    },
    RootRole {
        baseline_role: BuildDeclarationKind,
        candidate_role: BuildDeclarationKind,
        broken_contract: ReviewOnlyRootRoleContract,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackagePolicyObligationFingerprint(pub(super) [u8; 32]);
impl PackagePolicyObligationFingerprint {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackagePolicyDecisionResolutionFingerprint(pub(super) [u8; 32]);
impl PackagePolicyDecisionResolutionFingerprint {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

/// Issued only for a blocking row or a same-key directional root-role change.
/// Its private fields permit constant-time decision construction after one
/// bounded enumeration, without repeatedly rebuilding the complete set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackagePolicyDecisionObligation {
    pub(super) change_set: PackagePolicyChangeFingerprint,
    pub(super) package: PackageKeyIdentity,
    pub(super) subject: PackagePolicyDecisionSubject,
    pub(super) fingerprint: PackagePolicyObligationFingerprint,
}
impl PackagePolicyDecisionObligation {
    pub const fn change_set(&self) -> PackagePolicyChangeFingerprint {
        self.change_set
    }
    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }
    pub const fn subject(&self) -> PackagePolicyDecisionSubject {
        self.subject
    }
    pub const fn fingerprint(&self) -> PackagePolicyObligationFingerprint {
        self.fingerprint
    }
}

/// Closed accept/reject policy for an exact current comparison obligation.
/// This does not claim that anyone inspected source or completed an audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackagePolicyDecision {
    pub(super) obligation: PackagePolicyDecisionObligation,
    pub(super) disposition: ReviewOnlyRootPolicyDisposition,
}
impl PackagePolicyDecision {
    pub const fn change_set(&self) -> PackagePolicyChangeFingerprint {
        self.obligation.change_set
    }
    pub const fn package(&self) -> PackageKeyIdentity {
        self.obligation.package
    }
    pub const fn obligation(&self) -> &PackagePolicyDecisionObligation {
        &self.obligation
    }
    pub const fn disposition(&self) -> ReviewOnlyRootPolicyDisposition {
        self.disposition
    }
}

/// A complete bijection between current blockers and explicit decisions.
/// Rejection is complete treatment, not permission. An empty resolution is
/// valid only when the current comparison has no blockers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyDecisionResolution {
    pub(super) change_set: PackagePolicyChangeFingerprint,
    pub(super) decisions: Vec<PackagePolicyDecision>,
    pub(super) fingerprint: PackagePolicyDecisionResolutionFingerprint,
    pub(super) all_required_changes_accepted: bool,
}
impl PackagePolicyDecisionResolution {
    pub const fn change_set(&self) -> PackagePolicyChangeFingerprint {
        self.change_set
    }
    pub fn decisions(&self) -> &[PackagePolicyDecision] {
        &self.decisions
    }
    pub const fn fingerprint(&self) -> PackagePolicyDecisionResolutionFingerprint {
        self.fingerprint
    }
    pub const fn all_required_changes_accepted(&self) -> bool {
        self.all_required_changes_accepted
    }
}
