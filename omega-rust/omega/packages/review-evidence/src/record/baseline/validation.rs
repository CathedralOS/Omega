//! Cross-component associations, without compiler or acceptance reconstruction.

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
use crate::record::PackageReviewSymbolicBoundaryApplicationArgument;
use crate::record::validation::ordered;

impl PackagePolicyBaseline {
    pub(crate) fn validate_canonical_structure(&self) -> Result<(), &'static str> {
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
        self.public_api.validate_canonical_structure(self.package)?;
        self.callables.validate_canonical_structure()?;
        self.selected_providers.validate_canonical_structure()?;
        self.terminal_permissions.validate_canonical_structure()?;
        self.representation.validate_canonical_structure()?;
        self.boundary_applications.validate_canonical_structure(
            self.package,
            self.target,
            &self.selected_providers,
        )?;
        self.symbolic_demands()?;
        ordered(&self.external_supplies)?;
        self.validate_external_associations()?;
        ordered(&self.dangerous_capabilities)?;
        ordered(&self.slack_uses)?;
        ordered(&self.semantic_dependencies)?;
        ordered(&self.service_audits)?;
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
        for dependency in &self.semantic_dependencies {
            dependency.dependency.validate_resolved()?;
            match &dependency.consumer {
                PackagePolicySemanticDependencyConsumer::Callable(identity) => {
                    self.callable(identity)?;
                }
                PackagePolicySemanticDependencyConsumer::PackageImplementation => {
                    if dependency.exposure
                        != PackageReviewSemanticDependencyExposure::PrivateImplementation
                    {
                        return Err("private package consumer claims public semantic exposure");
                    }
                }
            }
        }
        Ok(())
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
            // Foreign operator declarations belong to their own package's
            // baseline. This aggregate can check their qualified coordinate,
            // not manufacture a local copy of the foreign declaration.
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
    service: &PackageReviewNominalIdentity,
) -> bool {
    callable
        .published_service_reach()
        .is_some_and(|values| values.contains(service))
        || callable
            .checked_service_reach()
            .realized()
            .is_some_and(|values| values.contains(service))
        || callable
            .checked_service_reach()
            .concrete()
            .is_some_and(|values| values.contains(service))
        || callable
            .unresolved_installation_reaches()
            .iter()
            .any(|value| value.upper_bound().contains(service))
        || callable
            .declared_synchronous_invocations()
            .is_some_and(|values| values.iter().any(|value| value.service() == Some(service)))
        || callable
            .realized_synchronous_invocations()
            .iter()
            .any(|value| value.service() == Some(service))
}
