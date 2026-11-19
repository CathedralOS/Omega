// Complete normalized callable surfaces and explicitly authored assumptions
// (`PackagePolicyCallables` and `PackagePolicyCallable`).
//
// Checked bodies retain their authored `declared_service_reach` ceiling
// (including explicit empty) or `None` for inference, while bodyless supplies
// retain their governing published ceiling without inferring authority from an
// unknown body. `declared_synchronous_invocations` preserves published direct
// synchronous-invocation ceilings using parameter ordinals or package-qualified
// service identities. `reachable_capability_flows` records flow modes across
// the reachable checked machine closure without exposing private helper
// identities.

mod conformances;
mod getters;
mod reach_dependency;
pub(crate) use conformances::PackagePolicyCallableConformance;
pub(crate) use reach_dependency::PackagePolicyServiceReachDependency;
pub(in crate::record) mod validation;

use crate::record::accessors::field_projection_accessors;
use crate::record::{
    PackagePolicyCapabilityFlow, PackagePolicyCrash, PackagePolicyMutation,
    PackagePolicyTermination, PackagePolicyTypeParameter, PackageReviewCallableContract,
    PackageReviewCallableSupply, PackageReviewCheckedServiceReach, PackageReviewConformanceBound,
    PackageReviewInstallationReach, PackageReviewNominalIdentity, PackageReviewOperatorRealization,
    PackageReviewSignatureParameter, PackageReviewSynchronousInvocation, PackageReviewTypeIdentity,
};
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyCallables {
    pub(crate) package: PackageKeyIdentity,
    pub(crate) target: TargetProfile,
    pub(crate) callables: Vec<PackagePolicyCallable>,
}

impl PackagePolicyCallables {
    field_projection_accessors!(copied package: semantic_vocabulary::PackageKeyIdentity);
    field_projection_accessors!(copied target: target::TargetProfile);
    field_projection_accessors!(sliced callables: &[PackagePolicyCallable]);
}

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
    pub(crate) target_context: crate::record::PackagePolicyTargetContext,
    pub(crate) role: PackagePolicyCallableRole,
    pub(crate) identity: PackageReviewNominalIdentity,
    pub(crate) supply: PackageReviewCallableSupply,
    pub(crate) spelling: Option<language_core::OperatorSpelling>, // Declared token binding, separate from selected operator realizations.
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackagePolicyTypeParameter>,
    pub(crate) conformance_bounds: Vec<PackageReviewConformanceBound>,
    pub(crate) parameters: Vec<PackageReviewSignatureParameter>,
    pub(crate) return_type: Option<PackageReviewTypeIdentity>,
    pub(crate) conformances: Vec<PackagePolicyCallableConformance>,
    pub(crate) operator_realizations: Vec<PackageReviewOperatorRealization>,
    pub(crate) requirement_realizations: Vec<crate::record::PackageReviewRequirementRealization>,
    pub(crate) contracts: Vec<PackageReviewCallableContract>,
    pub(crate) declared_service_reach: Option<Vec<PackageReviewNominalIdentity>>,
    pub(crate) service_reach_dependency: PackagePolicyServiceReachDependency,
    pub(crate) checked_service_reach: PackageReviewCheckedServiceReach,
    pub(crate) unresolved_installation_reaches: Vec<PackageReviewInstallationReach>,
    pub(crate) declared_synchronous_invocations: Option<Vec<PackageReviewSynchronousInvocation>>,
    pub(crate) realized_synchronous_invocations: Vec<PackageReviewSynchronousInvocation>,
    pub(crate) capability_flows: Vec<PackagePolicyCapabilityFlow>,
    pub(crate) reachable_capability_flows: Vec<PackagePolicyCapabilityFlow>,
    pub(crate) checked_may_suspend: bool,
    pub(crate) checked_may_block: bool,
    pub(crate) declared_may_suspend: Option<bool>,
    pub(crate) declared_may_block: Option<bool>,
    pub(crate) declared_termination: Option<PackagePolicyTermination>,
    pub(crate) checked_termination: PackagePolicyTermination,
    pub(crate) checked_crash: PackagePolicyCrash,
    pub(crate) mutation: PackagePolicyMutation,
}
