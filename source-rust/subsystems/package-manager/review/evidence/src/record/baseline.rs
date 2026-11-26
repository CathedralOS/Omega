// Inert package policy composition (`PackagePolicyBaseline`), independent of
// compiler replay evidence.
//
// `PackagePolicyBaseline` records one reviewed package in one exact target
// activation for comparison, not an acceptance decision or compiler
// certificate.

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
use crate::record::PackageReviewServiceIdentity;

mod component_assumptions;
pub use component_assumptions::PackagePolicyComponentAssumption;
mod native_bindings;
pub use native_bindings::PackagePolicyNativeBinding;
mod boundary_owners;
mod external;
mod getters;
mod validation;
use crate::record::accessors::field_projection_accessors;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyBaseline {
    pub schema_version: u16,
    pub package: PackageKeyIdentity,
    pub target: TargetProfile,
    pub public_api: PackagePolicyPublicApi,
    pub callables: PackagePolicyCallables,
    pub selected_providers: PackagePolicySelectedProviders,
    pub terminal_permissions: PackagePolicyTerminalPermissions,
    pub representation: PackagePolicyRepresentation,
    pub native_bindings: Vec<PackagePolicyNativeBinding>,
    pub external_supplies: Vec<PackagePolicyExternalExecutableSupply>,
    pub dangerous_capabilities: Vec<PackageReviewDangerousAuthority>,
    pub slack_uses: Vec<PackageReviewDangerousAuthoritySlack>,
    pub semantic_dependencies: Vec<PackagePolicySemanticDependency>,
    pub boundary_applications: PackagePolicyBoundaryApplications,
    pub service_audits: Vec<PackageReviewServiceIdentity>,
    pub restricted_build_requests: Vec<PackagePolicyRestrictedBuildRequest>,
    pub component_assumptions: Vec<PackagePolicyComponentAssumption>,
}

impl PackagePolicyBaseline {
    field_projection_accessors!(sliced service_audits: &[PackageReviewServiceIdentity]);

    pub fn apply_service_audit_policy(&mut self, services: &[PackageReviewServiceIdentity]) {
        use crate::record::PackageReviewDangerousAuthorityClass;
        self.service_audits = services.to_vec();
        self.service_audits.sort();
        self.service_audits.dedup();
        self.dangerous_capabilities.retain(|authority| {
            authority.class != PackageReviewDangerousAuthorityClass::ConsumerAudit
        });
        self.slack_uses
            .retain(|slack| slack.class != PackageReviewDangerousAuthorityClass::ConsumerAudit);
        let additions = self
            .service_audits
            .iter()
            .cloned()
            .map(|service| (service, PackageReviewDangerousAuthorityClass::ConsumerAudit))
            .collect::<Vec<_>>();
        self.apply_service_classes(&additions);
    }

    /// Derive findings and ceiling slack from the stored callable facts and exact classified identities.
    pub fn apply_service_classes(
        &mut self,
        classes: &[(
            PackageReviewServiceIdentity,
            crate::record::PackageReviewDangerousAuthorityClass,
        )],
    ) {
        for (service, class) in classes {
            let authority = PackageReviewDangerousAuthority {
                class: *class,
                service: service.clone(),
            };
            if self.dangerous_capabilities.contains(&authority)
                || !self
                    .callables
                    .callables
                    .iter()
                    .any(|callable| validation::exposes(callable, service))
            {
                continue;
            }
            self.dangerous_capabilities.push(authority);
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
                        class: *class,
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
