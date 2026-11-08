//! Callable behavior meaning without implementation locations or proof receipts.

use super::{
    PackagePolicyServiceProgressRoute, PackageReviewBooleanExpression,
    PackageReviewContractExpression, PackageReviewCrashCause, PackageReviewCrashInterface,
    PackageReviewNominalIdentity, PackageReviewProgressSubject,
    PackageReviewWriteFrameCompleteness,
};
use crate::record::accessors::field_projection_accessors;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyCapabilityFlow {
    pub(crate) capability: PackageReviewNominalIdentity,
    pub(crate) kind: flow_effects::CapabilityFlowKind,
}

impl PackagePolicyCapabilityFlow {
    field_projection_accessors!(borrowed capability: &PackageReviewNominalIdentity);
    field_projection_accessors!(copied kind: flow_effects::CapabilityFlowKind);
    pub(crate) fn compare_canonical(&self, other: &Self) -> std::cmp::Ordering {
        self.capability
            .cmp(&other.capability)
            .then_with(|| self.kind_tag().cmp(&other.kind_tag()))
    }
    fn kind_tag(&self) -> u8 {
        match self.kind {
            flow_effects::CapabilityFlowKind::Uses => 0,
            flow_effects::CapabilityFlowKind::Returns => 1,
            flow_effects::CapabilityFlowKind::Acquires => 2,
            flow_effects::CapabilityFlowKind::Stores => 3,
            flow_effects::CapabilityFlowKind::Derives => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyMutation {
    pub(crate) completeness: PackageReviewWriteFrameCompleteness,
    pub(crate) paths: Vec<String>,
}
impl PackagePolicyMutation {
    field_projection_accessors!(copied completeness: PackageReviewWriteFrameCompleteness);
    field_projection_accessors!(sliced paths: &[String]);
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
    field_projection_accessors!(copied cause: PackageReviewCrashCause);
    field_projection_accessors!(sliced alternative_guards: &[PackagePolicyCrashGuard]);
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
    field_projection_accessors!(copied interface: PackageReviewCrashInterface);
    field_projection_accessors!(sliced published: &[PackagePolicyCrashRoute]);
    pub fn structural_runtime_requirements(&self) -> Option<&[PackageReviewBooleanExpression]> {
        self.structural_runtime_requirements.as_deref()
    }
    field_projection_accessors!(borrowed inferred: &PackagePolicyInferredCrash);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyProgressPremise {
    pub(crate) profile: PackageReviewNominalIdentity,
    pub(crate) subject: PackageReviewProgressSubject,
    pub(crate) projections: Vec<PackageReviewNominalIdentity>,
    pub(crate) establishment_routes: Vec<PackagePolicyServiceProgressRoute>,
}
impl PackagePolicyProgressPremise {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyTermination {
    NoGuarantee,
    Terminates {
        premises: Vec<PackagePolicyProgressPremise>,
    },
}
