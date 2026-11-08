//! Complete public declaration meaning, independent of legacy review bytes.

mod declarations;
mod getters;
mod signatures;
pub(in crate::record) mod validation;

use super::{PackageReviewConstShape, PackageReviewPropositionShape};
use crate::record::accessors::field_projection_accessors;

pub(crate) use declarations::{
    PackagePolicyConformanceShape, PackagePolicyDataShape, PackagePolicyDomainShape,
    PackagePolicyOperatorShape, PackagePolicyTraitRequirement, PackagePolicyTraitShape,
};

pub(crate) use signatures::{
    PackagePolicyMachineParameterContract, PackagePolicyMachineParameterSignature,
    PackagePolicyTypeParameter, PackagePolicyTypeParameterKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyPublicApi {
    pub(crate) traits: Vec<PackagePolicyTraitShape>,
    pub(crate) conformances: Vec<PackagePolicyConformanceShape>,
    pub(crate) domains: Vec<PackagePolicyDomainShape>,
    pub(crate) propositions: Vec<PackageReviewPropositionShape>,
    pub(crate) consts: Vec<PackageReviewConstShape>,
    pub(crate) operators: Vec<PackagePolicyOperatorShape>,
    pub(crate) data: Vec<PackagePolicyDataShape>,
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
