use crate::record::PackagePolicyBaseline;
use crate::record::PackagePolicyBoundaryApplications;
use crate::record::PackagePolicyCallables;
use crate::record::PackagePolicyConformanceShape;
use crate::record::PackagePolicyDataShape;
use crate::record::PackagePolicyDomainShape;
use crate::record::PackagePolicyExternalExecutableSupply;
use crate::record::PackagePolicyOperatorShape;
use crate::record::PackagePolicyPublicApi;
use crate::record::PackagePolicyRepresentation;
use crate::record::PackagePolicyRestrictedBuildRequest;
use crate::record::PackagePolicySelectedProviders;
use crate::record::PackagePolicySemanticDependency;
use crate::record::PackagePolicyTerminalPermissions;
use crate::record::PackagePolicyTraitShape;
use crate::record::PackageReviewConstShape;
use crate::record::PackageReviewDangerousAuthority;
use crate::record::PackageReviewDangerousAuthoritySlack;
use crate::record::PackageReviewPropositionShape;
use crate::record::accessors::field_projection_accessors;

impl PackagePolicyBaseline {
    field_projection_accessors!(copied package: semantic_vocabulary::PackageKeyIdentity);
    field_projection_accessors!(copied target: target::TargetProfile);
    field_projection_accessors!(borrowed public_api: &PackagePolicyPublicApi);
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
    field_projection_accessors!(borrowed callables: &PackagePolicyCallables);
    field_projection_accessors!(borrowed selected_providers: &PackagePolicySelectedProviders);
    field_projection_accessors!(borrowed terminal_permissions: &PackagePolicyTerminalPermissions);
    field_projection_accessors!(borrowed representation: &PackagePolicyRepresentation);
    field_projection_accessors!(sliced external_supplies: &[PackagePolicyExternalExecutableSupply]);
    field_projection_accessors!(sliced dangerous_capabilities: &[PackageReviewDangerousAuthority]);
    field_projection_accessors!(sliced slack_uses: &[PackageReviewDangerousAuthoritySlack]);
    field_projection_accessors!(sliced semantic_dependencies: &[PackagePolicySemanticDependency]);
    field_projection_accessors!(borrowed boundary_applications: &PackagePolicyBoundaryApplications);
    /// Restricted build-host requests this package's admitted build activation
    /// asked of the host before it executed, in issue order.
    pub fn restricted_build_requests(&self) -> &[PackagePolicyRestrictedBuildRequest] {
        &self.restricted_build_requests
    }
}
