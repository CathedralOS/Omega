// Inert package policy composition (`PackagePolicyBaseline`), independent of
// compiler replay evidence.
//
// `PackagePolicyBaseline` records one reviewed package in one exact target
// activation for comparison, not an acceptance decision or compiler
// certificate.

use crate::record::accessors::field_projection_accessors;
use crate::record::{
    PackagePolicyBoundaryApplications, PackagePolicyCallables,
    PackagePolicyExternalExecutableSupply, PackagePolicyProviderBinding, PackagePolicyPublicApi,
    PackagePolicyRepresentation, PackagePolicyRestrictedBuildRequest,
    PackagePolicySelectedProviders, PackagePolicySemanticDependency, PackagePolicyServiceMethod,
    PackagePolicyTerminalPermissions, PackageReviewDangerousAuthority,
    PackageReviewDangerousAuthoritySlack, PackageReviewNominalOwner, PackageReviewServiceIdentity,
};
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

mod boundary_owners;
mod external;
mod getters;
mod validation;

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

// Environmental premises of the independent components this root deploys
// (`PackagePolicyComponentAssumption`).
//
// Each row is one assumption from a verified component's roster, together with
// the component's package and the exact description rows bound by it. The
// consuming project's lock retains the reviewed row; root Product admission
// rejects a fresh row the lock does not hold
// (wiki/spec/build/component_publication.md#consumer-acceptance-and-build-attenuation).

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyComponentAssumption {
    pub component: PackageKeyIdentity,
    pub assumption: [u8; 32],
    pub bound_rows: Vec<String>,
}

impl PackagePolicyComponentAssumption {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.bound_rows.is_empty() {
            return Err("component assumption binds no description row");
        }
        if self.bound_rows.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("component assumption repeats or reorders its bound rows");
        }
        Ok(())
    }
}

// Completed native `Build` choices (`PackagePolicyNativeBinding`), including
// private and unused supplies.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyNativeBinding {
    pub application: PackagePolicyNativeApplication,
    pub method: PackagePolicyServiceMethod,
    pub binding: PackagePolicyProviderBinding,
}
/// A concrete native call has an ABI plan, including a monomorphic declaration
/// with no caller. An unused generic import keeps its binders and authored policy.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PackagePolicyNativeApplication {
    Concrete(Option<String>),
    UnusedGeneric {
        declaration: String,
        unbound_parameters: Vec<String>,
        custom_calling_policy: Option<crate::record::PackageReviewNominalIdentity>,
        source_occurrence: [u8; 32],
        evaluation: [u8; 32],
    },
}
impl PackagePolicyNativeBinding {
    /// The complete closed callable selected by Build, when the binding has
    /// static arguments. Its original method remains in the calling ABI key.
    pub fn application(&self) -> Option<&str> {
        match &self.application {
            PackagePolicyNativeApplication::Concrete(application) => application.as_deref(),
            PackagePolicyNativeApplication::UnusedGeneric { .. } => None,
        }
    }

    pub fn method(&self) -> &PackagePolicyServiceMethod {
        &self.method
    }

    pub fn locator(&self) -> Option<&crate::record::PackageReviewForeignLocator> {
        match &self.binding {
            PackagePolicyProviderBinding::Import { locator, .. } => Some(locator),
            _ => None,
        }
    }

    pub fn key(
        &self,
    ) -> (
        &crate::record::PackageReviewNominalIdentity,
        &PackagePolicyNativeApplication,
        &str,
    ) {
        (
            &self.method.requirement,
            &self.application,
            self.producer().map_or("", |p| p.callable_identity.as_str()),
        )
    }
    pub fn producer(&self) -> Option<&crate::record::PackagePolicyEvaluatedBindingProducer> {
        match &self.binding {
            PackagePolicyProviderBinding::Import { producer, .. }
            | PackagePolicyProviderBinding::FunctionField { producer, .. } => Some(producer),
            PackagePolicyProviderBinding::Syscall {
                evaluated: Some(evaluated),
                ..
            } => Some(&evaluated.producer),
            _ => None,
        }
    }
    pub(crate) fn validate(
        &self,
        package: semantic_vocabulary::PackageKeyIdentity,
        target: target::TargetProfile,
    ) -> Result<(), &'static str> {
        let producer = self
            .producer()
            .ok_or("native inventory has no evaluated Build producer")?;
        if producer.package != Some(package)
            || producer.declaration.owner() != PackageReviewNominalOwner::Package(package)
        {
            return Err("native inventory substituted its supplying package");
        }
        crate::record::package::validate_native_service_method(&self.method, target)?;
        self.binding.validate_native(target)?;
        let application = match &self.application {
            PackagePolicyNativeApplication::Concrete(application) => application,
            PackagePolicyNativeApplication::UnusedGeneric {
                declaration,
                unbound_parameters,
                custom_calling_policy,
                source_occurrence,
                evaluation,
            } => {
                if !matches!(self.binding, PackagePolicyProviderBinding::Import { .. })
                    || !self.method.calling.is_empty()
                    || !self
                        .method
                        .requirement
                        .matches_callable_declaration(declaration)
                    || unbound_parameters.is_empty()
                    || unbound_parameters.len() != self.method.signature.static_parameters.len()
                    || unbound_parameters
                        .iter()
                        .enumerate()
                        .any(|(i, name)| name.is_empty() || unbound_parameters[..i].contains(name))
                    || *source_occurrence == [0; 32]
                    || *evaluation == [0; 32]
                {
                    return Err(
                        "unused generic import lacks its open declaration or Build provenance",
                    );
                }
                if let Some(policy) = custom_calling_policy {
                    policy.validate_resolved()?;
                }
                return Ok(());
            }
        };
        let [calling] = self.method.calling.as_slice() else {
            return Err("native inventory has no unique pinned calling application");
        };
        match application {
            Some(application)
                if !self
                    .method
                    .requirement
                    .matches_closed_calling_application(application, &calling.abi)? =>
            {
                return Err(
                    "native inventory application differs from its complete calling identity",
                );
            }
            None if !calling.abi.operands.is_empty() => {
                return Err("native inventory lost its closed calling application");
            }
            _ => {}
        }
        if calling.abi.requirement_identity == calling.requirement.path {
            return Err("native inventory has no complete declaration ABI key");
        }
        if let PackagePolicyProviderBinding::FunctionField { selector, .. } = &self.binding {
            let expected = application
                .as_deref()
                .unwrap_or(&calling.abi.requirement_identity);
            if selector.table.requirement_application != expected.as_bytes()
                || selector.table.declaration_position >= self.method.parameter_count as u64
                || calling
                    .dispatch_only_formal
                    .is_some_and(|formal| u64::from(formal) != selector.table.declaration_position)
            {
                return Err("function table selects a formal outside its reviewed requirement");
            }
        } else if calling.dispatch_only_formal.is_some() {
            return Err("native argument is only omitted for table dispatch");
        }
        Ok(())
    }
}
