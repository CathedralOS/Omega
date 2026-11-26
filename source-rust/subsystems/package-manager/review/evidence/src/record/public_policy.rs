// Complete public declaration meaning (`PackagePolicyPublicApi`), independent
// of legacy review bytes.

mod declarations;
mod getters;
pub(in crate::record) mod validation;

use super::{PackageReviewConstShape, PackageReviewPropositionShape};
use crate::record::accessors::field_projection_accessors;
use crate::record::{
    PackagePolicyCrashRoute, PackagePolicyTermination, PackageReviewCallableContract,
    PackageReviewDataProperties, PackageReviewNominalIdentity,
    PackageReviewPropositionParameterSignature, PackageReviewServiceIdentity,
    PackageReviewSignatureParameter, PackageReviewSynchronousInvocation, PackageReviewTypeIdentity,
};
pub(crate) use declarations::PackagePolicyOperatorShape;
pub use declarations::{
    PackagePolicyConformanceShape, PackagePolicyDataShape, PackagePolicyDomainShape,
    PackagePolicyTraitRequirement, PackagePolicyTraitShape, PackagePolicyTypeAlias,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyPublicApi {
    pub traits: Vec<PackagePolicyTraitShape>,
    pub conformances: Vec<PackagePolicyConformanceShape>,
    pub domains: Vec<PackagePolicyDomainShape>,
    pub propositions: Vec<PackageReviewPropositionShape>,
    pub consts: Vec<PackageReviewConstShape>,
    pub operators: Vec<PackagePolicyOperatorShape>,
    pub data: Vec<PackagePolicyDataShape>,
    /// Older baseline formats did not describe aliases, even when a package declared them.
    pub type_aliases: Option<Vec<PackagePolicyTypeAlias>>,
}

impl PackagePolicyPublicApi {
    pub(crate) fn type_aliases_for_version(
        &self,
        version: u16,
    ) -> Result<Option<&[PackagePolicyTypeAlias]>, &'static str> {
        use crate::encoding::{
            BASELINE_WITH_TYPE_ALIASES, BASELINE_WITHOUT_STRUCTURAL_PARAMETERS,
            PACKAGE_POLICY_BASELINE_VERSION,
        };
        match (version, self.type_aliases.as_deref()) {
            (BASELINE_WITHOUT_STRUCTURAL_PARAMETERS..BASELINE_WITH_TYPE_ALIASES, None) => Ok(None),
            (BASELINE_WITH_TYPE_ALIASES..=PACKAGE_POLICY_BASELINE_VERSION, Some(aliases)) => {
                Ok(Some(aliases))
            }
            _ => Err("public type alias inventory disagrees with its policy version"),
        }
    }

    field_projection_accessors!(sliced traits: &[PackagePolicyTraitShape]);
    field_projection_accessors!(sliced conformances: &[PackagePolicyConformanceShape]);
    field_projection_accessors!(sliced domains: &[PackagePolicyDomainShape]);
    field_projection_accessors!(sliced propositions: &[PackageReviewPropositionShape]);
    field_projection_accessors!(sliced consts: &[PackageReviewConstShape]);
    field_projection_accessors!(sliced operators: &[PackagePolicyOperatorShape]);
    field_projection_accessors!(sliced data: &[PackagePolicyDataShape]);
}

// Public policy type parameters, machine parameter contracts, and signature telescopes.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyTypeParameter {
    pub kind: PackagePolicyTypeParameterKind,
    pub bounds: PackageReviewDataProperties,
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
    pub(crate) service_reach: Vec<PackageReviewServiceIdentity>,
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
