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
    /// A positional code selection linked to the owner's conformance bounds.
    Implementation,
    Const(PackageReviewTypeIdentity),
    /// A runtime-capable value binder; the public record preserves the
    /// authored kind rather than strengthening it to `const`.
    Value(PackageReviewTypeIdentity),
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
    pub const fn kind(&self) -> &PackagePolicyTypeParameterKind {
        &self.kind
    }
    pub const fn bounds(&self) -> PackageReviewDataProperties {
        self.bounds
    }
}
impl PackagePolicyMachineParameterContract {}
impl PackagePolicyMachineParameterSignature {
    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }
    pub fn type_parameters(&self) -> &[PackagePolicyTypeParameter] {
        &self.type_parameters
    }
    pub fn parameters(&self) -> &[PackageReviewSignatureParameter] {
        &self.parameters
    }
    crate::record::accessors::package_policy_callable_signature_accessors!();
}
