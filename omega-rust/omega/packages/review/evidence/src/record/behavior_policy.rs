//! Callable behavior meaning without implementation locations or proof receipts.

use super::{
    PackagePolicyServiceProgressRoute, PackageReviewBooleanExpression,
    PackageReviewContractExpression, PackageReviewCrashCause, PackageReviewCrashInterface,
    PackageReviewNominalIdentity, PackageReviewProgressSubject,
    PackageReviewWriteFrameCompleteness,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyCapabilityFlow {
    pub(crate) capability: PackageReviewNominalIdentity,
    pub(crate) kind: psi_effects::CapabilityFlowKind,
}

impl PackagePolicyCapabilityFlow {
    pub fn capability(&self) -> &PackageReviewNominalIdentity {
        &self.capability
    }
    pub const fn kind(&self) -> psi_effects::CapabilityFlowKind {
        self.kind
    }
    pub(crate) fn compare_canonical(&self, other: &Self) -> std::cmp::Ordering {
        self.capability
            .cmp(&other.capability)
            .then_with(|| self.kind_tag().cmp(&other.kind_tag()))
    }
    fn kind_tag(&self) -> u8 {
        match self.kind {
            psi_effects::CapabilityFlowKind::Uses => 0,
            psi_effects::CapabilityFlowKind::Returns => 1,
            psi_effects::CapabilityFlowKind::Acquires => 2,
            psi_effects::CapabilityFlowKind::Stores => 3,
            psi_effects::CapabilityFlowKind::Derives => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyMutation {
    pub(crate) completeness: PackageReviewWriteFrameCompleteness,
    pub(crate) paths: Vec<String>,
}
impl PackagePolicyMutation {
    pub const fn completeness(&self) -> PackageReviewWriteFrameCompleteness {
        self.completeness
    }
    pub fn paths(&self) -> &[String] {
        &self.paths
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyCrashGuard {
    Truth,
    Expression(PackageReviewContractExpression),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyCrashRoute {
    pub(crate) cause: PackageReviewCrashCause,
    pub(crate) alternative_guards: Vec<PackagePolicyCrashGuard>,
}
impl PackagePolicyCrashRoute {
    pub const fn cause(&self) -> PackageReviewCrashCause {
        self.cause
    }
    pub fn alternative_guards(&self) -> &[PackagePolicyCrashGuard] {
        &self.alternative_guards
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackagePolicyInferredCrash {
    /// The checked local dependency graph did not yield a closed summary.
    Unknown,
    /// A conservative, complete cause set, not proof that each cause occurs.
    Complete {
        causes: Vec<PackageReviewCrashCause>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyCrash {
    pub(crate) interface: PackageReviewCrashInterface,
    pub(crate) published: Vec<PackagePolicyCrashRoute>,
    pub(crate) structural_runtime_requirements: Option<Vec<PackageReviewBooleanExpression>>,
    pub(crate) inferred: PackagePolicyInferredCrash,
}
impl PackagePolicyCrash {
    pub const fn interface(&self) -> PackageReviewCrashInterface {
        self.interface
    }
    pub fn published(&self) -> &[PackagePolicyCrashRoute] {
        &self.published
    }
    pub fn structural_runtime_requirements(&self) -> Option<&[PackageReviewBooleanExpression]> {
        self.structural_runtime_requirements.as_deref()
    }
    pub fn inferred(&self) -> &PackagePolicyInferredCrash {
        &self.inferred
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyProgressPremise {
    pub(crate) profile: PackageReviewNominalIdentity,
    pub(crate) subject: PackageReviewProgressSubject,
    pub(crate) projections: Vec<PackageReviewNominalIdentity>,
    pub(crate) establishment_routes: Vec<PackagePolicyServiceProgressRoute>,
}
impl PackagePolicyProgressPremise {
    pub fn profile(&self) -> &PackageReviewNominalIdentity {
        &self.profile
    }
    pub fn subject(&self) -> &PackageReviewProgressSubject {
        &self.subject
    }
    pub fn projections(&self) -> &[PackageReviewNominalIdentity] {
        &self.projections
    }
    pub fn establishment_routes(&self) -> &[PackagePolicyServiceProgressRoute] {
        &self.establishment_routes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyTermination {
    NoGuarantee,
    Terminates {
        premises: Vec<PackagePolicyProgressPremise>,
    },
}
