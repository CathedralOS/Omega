//! Complete normalized callable surfaces and explicitly authored assumptions.

mod conformances;
mod getters;
pub use conformances::PackagePolicyCallableConformance;
pub(in crate::record) mod validation;

use crate::record::{
    PackagePolicyCapabilityFlow, PackagePolicyCrash, PackagePolicyMutation,
    PackagePolicyTermination, PackagePolicyTypeParameter, PackageReviewCallableContract,
    PackageReviewCallableParameter, PackageReviewCallableSupply, PackageReviewCheckedServiceReach,
    PackageReviewConformanceBound, PackageReviewInstallationReach, PackageReviewNominalIdentity,
    PackageReviewOperatorRealization, PackageReviewSynchronousInvocation,
    PackageReviewTypeIdentity,
};
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;

/// Inert root-activation policy, not a compiler proof or an acceptance decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyCallables {
    pub(crate) package: PackageKeyIdentity,
    pub(crate) target: TargetProfile,
    pub(crate) callables: Vec<PackagePolicyCallable>,
}

impl PackagePolicyCallables {
    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }
    pub const fn target(&self) -> TargetProfile {
        self.target
    }
    pub fn callables(&self) -> &[PackagePolicyCallable] {
        &self.callables
    }
}

/// Private accepted claims remain visible without inventing a public API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyCallableRole {
    Boundary,
    Public,
    Build,
    PrivateAssumption,
    PrivateExternal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyCallable {
    pub(crate) role: PackagePolicyCallableRole,
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) supply: PackageReviewCallableSupply,
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackagePolicyTypeParameter>,
    pub(crate) conformance_bounds: Vec<PackageReviewConformanceBound>,
    pub(crate) parameters: Vec<PackageReviewCallableParameter>,
    pub(crate) return_type: Option<PackageReviewTypeIdentity>,
    pub(crate) conformances: Vec<PackagePolicyCallableConformance>,
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
    pub(crate) capability_flows: Vec<PackagePolicyCapabilityFlow>,
    /// Flow modes across the exact reachable checked machine closure, including
    /// this callable. Private helper identities are not policy coordinates.
    pub(crate) reachable_capability_flows: Vec<PackagePolicyCapabilityFlow>,
    /// Exact checked operational summary. Published callable surfaces expose
    /// their authored may-ceiling; the build-machine lane may remain inferred.
    pub(crate) checked_may_suspend: bool,
    pub(crate) checked_may_block: bool,
    /// Published ceilings remain distinct from the checked body's effects.
    pub(crate) declared_may_suspend: Option<bool>,
    pub(crate) declared_may_block: Option<bool>,
    pub(crate) declared_termination: Option<PackagePolicyTermination>,
    pub(crate) checked_termination: PackagePolicyTermination,
    pub(crate) checked_crash: PackagePolicyCrash,
    pub(crate) mutation: PackagePolicyMutation,
}
