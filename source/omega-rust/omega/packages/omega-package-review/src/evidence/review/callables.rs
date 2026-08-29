use super::super::{
    authority::{
        PackageReviewCapabilityFlow, PackageReviewCrash, PackageReviewInstallationReach,
        PackageReviewMutation, PackageReviewTermination,
    },
    contracts::{
        PackageReviewCallableContract, PackageReviewCallableRole, PackageReviewCallableSupply,
        PackageReviewOperatorRealization, PackageReviewSynchronousInvocation,
    },
    identity::PackageReviewNominalIdentity,
    signatures::{
        PackageReviewCallableConformance, PackageReviewCallableParameter,
        PackageReviewConformanceBound, PackageReviewTypeIdentity, PackageReviewTypeParameter,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPackageCallableReview {
    pub(crate) role: PackageReviewCallableRole,
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) supply: PackageReviewCallableSupply,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackageReviewTypeParameter>,
    pub(crate) conformance_bounds: Vec<PackageReviewConformanceBound>,
    pub(crate) parameters: Vec<PackageReviewCallableParameter>,
    pub(crate) return_type: PackageReviewTypeIdentity,
    pub(crate) conformances: Vec<PackageReviewCallableConformance>,
    pub(crate) operator_realizations: Vec<PackageReviewOperatorRealization>,
    pub(crate) contracts: Vec<PackageReviewCallableContract>,
    /// `Some` preserves a published ceiling, including an explicitly empty
    /// one. `None` is retained for the current ordinary build-machine form;
    /// admission must not silently reinterpret it as a public empty promise.
    pub(crate) declared_service_reach: Option<Vec<PackageReviewNominalIdentity>>,
    pub(crate) checked_service_reach: PackageReviewCheckedServiceReach,
    pub(crate) unresolved_installation_reaches: Vec<PackageReviewInstallationReach>,
    /// `Some` preserves a published direct synchronous-invocation ceiling,
    /// including an explicitly empty one. Targets retain parameter ordinals
    /// or package-qualified service identities, never display strings.
    pub(crate) declared_synchronous_invocations: Option<Vec<PackageReviewSynchronousInvocation>>,
    pub(crate) realized_synchronous_invocations: Vec<PackageReviewSynchronousInvocation>,
    pub(crate) capability_flows: Vec<PackageReviewCapabilityFlow>,
    /// Exact checked operational summary. Published callable surfaces expose
    /// their authored may-ceiling; the build-machine lane may remain inferred.
    pub(crate) checked_may_suspend: bool,
    pub(crate) checked_may_block: bool,
    pub(crate) checked_termination: PackageReviewTermination,
    pub(crate) checked_crash: PackageReviewCrash,
    pub(crate) mutation: Vec<PackageReviewMutation>,
}

/// Whether package review has a checked implementation body from which exact
/// service reach can be reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageReviewCheckedServiceReach {
    NoCheckedBody,
    CheckedBody {
        realized: Vec<PackageReviewNominalIdentity>,
        concrete: Vec<PackageReviewNominalIdentity>,
    },
}

impl PackageReviewCheckedServiceReach {
    pub fn realized(&self) -> Option<&[PackageReviewNominalIdentity]> {
        match self {
            Self::NoCheckedBody => None,
            Self::CheckedBody { realized, .. } => Some(realized),
        }
    }

    pub fn concrete(&self) -> Option<&[PackageReviewNominalIdentity]> {
        match self {
            Self::NoCheckedBody => None,
            Self::CheckedBody { concrete, .. } => Some(concrete),
        }
    }
}

impl CheckedPackageCallableReview {
    pub const fn role(&self) -> PackageReviewCallableRole {
        self.role
    }

    pub fn identity(&self) -> &PackageReviewNominalIdentity {
        &self.identity
    }

    pub const fn supply(&self) -> PackageReviewCallableSupply {
        self.supply
    }

    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }

    pub fn type_parameters(&self) -> &[PackageReviewTypeParameter] {
        &self.type_parameters
    }

    pub fn conformance_bounds(&self) -> &[PackageReviewConformanceBound] {
        &self.conformance_bounds
    }

    pub fn parameters(&self) -> &[PackageReviewCallableParameter] {
        &self.parameters
    }

    pub const fn return_type(&self) -> &PackageReviewTypeIdentity {
        &self.return_type
    }

    pub fn conformances(&self) -> &[PackageReviewCallableConformance] {
        &self.conformances
    }

    pub fn operator_realizations(&self) -> &[PackageReviewOperatorRealization] {
        &self.operator_realizations
    }

    pub fn contracts(&self) -> &[PackageReviewCallableContract] {
        &self.contracts
    }

    pub fn declared_service_reach(&self) -> Option<&[PackageReviewNominalIdentity]> {
        self.declared_service_reach.as_deref()
    }

    pub const fn checked_service_reach(&self) -> &PackageReviewCheckedServiceReach {
        &self.checked_service_reach
    }

    pub fn unresolved_installation_reaches(&self) -> &[PackageReviewInstallationReach] {
        &self.unresolved_installation_reaches
    }

    pub fn declared_synchronous_invocations(
        &self,
    ) -> Option<&[PackageReviewSynchronousInvocation]> {
        self.declared_synchronous_invocations.as_deref()
    }

    pub fn realized_synchronous_invocations(&self) -> &[PackageReviewSynchronousInvocation] {
        &self.realized_synchronous_invocations
    }

    pub fn capability_flows(&self) -> &[PackageReviewCapabilityFlow] {
        &self.capability_flows
    }

    pub const fn checked_may_suspend(&self) -> bool {
        self.checked_may_suspend
    }

    pub const fn checked_may_block(&self) -> bool {
        self.checked_may_block
    }

    pub const fn checked_termination(&self) -> &PackageReviewTermination {
        &self.checked_termination
    }

    pub const fn checked_crash(&self) -> &PackageReviewCrash {
        &self.checked_crash
    }

    pub fn mutation(&self) -> &[PackageReviewMutation] {
        &self.mutation
    }
}
