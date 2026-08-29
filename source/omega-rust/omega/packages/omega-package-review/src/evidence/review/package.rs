use super::super::{
    authority::{PackageReviewDangerousAuthority, PackageReviewDangerousAuthoritySlack},
    contracts::{
        PackageReviewConstShape, PackageReviewOperatorShape, PackageReviewPropositionShape,
    },
    identity::PackageReviewSemanticDependency,
    public_api::{
        PackageReviewDataShape, PackageReviewDomainShape, PackageReviewRepresentationTcb,
    },
    signatures::{
        PackageReviewConformanceShape, PackageReviewExternalExecutableSupply,
        PackageReviewTraitShape,
    },
};
use super::{
    CheckedPackageCallableReview, CheckedPackageProviderFamilyReview, CheckedPackageProviderReview,
    PackageReviewCanonicalRowSources,
};
use psi_core::PackageKeyIdentity;

#[derive(Debug, Clone)]
pub struct CheckedPackageReviewProjection {
    pub(crate) package: PackageKeyIdentity,
    pub(crate) target: omega_target::TargetProfile,
    pub(crate) public_traits: Vec<PackageReviewTraitShape>,
    pub(crate) public_conformances: Vec<PackageReviewConformanceShape>,
    pub(crate) public_domains: Vec<PackageReviewDomainShape>,
    pub(crate) public_propositions: Vec<PackageReviewPropositionShape>,
    pub(crate) public_consts: Vec<PackageReviewConstShape>,
    pub(crate) public_operators: Vec<PackageReviewOperatorShape>,
    pub(crate) public_data: Vec<PackageReviewDataShape>,
    pub(crate) representation_tcb: Vec<PackageReviewRepresentationTcb>,
    pub(crate) semantic_dependencies: Vec<PackageReviewSemanticDependency>,
    pub(crate) callables: Vec<CheckedPackageCallableReview>,
    pub(crate) external_executable_supply: Vec<PackageReviewExternalExecutableSupply>,
    pub(crate) dangerous_authorities: Vec<PackageReviewDangerousAuthority>,
    pub(crate) dangerous_authority_slack: Vec<PackageReviewDangerousAuthoritySlack>,
    pub(crate) selected_providers: Vec<CheckedPackageProviderReview>,
    pub(crate) selected_provider_families: Vec<CheckedPackageProviderFamilyReview>,
    pub(crate) row_sources: PackageReviewCanonicalRowSources,
}

impl PartialEq for CheckedPackageReviewProjection {
    fn eq(&self, other: &Self) -> bool {
        self.package == other.package
            && self.target == other.target
            && self.public_traits == other.public_traits
            && self.public_conformances == other.public_conformances
            && self.public_domains == other.public_domains
            && self.public_propositions == other.public_propositions
            && self.public_consts == other.public_consts
            && self.public_operators == other.public_operators
            && self.public_data == other.public_data
            && self.representation_tcb == other.representation_tcb
            && self.semantic_dependencies == other.semantic_dependencies
            && self.callables == other.callables
            && self.external_executable_supply == other.external_executable_supply
            && self.dangerous_authorities == other.dangerous_authorities
            && self.dangerous_authority_slack == other.dangerous_authority_slack
            && self.selected_providers == other.selected_providers
            && self.selected_provider_families == other.selected_provider_families
    }
}

impl Eq for CheckedPackageReviewProjection {}
