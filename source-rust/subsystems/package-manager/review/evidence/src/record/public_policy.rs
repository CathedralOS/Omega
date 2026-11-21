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
    PackagePolicyTraitRequirement, PackagePolicyTraitShape,
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
}

impl PackagePolicyPublicApi {
    field_projection_accessors!(sliced traits: &[PackagePolicyTraitShape]);
    field_projection_accessors!(sliced conformances: &[PackagePolicyConformanceShape]);
    field_projection_accessors!(sliced domains: &[PackagePolicyDomainShape]);
    field_projection_accessors!(sliced propositions: &[PackageReviewPropositionShape]);
    field_projection_accessors!(sliced consts: &[PackageReviewConstShape]);
    field_projection_accessors!(sliced operators: &[PackagePolicyOperatorShape]);
    field_projection_accessors!(sliced data: &[PackagePolicyDataShape]);
}
