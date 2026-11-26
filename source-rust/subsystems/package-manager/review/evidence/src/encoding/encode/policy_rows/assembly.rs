// Row counting and whole-baseline row projection across public API, callables, providers, terminal permissions, representation, and authority obligations.

use super::super::{baseline, callable_policy, selected_providers, values};
use super::Builder;
use crate::encoding::PackageReviewEncodingError;
use crate::encoding::encode::policy_rows::components;
use crate::encoding::encode::policy_rows::declarations;
use crate::encoding::encode::policy_rows::rejected;
use crate::encoding::encode::values::identity::encode_service;
use crate::record::PackagePolicyBaseline;
use crate::record::PackagePolicyBoundaryApplications;
use crate::record::PackagePolicyCallables;
use crate::record::PackagePolicyRowKind;
use crate::record::PackageReviewCallableSupply;
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
        api.type_aliases.as_deref().map_or(0, <[_]>::len),
        policy.callables.callables.len(),
        policy.terminal_permissions.services.len(),
        representation.declarations.len(),
        representation.producer_availability.len(),
        representation.selected_availability.len(),
        representation.demands.len(),
        policy.external_supplies.len(),
        policy.native_bindings.len(),
        policy.dangerous_capabilities.len(),
        policy.slack_uses.len(),
        policy.semantic_dependencies.len(),
        policy.boundary_applications.demands.len(),
        policy.restricted_build_requests.len(),
        policy.service_audits.len(),
        policy.component_assumptions.len(),
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

pub(super) fn acceptance_count(
    policy: &PackagePolicyBaseline,
) -> Result<usize, PackageReviewEncodingError> {
    [
        policy
            .callables
            .callables
            .iter()
            .filter(|callable| {
                matches!(
                    callable.supply,
                    PackageReviewCallableSupply::AdmissionClaim
                        | PackageReviewCallableSupply::ExternalRealization
                )
            })
            .count(),
        policy.external_supplies.len(),
        policy.native_bindings.len(),
        policy.dangerous_capabilities.len(),
        policy.restricted_build_requests.len(),
        policy.service_audits.len(),
        policy.component_assumptions.len(),
    ]
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
            .ok_or_else(|| rejected("acceptance row count overflows"))
    })
}

// Shared by the review baseline and root Product admission so a lock row
// accepted at review is the exact row admission recomputes.
pub(super) fn component_assumptions_rows(
    builder: &mut Builder,
    rows: &[crate::record::PackagePolicyComponentAssumption],
) -> Result<(), PackageReviewEncodingError> {
    for assumption in rows {
        builder.push(
            PackagePolicyRowKind::ComponentAssumption,
            true,
            true,
            |encoder| {
                encoder.package_identity(assumption.component);
                encoder.fixed_bytes(&assumption.assumption);
                Ok(())
            },
            |encoder| baseline::component_assumption(encoder, assumption),
        )?;
    }
    Ok(())
}

pub(super) fn project(
    builder: &mut Builder,
    policy: &PackagePolicyBaseline,
) -> Result<(), PackageReviewEncodingError> {
    let PackagePolicyBaseline {
        schema_version,
        package,
        target,
        public_api,
        callables,
        selected_providers,
        terminal_permissions,
        representation,
        external_supplies,
        native_bindings,
        dangerous_capabilities,
        slack_uses,
        semantic_dependencies: _,
        boundary_applications,
        restricted_build_requests,
        service_audits,
        component_assumptions,
    } = policy;
    builder.push(
        PackagePolicyRowKind::Header,
        false,
        false,
        |_| Ok(()),
        |encoder| {
            encoder.package_identity_field("package", *package)?;
            encoder.string_field("target", target.identity().as_str())
        },
    )?;
    declarations::project(builder, public_api, *schema_version)?;
    for service in service_audits {
        builder.push(
            PackagePolicyRowKind::ServiceAudit,
            true,
            true,
            |encoder| encode_service(encoder, service),
            |encoder| encode_service(encoder, service),
        )?;
    }
    component_assumptions_rows(builder, component_assumptions)?;
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
            |encoder| {
                encode_nominal(encoder, &callable.identity)?;
                values::identity::encode_target_context(encoder, &callable.target_context)
            },
            |encoder| callable_policy::encode_callable(encoder, callable),
        )?;
    }
    let PackagePolicyBoundaryApplications {
        demands,
        realizations,
    } = boundary_applications;
    let provider_schema = if selected_providers.has_application_grants() {
        crate::encoding::BASELINE_WITH_PROVIDER_APPLICATION_GRANTS
    } else {
        crate::encoding::BASELINE_WITHOUT_STRUCTURAL_PARAMETERS
    };
    builder.push_versioned(
        (
            PackagePolicyRowKind::SelectedProviderAssociation,
            provider_schema,
        ),
        false,
        false,
        |_| Ok(()),
        |encoder| {
            encoder.field_by(
                "selected_providers",
                selected_providers,
                selected_providers::policy,
            )?;
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
    for native in native_bindings {
        builder.push(
            PackagePolicyRowKind::NativeBinding,
            true,
            true,
            |encoder| {
                encode_nominal(encoder, &native.method.requirement)?;
                let producer = native
                    .producer()
                    .ok_or_else(|| rejected("native row has no Build producer"))?;
                encode_nominal(encoder, &producer.declaration)?;
                encoder.string(&producer.callable_identity)
            },
            |encoder| baseline::native_binding(encoder, native),
        )?;
    }
    for supply in external_supplies {
        builder.push(
            PackagePolicyRowKind::ExternalSupply,
            true,
            true,
            |encoder| {
                encode_nominal(encoder, supply.callable())?;
                values::identity::encode_target_context(encoder, supply.target_context())
            },
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
    for dependency in policy
        .validated_semantic_dependencies()
        .map_err(PackageReviewEncodingError::new)?
    {
        if let crate::record::PackagePolicySemanticDependency::CarriedType {
            dependency: nominal,
            ..
        } = dependency
        {
            builder.push_versioned(
                (
                    PackagePolicyRowKind::SemanticDependency,
                    crate::encoding::BASELINE_WITH_CARRIED_TYPES,
                ),
                false,
                false,
                |encoder| baseline::carried_type_key(encoder, nominal),
                |encoder| baseline::semantic_dependency(encoder, dependency),
            )?;
            continue;
        }
        builder.push(
            PackagePolicyRowKind::SemanticDependency,
            false,
            false,
            |encoder| baseline::semantic_dependency(encoder, dependency),
            |encoder| baseline::semantic_dependency(encoder, dependency),
        )?;
    }
    for (ordinal, request) in restricted_build_requests.iter().enumerate() {
        let ordinal = u64::try_from(ordinal)
            .map_err(|_| rejected("restricted build request ordinal overflows"))?;
        builder.push(
            PackagePolicyRowKind::RestrictedBuildRequest,
            true,
            true,
            |encoder| {
                let (name, tag) = baseline::restricted_build::operation(&request.operation);
                encoder.tag(name, tag);
                encoder.u64(ordinal);
                Ok(())
            },
            |encoder| baseline::restricted_build::request(encoder, request),
        )?;
    }
    Ok(())
}
