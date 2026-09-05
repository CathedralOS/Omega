use super::super::{baseline, callable_policy, selected_providers, values};
use super::*;
use values::identity::encode_nominal;

pub(super) fn count(policy: &PackagePolicyBaseline) -> Result<usize, PackageReviewEncodingError> {
    let api = &policy.public_api;
    let representation = &policy.representation;
    let counts = [
        3,
        api.traits.len(),
        api.conformances.len(),
        api.domains.len(),
        api.propositions.len(),
        api.consts.len(),
        api.operators.len(),
        api.data.len(),
        policy.callables.callables.len(),
        policy.terminal_permissions.services.len(),
        representation.declarations.len(),
        representation.producer_availability.len(),
        representation.selected_availability.len(),
        representation.demands.len(),
        policy.external_supplies.len(),
        policy.dangerous_capabilities.len(),
        policy.slack_uses.len(),
        policy.semantic_dependencies.len(),
        policy.boundary_applications.demands.len(),
    ];
    counts
        .into_iter()
        .chain(
            policy
                .terminal_permissions
                .services
                .iter()
                .map(|service| service.permissions.len()),
        )
        .try_fold(0usize, |total, count| {
            total
                .checked_add(count)
                .ok_or_else(|| rejected("package policy row count overflows"))
        })
}

pub(super) fn project(
    builder: &mut Builder,
    policy: &PackagePolicyBaseline,
) -> Result<(), PackageReviewEncodingError> {
    // Adding any aggregate family requires an explicit row-projection decision.
    let PackagePolicyBaseline {
        package,
        target,
        public_api,
        callables,
        selected_providers,
        terminal_permissions,
        representation,
        external_supplies,
        dangerous_capabilities,
        slack_uses,
        semantic_dependencies,
        boundary_applications,
    } = policy;
    builder.push(
        PackagePolicyRowKind::Header,
        false,
        false,
        |_| Ok(()),
        |encoder| {
            encoder.field("package", |encoder| {
                encoder.package_identity(*package);
                Ok(())
            })?;
            encoder.field("target", |encoder| {
                encoder.string(target.identity().as_str())
            })
        },
    )?;
    declarations::project(builder, public_api)?;
    let PackagePolicyCallables {
        package: _,
        target: _,
        callables,
    } = callables;
    for callable in callables {
        let initial = matches!(
            callable.supply,
            PackageReviewCallableSupply::AdmissionClaim
                | PackageReviewCallableSupply::ExternalRealization
        );
        let audit = callable.supply == PackageReviewCallableSupply::ExternalRealization;
        builder.push(
            PackagePolicyRowKind::Callable,
            initial,
            audit,
            |encoder| encode_nominal(encoder, &callable.identity),
            |encoder| callable_policy::encode_callable(encoder, callable),
        )?;
    }
    let PackagePolicyBoundaryApplications {
        demands,
        realizations,
    } = boundary_applications;
    // Family and closed-application indices remain inside this single association.
    builder.push(
        PackagePolicyRowKind::SelectedProviderAssociation,
        false,
        false,
        |_| Ok(()),
        |encoder| {
            encoder.field("selected_providers", |encoder| {
                selected_providers::policy(encoder, selected_providers)
            })?;
            encoder.field("closed_applications", |encoder| {
                encoder.sequence(realizations, baseline::boundary::realization)
            })
        },
    )?;
    for demand in demands {
        builder.push(
            PackagePolicyRowKind::SymbolicBoundaryDemand,
            false,
            false,
            |encoder| baseline::boundary::demand(encoder, demand),
            |encoder| baseline::boundary::demand(encoder, demand),
        )?;
    }
    components::terminal(builder, terminal_permissions)?;
    components::representation(builder, representation)?;
    for supply in external_supplies {
        builder.push(
            PackagePolicyRowKind::ExternalSupply,
            true,
            true,
            |encoder| encode_nominal(encoder, supply.callable()),
            |encoder| values::external_policy::validated_value(encoder, supply),
        )?;
    }
    for authority in dangerous_capabilities {
        builder.push(
            PackagePolicyRowKind::DangerousCapability,
            true,
            true,
            |encoder| super::super::declarations::encode_dangerous_authority(encoder, authority),
            |encoder| super::super::declarations::encode_dangerous_authority(encoder, authority),
        )?;
    }
    for slack in slack_uses {
        builder.push(
            PackagePolicyRowKind::DangerousSlack,
            false,
            true,
            |encoder| super::super::declarations::encode_dangerous_authority_slack(encoder, slack),
            |encoder| super::super::declarations::encode_dangerous_authority_slack(encoder, slack),
        )?;
    }
    for dependency in semantic_dependencies {
        // Exposure is an independent checked occurrence class. Both can coexist.
        builder.push(
            PackagePolicyRowKind::SemanticDependency,
            false,
            false,
            |encoder| baseline::semantic_dependency(encoder, dependency),
            |encoder| baseline::semantic_dependency(encoder, dependency),
        )?;
    }
    Ok(())
}
