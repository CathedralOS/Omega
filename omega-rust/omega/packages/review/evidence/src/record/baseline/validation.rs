//! Cross-component associations, without compiler or acceptance reconstruction.

use super::*;

impl PackagePolicyBaseline {
    pub(crate) fn validate_canonical_structure(&self) -> Result<(), &'static str> {
        if self.callables.package != self.package
            || self.callables.target != self.target
            || self.selected_providers.package != self.package
            || self.selected_providers.target != self.target
            || self.terminal_permissions.package != self.package
            || self.terminal_permissions.target != self.target
            || self.representation.package != self.package
            || representation_profile(self.representation.target.profile()) != self.target
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
        for supply in &self.external_supplies {
            nominal(supply.callable())?;
            if supply.callable().owner() != PackageReviewNominalOwner::Package(self.package) {
                return Err("external policy callable belongs to another package");
            }
            match supply.binding() {
                PackagePolicyExternalBinding::NormalizedImport { target, .. }
                | PackagePolicyExternalBinding::NormalizedSyscall { target, .. }
                    if target != self.target.identity().as_str() =>
                {
                    return Err("external policy binding belongs to another target");
                }
                _ => {}
            }
        }
        for authority in &self.dangerous_capabilities {
            nominal(&authority.service)?;
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
            nominal(&slack.service)?;
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
        for dependency in &self.semantic_dependencies {
            nominal(&dependency.dependency)?;
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
                let operator = self
                    .public_api
                    .operators
                    .iter()
                    .find(|operator| operator.coordinate == demand.operator_coordinate)
                    .ok_or("symbolic demand has no exact retained local operator")?;
                if !operator.is_boundary || operator.type_parameters.len() != demand.arguments.len()
                {
                    return Err("symbolic demand differs from its boundary operator telescope");
                }
                for argument in &demand.arguments {
                    let PackageReviewSymbolicBoundaryApplicationArgument::TypeBinder {
                        requirement_binder_ordinal,
                        ..
                    } = argument;
                    if !operator
                        .type_parameters
                        .get(*requirement_binder_ordinal as usize)
                        .is_some_and(|parameter| {
                            matches!(parameter.kind, PackagePolicyTypeParameterKind::Type)
                        })
                    {
                        return Err(
                            "symbolic demand requirement binder is not an exact retained type parameter",
                        );
                    }
                }
            }
        }
        Ok(())
    }
}

fn exposes(callable: &PackagePolicyCallable, service: &PackageReviewNominalIdentity) -> bool {
    callable
        .declared_service_reach()
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

fn nominal(identity: &PackageReviewNominalIdentity) -> Result<(), &'static str> {
    if identity.path.is_empty() || identity.owner == PackageReviewNominalOwner::Unresolved {
        return Err("package policy nominal is empty or unresolved");
    }
    Ok(())
}

fn ordered<T: Ord>(rows: &[T]) -> Result<(), &'static str> {
    if rows.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("package policy rows are repeated or out of order");
    }
    Ok(())
}

fn representation_profile(profile: PackageReviewRepresentationTargetProfile) -> TargetProfile {
    use PackageReviewRepresentationTargetProfile as Profile;
    match profile {
        Profile::LinuxArm64 => TargetProfile::LinuxArm64,
        Profile::LinuxX64 => TargetProfile::LinuxX64,
        Profile::MacosArm64 => TargetProfile::MacosArm64,
        Profile::WindowsX64 => TargetProfile::WindowsX64,
        Profile::UefiX64 => TargetProfile::UefiX64,
        Profile::CrossPlatformCli => TargetProfile::CrossPlatformCli,
        Profile::LocalUnchecked => TargetProfile::LocalUnchecked,
    }
}
