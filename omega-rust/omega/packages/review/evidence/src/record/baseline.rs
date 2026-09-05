//! Inert package policy composition, independent of compiler replay evidence.

mod external;
mod getters;
mod validation;

use super::*;
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;

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
}
