// Complete public declaration meaning (`PackagePolicyPublicApi`), independent
// of legacy review bytes.

mod declarations;
mod getters;
mod signatures;
pub(in crate::record) mod validation;

use super::{PackageReviewConstShape, PackageReviewPropositionShape};
use crate::record::accessors::field_projection_accessors;

pub(crate) use declarations::PackagePolicyOperatorShape;
pub use declarations::{
    PackagePolicyConformanceShape, PackagePolicyDataShape, PackagePolicyDomainShape,
    PackagePolicyTraitRequirement, PackagePolicyTraitShape, PackagePolicyTypeAlias,
};

pub(crate) use signatures::PackagePolicyMachineParameterSignature;
pub use signatures::{
    PackagePolicyMachineParameterContract, PackagePolicyTypeParameter,
    PackagePolicyTypeParameterKind,
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
            (
                BASELINE_WITHOUT_STRUCTURAL_PARAMETERS..BASELINE_WITH_TYPE_ALIASES,
                None,
            ) => Ok(None),
            (BASELINE_WITH_TYPE_ALIASES..=PACKAGE_POLICY_BASELINE_VERSION, Some(aliases)) => Ok(Some(aliases)),
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
