use super::*;

impl PackagePolicyBaseline {
    pub const fn package(&self) -> PackageKeyIdentity {
        self.package
    }
    pub const fn target(&self) -> TargetProfile {
        self.target
    }
    pub const fn public_api(&self) -> &PackagePolicyPublicApi {
        &self.public_api
    }
    pub fn public_traits(&self) -> &[PackagePolicyTraitShape] {
        self.public_api.traits()
    }
    pub fn public_conformances(&self) -> &[PackagePolicyConformanceShape] {
        self.public_api.conformances()
    }
    pub fn public_domains(&self) -> &[PackagePolicyDomainShape] {
        self.public_api.domains()
    }
    pub fn public_propositions(&self) -> &[PackageReviewPropositionShape] {
        self.public_api.propositions()
    }
    pub fn public_consts(&self) -> &[PackageReviewConstShape] {
        self.public_api.consts()
    }
    pub fn public_operators(&self) -> &[PackagePolicyOperatorShape] {
        self.public_api.operators()
    }
    pub fn public_data(&self) -> &[PackagePolicyDataShape] {
        self.public_api.data()
    }
    pub const fn callables(&self) -> &PackagePolicyCallables {
        &self.callables
    }
    pub const fn selected_providers(&self) -> &PackagePolicySelectedProviders {
        &self.selected_providers
    }
    pub const fn terminal_permissions(&self) -> &PackagePolicyTerminalPermissions {
        &self.terminal_permissions
    }
    pub const fn representation(&self) -> &PackagePolicyRepresentation {
        &self.representation
    }
    pub fn external_supplies(&self) -> &[PackagePolicyExternalExecutableSupply] {
        &self.external_supplies
    }
    pub fn dangerous_capabilities(&self) -> &[PackageReviewDangerousAuthority] {
        &self.dangerous_capabilities
    }
    pub fn slack_uses(&self) -> &[PackageReviewDangerousAuthoritySlack] {
        &self.slack_uses
    }
    pub fn semantic_dependencies(&self) -> &[PackagePolicySemanticDependency] {
        &self.semantic_dependencies
    }
    pub const fn boundary_applications(&self) -> &PackagePolicyBoundaryApplications {
        &self.boundary_applications
    }
}
