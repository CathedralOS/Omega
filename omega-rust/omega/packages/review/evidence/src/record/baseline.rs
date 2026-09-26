//! Inert package policy composition, independent of compiler replay evidence.
use crate::record::PackagePolicyBoundaryApplications;
use crate::record::PackagePolicyCallables;
use crate::record::PackagePolicyExternalExecutableSupply;
use crate::record::PackagePolicyPublicApi;
use crate::record::PackagePolicyRepresentation;
use crate::record::PackagePolicyRestrictedBuildRequest;
use crate::record::PackagePolicySelectedProviders;
use crate::record::PackagePolicySemanticDependency;
use crate::record::PackagePolicyTerminalPermissions;
use crate::record::PackageReviewDangerousAuthority;
use crate::record::PackageReviewDangerousAuthoritySlack;

mod boundary_owners;
mod external;
mod getters;
mod validation;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

/// One reviewed package in one exact target activation. This record carries
/// comparison meaning, not an acceptance decision or a compiler certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyBaseline {
    pub(crate) package: PackageKeyIdentity,
    pub(crate) target: TargetProfile,
    pub(crate) public_api: PackagePolicyPublicApi,
    pub(crate) callables: PackagePolicyCallables,
    pub(crate) selected_providers: PackagePolicySelectedProviders,
    pub(crate) terminal_permissions: PackagePolicyTerminalPermissions,
    pub(crate) representation: PackagePolicyRepresentation,
    pub(crate) external_supplies: Vec<PackagePolicyExternalExecutableSupply>,
    pub(crate) dangerous_capabilities: Vec<PackageReviewDangerousAuthority>,
    pub(crate) slack_uses: Vec<PackageReviewDangerousAuthoritySlack>,
    pub(crate) semantic_dependencies: Vec<PackagePolicySemanticDependency>,
    pub(crate) boundary_applications: PackagePolicyBoundaryApplications,
    /// The build-host requests this package's admitted build activation asked
    /// before it executed, in issue order. Each retains as its own decision
    /// row so accepted meaning survives in `omega.lock` without host paths or
    /// live grant state.
    pub(crate) restricted_build_requests: Vec<PackagePolicyRestrictedBuildRequest>,
}
