//! Complete public declaration meaning, independent of legacy review bytes.

mod declarations;
mod getters;
mod signatures;
pub(in crate::record) mod validation;

use super::{PackageReviewConstShape, PackageReviewPropositionShape};
pub use declarations::{
    PackagePolicyConformanceShape, PackagePolicyDataShape, PackagePolicyDomainShape,
    PackagePolicyOperatorShape, PackagePolicyTraitRequirement, PackagePolicyTraitShape,
};
pub use signatures::{
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
    pub fn traits(&self) -> &[PackagePolicyTraitShape] {
        &self.traits
    }
    pub fn conformances(&self) -> &[PackagePolicyConformanceShape] {
        &self.conformances
    }
    pub fn domains(&self) -> &[PackagePolicyDomainShape] {
        &self.domains
    }
    pub fn propositions(&self) -> &[PackageReviewPropositionShape] {
        &self.propositions
    }
    pub fn consts(&self) -> &[PackageReviewConstShape] {
        &self.consts
    }
    pub fn operators(&self) -> &[PackagePolicyOperatorShape] {
        &self.operators
    }
    pub fn data(&self) -> &[PackagePolicyDataShape] {
        &self.data
    }
}
