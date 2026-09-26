use crate::package_evidence::record::PackagePolicyBaseline;
use crate::package_evidence::record::PackagePolicyBoundaryApplications;
use crate::package_evidence::record::PackagePolicyCallables;
use crate::package_evidence::record::PackagePolicyConformanceShape;
use crate::package_evidence::record::PackagePolicyDataShape;
use crate::package_evidence::record::PackagePolicyDomainShape;
use crate::package_evidence::record::PackagePolicyExternalExecutableSupply;
use crate::package_evidence::record::PackagePolicyOperatorShape;
use crate::package_evidence::record::PackagePolicyPublicApi;
use crate::package_evidence::record::PackagePolicyRepresentation;
use crate::package_evidence::record::PackagePolicyRestrictedBuildRequest;
use crate::package_evidence::record::PackagePolicySelectedProviders;
use crate::package_evidence::record::PackagePolicySemanticDependency;
use crate::package_evidence::record::PackagePolicyTerminalPermissions;
use crate::package_evidence::record::PackagePolicyTraitShape;
use crate::package_evidence::record::PackageReviewConstShape;
use crate::package_evidence::record::PackageReviewDangerousAuthority;
use crate::package_evidence::record::PackageReviewDangerousAuthoritySlack;
use crate::package_evidence::record::PackageReviewPropositionShape;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

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
    /// Restricted build-host requests this package's admitted build activation
    /// asked of the host before it executed, in issue order.
    pub fn restricted_build_requests(&self) -> &[PackagePolicyRestrictedBuildRequest] {
        &self.restricted_build_requests
    }
}
