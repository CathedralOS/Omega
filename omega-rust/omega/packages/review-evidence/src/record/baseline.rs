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
    pub(crate) assumed_bindings: Vec<String>,
    pub(crate) service_audits: Vec<crate::record::PackageReviewNominalIdentity>,
    pub(crate) restricted_build_requests: Vec<PackagePolicyRestrictedBuildRequest>,
}

impl PackagePolicyBaseline {
    pub fn service_audits(&self) -> &[crate::record::PackageReviewNominalIdentity] {
        &self.service_audits
    }

    pub fn apply_service_audit_policy(
        &mut self,
        services: &[crate::record::PackageReviewNominalIdentity],
    ) {
        use crate::record::PackageReviewDangerousAuthorityClass;
        self.service_audits = services.to_vec();
        self.service_audits.sort();
        self.service_audits.dedup();
        self.dangerous_capabilities.retain(|authority| {
            authority.class != PackageReviewDangerousAuthorityClass::ConsumerAudit
        });
        self.slack_uses
            .retain(|slack| slack.class != PackageReviewDangerousAuthorityClass::ConsumerAudit);
        for service in &self.service_audits {
            if self
                .dangerous_capabilities
                .iter()
                .any(|authority| &authority.service == service)
            {
                continue;
            }
            if !self
                .callables
                .callables
                .iter()
                .any(|callable| validation::exposes(callable, service))
            {
                continue;
            }
            self.dangerous_capabilities
                .push(PackageReviewDangerousAuthority {
                    class: PackageReviewDangerousAuthorityClass::ConsumerAudit,
                    service: service.clone(),
                });
            for callable in &self.callables.callables {
                if callable
                    .declared_service_reach()
                    .is_some_and(|ceiling| ceiling.contains(service))
                    && callable
                        .checked_service_reach()
                        .realized()
                        .is_some_and(|realized| !realized.contains(service))
                {
                    self.slack_uses.push(PackageReviewDangerousAuthoritySlack {
                        class: PackageReviewDangerousAuthorityClass::ConsumerAudit,
                        callable: callable.identity().clone(),
                        service: service.clone(),
                    });
                }
            }
        }
        self.dangerous_capabilities.sort();
        self.dangerous_capabilities.dedup();
        self.slack_uses.sort();
        self.slack_uses.dedup();
    }
}
