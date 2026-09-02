use super::super::{
    authority::{PackageReviewDangerousAuthority, PackageReviewDangerousAuthoritySlack},
    contracts::{
        PackageReviewConstShape, PackageReviewContractEntailmentOpenObligation,
        PackageReviewOperatorShape, PackageReviewPropositionShape,
    },
    data::PackageReviewDataShape,
    domains::PackageReviewDomainShape,
    identity::PackageReviewSemanticDependency,
    representation::PackageReviewRepresentationTcb,
    signatures::{
        PackageReviewConformanceShape, PackageReviewExternalExecutableSupply,
        PackageReviewTraitShape,
    },
    terminal_authority::PackageReviewTerminalAuthorityPermission,
};
use super::{
    CheckedPackageBoundaryApplicationDemandReview,
    CheckedPackageBoundaryApplicationRealizationReview, CheckedPackageCallableReview,
    CheckedPackageProviderFamilyReview, CheckedPackageProviderReview,
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
    pub(crate) contract_entailment_open_obligations:
        Vec<PackageReviewContractEntailmentOpenObligation>,
    pub(crate) external_executable_supply: Vec<PackageReviewExternalExecutableSupply>,
    pub(crate) dangerous_authorities: Vec<PackageReviewDangerousAuthority>,
    pub(crate) dangerous_authority_slack: Vec<PackageReviewDangerousAuthoritySlack>,
    pub(crate) terminal_authority_permissions: Vec<PackageReviewTerminalAuthorityPermission>,
    pub(crate) selected_providers: Vec<CheckedPackageProviderReview>,
    pub(crate) selected_provider_families: Vec<CheckedPackageProviderFamilyReview>,
    pub(crate) boundary_application_realizations:
        Vec<CheckedPackageBoundaryApplicationRealizationReview>,
    pub(crate) boundary_application_demands: Vec<CheckedPackageBoundaryApplicationDemandReview>,
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
            && self.contract_entailment_open_obligations
                == other.contract_entailment_open_obligations
            && self.external_executable_supply == other.external_executable_supply
            && self.dangerous_authorities == other.dangerous_authorities
            && self.dangerous_authority_slack == other.dangerous_authority_slack
            && self.terminal_authority_permissions == other.terminal_authority_permissions
            && self.selected_providers == other.selected_providers
            && self.selected_provider_families == other.selected_provider_families
            && self.boundary_application_realizations == other.boundary_application_realizations
            && self.boundary_application_demands == other.boundary_application_demands
    }
}

impl Eq for CheckedPackageReviewProjection {}
