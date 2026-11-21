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
pub use reach_dependency::PackagePolicyServiceReachDependency;
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
    pub package: PackageKeyIdentity,
    pub target: TargetProfile,
    pub callables: Vec<PackagePolicyCallable>,
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
    pub target_context: crate::record::PackagePolicyTargetContext,
    pub role: PackagePolicyCallableRole,
    pub identity: PackageReviewNominalIdentity,
    pub supply: PackageReviewCallableSupply,
    pub spelling: Option<language_core::OperatorSpelling>, // Declared token binding, separate from selected operator realizations.
    pub lifetime_parameter_count: usize,
    pub type_parameters: Vec<PackagePolicyTypeParameter>,
    pub conformance_bounds: Vec<PackageReviewConformanceBound>,
    pub parameters: Vec<PackageReviewSignatureParameter>,
    pub return_type: Option<PackageReviewTypeIdentity>,
    pub conformances: Vec<PackagePolicyCallableConformance>,
    pub operator_realizations: Vec<PackageReviewOperatorRealization>,
    pub requirement_realizations: Vec<crate::record::PackageReviewRequirementRealization>,
    pub contracts: Vec<PackageReviewCallableContract>,
    pub declared_service_reach: Option<Vec<PackageReviewNominalIdentity>>,
    pub service_reach_dependency: PackagePolicyServiceReachDependency,
    pub checked_service_reach: PackageReviewCheckedServiceReach,
    pub unresolved_installation_reaches: Vec<PackageReviewInstallationReach>,
    pub declared_synchronous_invocations: Option<Vec<PackageReviewSynchronousInvocation>>,
    pub realized_synchronous_invocations: Vec<PackageReviewSynchronousInvocation>,
    pub capability_flows: Vec<PackagePolicyCapabilityFlow>,
    pub reachable_capability_flows: Vec<PackagePolicyCapabilityFlow>,
    pub checked_may_suspend: bool,
    pub checked_may_block: bool,
    pub declared_may_suspend: Option<bool>,
    pub declared_may_block: Option<bool>,
    pub declared_termination: Option<PackagePolicyTermination>,
    pub checked_termination: PackagePolicyTermination,
    pub checked_crash: PackagePolicyCrash,
    pub mutation: PackagePolicyMutation,
}
