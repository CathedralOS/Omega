use super::super::rows::PackageReviewCanonicalRowSource;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageReviewCanonicalRowSources {
    pub(crate) public_traits: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_conformances: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_domains: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_propositions: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_consts: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_operators: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) public_data: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) representation_tcb: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) semantic_dependencies: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) callables: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) external_executable_supply: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) dangerous_authorities: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) dangerous_authority_slack: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) boundary_application_realizations: Vec<PackageReviewCanonicalRowSource>,
    pub(crate) selected_provider_set: PackageReviewCanonicalRowSource,
}
