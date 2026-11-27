// Cross-component structural validation for `PackagePolicyBaseline`, without
// compiler or acceptance reconstruction.
//
// Foreign operator declarations belong to their own package's baseline, so this
// aggregate checks their qualified coordinate and delegates local operator
// demand validation to `boundary_owners::validate_demand`.

use crate::record::PackagePolicyBaseline;
use crate::record::PackagePolicyCallable;
use crate::record::PackagePolicyCallableRole;
use crate::record::PackagePolicyExternalBinding;
use crate::record::PackagePolicySemanticDependencyConsumer;
use crate::record::PackagePolicyTargetContext;
use crate::record::PackagePolicyTypeParameterKind;
use crate::record::PackageReviewCallableSupply;
use crate::record::PackageReviewNominalIdentity;
use crate::record::PackageReviewNominalOwner;
use crate::record::PackageReviewSemanticDependencyExposure;
use crate::record::PackageReviewServiceIdentity;
use crate::record::PackageReviewSymbolicBoundaryApplicationArgument;
use crate::record::validation::ordered;

impl PackagePolicyBaseline {
    pub fn validate_canonical_structure(&self) -> Result<(), &'static str> {
        if self.callables.package != self.package
            || self.callables.target != self.target
            || self.selected_providers.package != self.package
            || self.selected_providers.target != self.target
            || self.terminal_permissions.package != self.package
            || self.terminal_permissions.target != self.target
            || self.representation.package != self.package
            || self.representation.target.profile().backend_profile() != self.target
        {
            return Err("package policy components disagree about package or target");
        }
        self.public_api
            .type_aliases_for_version(self.schema_version)?;
        self.public_api.validate_canonical_structure(self.package)?;
        self.callables.validate_canonical_structure()?;
        self.selected_providers.validate_canonical_structure()?;
        if self.schema_version < crate::encoding::BASELINE_WITH_PROVIDER_APPLICATION_GRANTS
            && self.selected_providers.has_application_grants()
        {
            return Err("this package policy version cannot record provider application grants");
        }
        self.terminal_permissions.validate_canonical_structure()?;
        self.representation.validate_canonical_structure()?;
        self.boundary_applications.validate_canonical_structure(
            self.package,
            self.target,
            &self.selected_providers,
        )?;
        self.symbolic_demands()?;
        for row in &self.native_bindings {
            if self.schema_version < crate::encoding::BASELINE_WITH_UNUSED_GENERIC_IMPORTS
                && matches!(
                    row.application,
                    crate::record::PackagePolicyNativeApplication::UnusedGeneric { .. }
                )
            {
                return Err("this package policy version cannot record unused generic imports");
            }
            row.validate(self.package, self.target)?;
        }
        if self
            .native_bindings
            .windows(2)
            .any(|pair| pair[0].key() >= pair[1].key())
        {
            return Err("native inventory repeats or reorders its exact producer/requirement");
        }
        ordered(&self.external_supplies)?;
        self.validate_external_associations()?;
        ordered(&self.dangerous_capabilities)?;
        ordered(&self.slack_uses)?;
        self.validated_semantic_dependencies()?;
        ordered(&self.service_audits)?;
        ordered(&self.component_assumptions)?;
        for row in &self.component_assumptions {
            row.validate()?;
        }
        for service in &self.service_audits {
            service.validate_resolved()?;
        }
        for supply in &self.external_supplies {
            supply.callable().validate_resolved()?;
            if supply.callable().owner() != PackageReviewNominalOwner::Package(self.package) {
                return Err("external policy callable belongs to another package");
            }
            let expected_target = match supply.target_context() {
                PackagePolicyTargetContext::TargetIndependent => self.target,
                PackagePolicyTargetContext::TargetMember { target } => {
                    target::TargetProfile::from_canonical_target_name(target)
                        .map_err(|_| "external policy has an uncatalogued authored target")?
                }
            };
            match supply.binding() {
                PackagePolicyExternalBinding::NormalizedImport { target, .. }
                | PackagePolicyExternalBinding::NormalizedSyscall { target, .. }
                    if target != expected_target.identity().as_str() =>
                {
                    return Err("external policy binding belongs to another target");
                }
                _ => {}
            }
        }
        for authority in &self.dangerous_capabilities {
            authority.service.validate_resolved()?;
            if authority.class == crate::record::PackageReviewDangerousAuthorityClass::ConsumerAudit
                && !self.service_audits.contains(&authority.service)
            {
                return Err("consumer audit finding has no retained root policy registration");
            }
            if !self
                .callables
                .callables
                .iter()
                .any(|callable| exposes(callable, &authority.service))
            {
                return Err("dangerous capability has no retained callable exposure");
            }
        }
        for slack in &self.slack_uses {
            slack.service.validate_resolved()?;
            let callable = self
                .callables
                .callables
                .iter()
                .find(|callable| callable.identity == slack.callable)
                .ok_or("package policy relation refers to an absent callable")?;
            if !callable
                .declared_service_reach()
                .is_some_and(|ceiling| ceiling.contains(&slack.service))
                || !callable
                    .checked_service_reach()
                    .realized()
                    .is_some_and(|realized| !realized.contains(&slack.service))
            {
                return Err("authority slack differs from its retained ceiling and realized reach");
            }
            if !self.dangerous_capabilities.iter().any(|authority| {
                authority.class == slack.class && authority.service == slack.service
            }) {
                return Err("authority slack has no matching dangerous capability");
            }
        }
        for request in &self.restricted_build_requests {
            if request.selected_target_profile != Some(self.target) {
                return Err("restricted build request belongs to another target activation");
            }
        }
        Ok(())
    }

    pub(crate) fn validated_semantic_dependencies(
        &self,
    ) -> Result<&[crate::record::PackagePolicySemanticDependency], &'static str> {
        ordered(&self.semantic_dependencies)?;
        let mut previous_carried: Option<(
            &crate::record::PackageReviewNominalIdentity,
            source_provenance::PackageSourceConsumptionCommitment,
        )> = None;
        for dependency in &self.semantic_dependencies {
            use crate::record::{PackagePolicySemanticDependency, PackageReviewNominalOwner};
            dependency.dependency().validate_resolved()?;
            match dependency {
                PackagePolicySemanticDependency::Declaration {
                    consumer, exposure, ..
                } => match consumer {
                    PackagePolicySemanticDependencyConsumer::Callable(identity) => {
                        self.callable(identity)?;
                    }
                    PackagePolicySemanticDependencyConsumer::PackageImplementation => {
                        if *exposure
                            != PackageReviewSemanticDependencyExposure::PrivateImplementation
                        {
                            return Err("private package consumer claims public semantic exposure");
                        }
                    }
                },
                PackagePolicySemanticDependency::CarriedType {
                    dependency,
                    supplier_sources,
                    ..
                } => {
                    if self.schema_version < crate::encoding::BASELINE_WITH_CARRIED_TYPES
                        || !matches!(dependency.owner(), PackageReviewNominalOwner::Package(owner) if owner != self.package)
                    {
                        return Err(
                            "carried type requires a different package's checked source commitment",
                        );
                    }
                    // Carried rows are ordered by nominal owner and declaration.
                    if let Some((previous, commitment)) = previous_carried
                        && (previous == dependency
                            || (previous.owner() == dependency.owner()
                                && commitment != *supplier_sources))
                    {
                        return Err(
                            "carried types disagree about their supplier or repeat a declaration",
                        );
                    }
                    previous_carried = Some((dependency, *supplier_sources));
                }
            }
        }
        Ok(&self.semantic_dependencies)
    }

    fn callable(&self, identity: &PackageReviewNominalIdentity) -> Result<(), &'static str> {
        if self
            .callables
            .callables
            .iter()
            .any(|callable| &callable.identity == identity)
        {
            Ok(())
        } else {
            Err("package policy relation refers to an absent callable")
        }
    }

    fn symbolic_demands(&self) -> Result<(), &'static str> {
        for demand in &self.boundary_applications.demands {
            let producer = self
                .callables
                .callables
                .iter()
                .find(|callable| callable.identity == demand.producer_callable)
                .ok_or("symbolic demand has no exact retained producer callable")?;
            if producer.role != PackagePolicyCallableRole::Public
                || producer.supply != PackageReviewCallableSupply::CheckedBody
            {
                return Err("symbolic demand producer is not a public checked body");
            }
            for argument in &demand.arguments {
                let PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
                    producer_binder_ordinal,
                    ..
                } = argument;
                if !producer
                    .type_parameters
                    .get(*producer_binder_ordinal as usize)
                    .is_some_and(|parameter| {
                        matches!(parameter.kind, PackagePolicyTypeParameterKind::Type)
                    })
                {
                    return Err(
                        "symbolic demand producer binder is not an exact retained type parameter",
                    );
                }
            }
            if demand.operator_coordinate.identity.owner
                == PackageReviewNominalOwner::Package(self.package)
            {
                super::boundary_owners::validate_demand(demand, self)?;
            }
        }
        Ok(())
    }
}

pub(super) fn exposes(
    callable: &PackagePolicyCallable,
    service: &PackageReviewServiceIdentity,
) -> bool {
    callable
        .exposed_services()
        .any(|candidate| candidate == service)
}
