use crate::record::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyTypeParameter {
    pub(crate) kind: PackagePolicyTypeParameterKind,
    pub(crate) bounds: PackageReviewDataProperties,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyTypeParameterKind {
    Type,
    Const(PackageReviewTypeIdentity),
    Machine(PackagePolicyMachineParameterContract),
    Proposition(PackageReviewPropositionParameterSignature),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyMachineParameterContract {
    RequirementIdentity,
    Nominal {
        trait_identity: PackageReviewNominalIdentity,
        requirement_identity: PackageReviewNominalIdentity,
    },
    Structural(PackagePolicyMachineParameterSignature),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyMachineParameterSignature {
    pub(crate) lifetime_parameter_count: usize,
    pub(crate) type_parameters: Vec<PackagePolicyTypeParameter>,
    pub(crate) parameters: Vec<PackageReviewMachineParameterValue>,
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
impl PackagePolicyMachineParameterContract {
    pub const fn structural(&self) -> Option<&PackagePolicyMachineParameterSignature> {
        if let Self::Structural(value) = self {
            Some(value)
        } else {
            None
        }
    }
    pub const fn nominal(
        &self,
    ) -> Option<(&PackageReviewNominalIdentity, &PackageReviewNominalIdentity)> {
        if let Self::Nominal {
            trait_identity,
            requirement_identity,
        } = self
        {
            Some((trait_identity, requirement_identity))
        } else {
            None
        }
    }
}
impl PackagePolicyMachineParameterSignature {
    pub const fn lifetime_parameter_count(&self) -> usize {
        self.lifetime_parameter_count
    }
    pub fn type_parameters(&self) -> &[PackagePolicyTypeParameter] {
        &self.type_parameters
    }
    pub fn parameters(&self) -> &[PackageReviewMachineParameterValue] {
        &self.parameters
    }
    pub const fn return_type(&self) -> Option<&PackageReviewTypeIdentity> {
        self.return_type.as_ref()
    }
    pub fn contracts(&self) -> &[PackageReviewCallableContract] {
        &self.contracts
    }
    pub fn published_crash(&self) -> &[PackagePolicyCrashRoute] {
        &self.published_crash
    }
    pub fn service_reach(&self) -> &[PackageReviewNominalIdentity] {
        &self.service_reach
    }
    pub const fn service_reach_is_installation_bound(&self) -> bool {
        self.service_reach_is_installation_bound
    }
    pub fn synchronous_invocations(&self) -> &[PackageReviewSynchronousInvocation] {
        &self.synchronous_invocations
    }
    pub const fn suspends(&self) -> bool {
        self.suspends
    }
    pub const fn blocks(&self) -> bool {
        self.blocks
    }
    pub const fn termination(&self) -> &PackagePolicyTermination {
        &self.termination
    }
}
