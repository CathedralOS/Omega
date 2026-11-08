// Public policy type parameters, machine parameter contracts, and signature telescopes.

use crate::record::accessors::field_projection_accessors;
use crate::record::{
    PackagePolicyCrashRoute, PackagePolicyTermination, PackageReviewCallableContract,
    PackageReviewDataProperties, PackageReviewNominalIdentity,
    PackageReviewPropositionParameterSignature, PackageReviewSignatureParameter,
    PackageReviewSynchronousInvocation, PackageReviewTypeIdentity,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyTypeParameter {
    pub(crate) kind: PackagePolicyTypeParameterKind,
    pub(crate) bounds: PackageReviewDataProperties,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyTypeParameterKind {
    Type,
    Implementation, // Positional code selection linked to the owner's conformance bounds.
    Const(PackageReviewTypeIdentity),
    Value(PackageReviewTypeIdentity), // Runtime-capable value binder preserving authored kind rather than strengthening to `const`.
    Machine(PackagePolicyMachineParameterContract),
    Proposition(PackageReviewPropositionParameterSignature),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyMachineParameterContract {
    RequirementIdentity,
    Nominal {
        trait_identity: PackageReviewNominalIdentity,
        requirement_identity: PackageReviewNominalIdentity,
        trait_arguments: Vec<PackageReviewTypeIdentity>,
    },
    Structural(PackagePolicyMachineParameterSignature),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyMachineParameterSignature {
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackagePolicyTypeParameter>,
    pub(crate) conformance_bounds: Vec<crate::record::PackageReviewConformanceBound>,
    pub(crate) parameters: Vec<PackageReviewSignatureParameter>,
    pub(crate) return_type: Option<PackageReviewTypeIdentity>,
    pub(crate) contracts: Vec<PackageReviewCallableContract>,
    pub(crate) published_crash: Vec<PackagePolicyCrashRoute>,
    pub(crate) service_reach: Vec<PackageReviewNominalIdentity>,
    pub(crate) service_reach_is_installation_bound: bool,
    pub(crate) synchronous_invocations: Vec<PackageReviewSynchronousInvocation>,
    pub(crate) suspends: bool,
    pub(crate) blocks: bool,
    pub(crate) termination: PackagePolicyTermination,
}

impl PackagePolicyTypeParameter {
    field_projection_accessors!(borrowed kind: &PackagePolicyTypeParameterKind);
    field_projection_accessors!(copied bounds: PackageReviewDataProperties);
}
impl PackagePolicyMachineParameterContract {}
impl PackagePolicyMachineParameterSignature {
    field_projection_accessors!(copied lifetime_parameter_count: usize);
    field_projection_accessors!(sliced type_parameters: &[PackagePolicyTypeParameter]);
    field_projection_accessors!(sliced conformance_bounds: &[crate::record::PackageReviewConformanceBound]);
    field_projection_accessors!(sliced parameters: &[PackageReviewSignatureParameter]);
    crate::record::accessors::package_policy_callable_signature_accessors!();
}
