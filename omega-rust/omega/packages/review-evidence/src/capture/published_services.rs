//! Service review rejoins admitted schema and calling rows with the immutable
//! declaration catalog. It does not derive source proof or provider eligibility.

use super::published_input::{PublishedPackageReviewInput, rejected};
use super::published_projection::{nominal, signature_type_identity};
use crate::record::*;
use diagnostics::Diagnostic;
use effects::provider_plan::{ServiceMethod, ServiceSchema};
use provider_planning::calling_policy_plans::BoundaryCallingPlanRealization;
use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;
use target::TargetProfile;

type Review<T> = Result<T, Vec<Diagnostic>>;

pub(super) fn requirement_identity(
    input: PublishedPackageReviewInput<'_>,
    requirement: SymbolHandle,
) -> Review<PackageReviewNominalIdentity> {
    let typed = input.candidate;
    let (owner, signature) = typed
        .trait_machine_signature(requirement)
        .ok_or_else(|| rejected("service requirement has no exact declaring signature"))?;
    let nominal = nominal(input, requirement)?;
    Ok(PackageReviewNominalIdentity {
        owner: nominal.owner,
        path: typed.trait_requirement_callable_identity_text(owner, signature),
    })
}

pub(super) fn method(
    input: PublishedPackageReviewInput<'_>,
    schema_symbol: SymbolHandle,
    requirement: SymbolHandle,
    schema: &ServiceSchema,
    method: &ServiceMethod,
) -> Review<PackagePolicyServiceMethod> {
    let typed = input.candidate;
    let (owner, signature) = typed
        .trait_machine_signature(requirement)
        .ok_or_else(|| rejected("service method lost its exact signature"))?;
    if owner.symbol != schema_symbol
        || !typed.trait_type_parameters(owner).is_empty()
        || !owner.lifetime_parameters.is_empty()
        || !typed.state_signature_type_parameters(signature).is_empty()
        || !signature.lifetime_parameters.is_empty()
        || !owner.conformance_bounds.is_empty()
        || !signature.conformance_bounds.is_empty()
    {
        return Err(rejected(
            "generic service needs its complete applied signature and lifetime projection",
        ));
    }
    let requirement_identity = requirement_identity(input, requirement)?;
    if schema.identity_text(method.requirement_identity) != requirement_identity.path {
        return Err(rejected(
            "service method differs from its exact requirement identity",
        ));
    }
    let parameters = typed
        .state_signature_parameters(signature)
        .iter()
        .map(|parameter| {
            Ok(PackageReviewSignatureParameter {
                name: parameter.name.name.to_string(),
                type_identity: signature_type_identity(
                    typed,
                    parameter.type_reference,
                    input.custody.sources().exact_toolchain_sources(),
                )?,
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
            })
        })
        .collect::<Review<Vec<_>>>()?;
    let result = method
        .has_result
        .then(|| {
            signature_type_identity(
                typed,
                signature.return_type,
                input.custody.sources().exact_toolchain_sources(),
            )
        })
        .transpose()?;
    if parameters.len() != method.parameter_count || result.is_some() != method.has_result {
        return Err(rejected(
            "service schema changed its complete formal/result telescope",
        ));
    }
    if !method.termination_premises.is_empty() {
        return Err(rejected(
            "service progress premises need their exact source domain and establishment routes",
        ));
    }
    let mut reach = Vec::new();
    for identity in &method.service_reach {
        let mut matching = typed.service_reaches.definitions().iter().filter(|row| {
            typed
                .normalized_hermetic_symbol_identity(row.symbol)
                .as_deref()
                == Ok(identity.as_str())
        });
        let (Some(row), None) = (matching.next(), matching.next()) else {
            return Err(rejected("service reach lost its exact source declaration"));
        };
        reach.push(nominal(input, row.symbol)?);
    }
    reach.sort();
    reach.dedup();
    if !method.synchronous_invocations.is_empty() {
        return Err(rejected(
            "service synchronous invocations need their exact positional/source target projection",
        ));
    }
    let calling = calling(input, schema_symbol, requirement, schema, method)?;
    let parameter_type_identities = parameters
        .iter()
        .map(|row| row.type_identity.canonical.clone())
        .collect();
    let result_type_identity = result.as_ref().map(|row| row.canonical.clone());
    Ok(PackagePolicyServiceMethod {
        completion: method.completion,
        name: method.name.clone(),
        requirement_owner: nominal(input, schema_symbol)?,
        requirement: requirement_identity,
        signature: PackagePolicyServiceSignature {
            schema_arguments: Vec::new(),
            schema_lifetime_parameter_count: 0,
            schema_lifetime_arguments: Vec::new(),
            application_lifetime_parameter_count: 0,
            requirement_arguments: Vec::new(),
            requirement_lifetime_arguments: Vec::new(),
            requirement_lifetime_parameter_count: 0,
            static_parameters: Vec::new(),
            parameters,
            result,
        },
        authority: PackagePolicyServiceAuthority {
            service_reach: reach,
            synchronous_invocations: Vec::new(),
            progress_premises: Vec::new(),
        },
        parameter_count: method.parameter_count,
        parameter_type_identities,
        entry_claims: method.entry_claims.clone(),
        has_result: method.has_result,
        result_type_identity,
        result_claims: method.result_claims.clone(),
        service_reach: method.service_reach.clone(),
        synchronous_invocations: method.synchronous_invocations.clone(),
        may_suspend: method.may_suspend,
        may_block: method.may_block,
        terminates_guarantee: method.terminates_guarantee,
        termination_premises: method.termination_premises.clone(),
        identities: schema.declaration.identities.clone(),
        calling,
    })
}

fn calling(
    input: PublishedPackageReviewInput<'_>,
    owner: SymbolHandle,
    requirement: SymbolHandle,
    schema: &ServiceSchema,
    method: &ServiceMethod,
) -> Review<Vec<PackagePolicyCallingPlan>> {
    let applications = schema.calling_for_requirement(method.requirement_identity);
    let mut rows = Vec::new();
    for application in applications {
        let mut candidates = input
            .custody
            .providers()
            .boundary_calling_plan_realizations
            .iter()
            .filter(|row| {
                row.boundary_trait == owner
                    && row.requirement_machine == requirement
                    && row.materialized_signature().abi() == &application.abi
            });
        let candidate = candidates
            .next()
            .ok_or_else(|| rejected("service method lost an admitted calling application"))?;
        if candidates.any(|other| other != candidate)
            || candidate.report_fingerprint != application.plan.report_fingerprint
            || candidate.commitment != application.plan.commitment
        {
            return Err(rejected(
                "service method changed its exact calling application custody",
            ));
        }
        rows.push(calling_plan(input, candidate)?);
    }
    Ok(rows)
}

fn calling_plan(
    input: PublishedPackageReviewInput<'_>,
    realization: &BoundaryCallingPlanRealization,
) -> Review<PackagePolicyCallingPlan> {
    let typed = input.candidate;
    if !realization.boundary_arguments.is_empty() {
        return Err(rejected(
            "calling application needs its complete supplied generic arguments",
        ));
    }
    let (owner, signature) = typed
        .trait_machine_signature(realization.requirement_machine)
        .ok_or_else(|| rejected("calling application lost its exact requirement signature"))?;
    if owner.symbol != realization.boundary_trait
        || !typed.trait_type_parameters(owner).is_empty()
        || !owner.lifetime_parameters.is_empty()
        || !typed.state_signature_type_parameters(signature).is_empty()
        || !signature.lifetime_parameters.is_empty()
    {
        return Err(rejected(
            "calling application needs its complete source telescope",
        ));
    }
    let materialized = realization.materialized_signature();
    let source = input.custody.source_inspection();
    let catalog = source
        .inventory()
        .services_for(source.program().as_ref())
        .map_err(|reason| vec![reason])?;
    let source_abi = catalog
        .signatures()
        .abi_signatures()
        .iter()
        .find(|row| row.id == materialized.abi().id())
        .ok_or_else(|| rejected("calling application lost its source-owned declaration ABI"))?;
    let source_key = semantic_vocabulary::CallingAbiIdentity {
        requirement_identity: source_abi.requirement_identity.clone(),
        declaring_application: source_abi.declaring_application.clone(),
        operands: source_abi.operands.clone(),
    };
    if &source_key != materialized.abi() {
        return Err(rejected(
            "calling application changed its complete source ABI identity",
        ));
    }

    if !materialized.callback_binders().is_empty()
        || !materialized.callback_demands().is_empty()
        || !materialized.callback_layout_catalog().is_empty()
        || !materialized.direct_callback_parameters().is_empty()
    {
        return Err(rejected(
            "calling application needs its complete callback binder and layout projection",
        ));
    }
    if !materialized.opaque_representation_uses().is_empty() {
        return Err(rejected(
            "calling application needs its exact opaque representation selections",
        ));
    }
    if input.custody.execution().selected_native_target() != Some(materialized.native_target()) {
        return Err(rejected(
            "calling application changed its admitted native target",
        ));
    }
    let (validated, report, commitment) = realization
        .replayed_validated_application()
        .map_err(|reason| rejected(&format!("invalid admitted calling application: {reason}")))?;
    if report == 0
        || report != realization.report_fingerprint
        || commitment != realization.commitment
    {
        return Err(rejected(
            "calling application changed its retained proof commitment",
        ));
    }
    let parameters = typed.state_signature_parameters(signature);
    if parameters.len() != materialized.parameters().len() {
        return Err(rejected("calling application omitted a semantic formal"));
    }
    let semantic_parameters = parameters
        .iter()
        .zip(materialized.parameters())
        .map(|(parameter, shape)| {
            Ok(PackagePolicyCallingParameter {
                name: parameter.name.name.to_string(),
                value_type: signature_type_identity(
                    typed,
                    parameter.type_reference,
                    input.custody.sources().exact_toolchain_sources(),
                )?,
                is_mutable: parameter.is_mutable,
                is_const: parameter.is_const,
                shape_root: *shape,
            })
        })
        .collect::<Review<Vec<_>>>()?;
    let semantic_result = materialized
        .result()
        .is_some()
        .then(|| {
            signature_type_identity(
                typed,
                signature.return_type,
                input.custody.sources().exact_toolchain_sources(),
            )
        })
        .transpose()?;
    if semantic_result.is_some() != materialized.result().is_some() {
        return Err(rejected("calling application changed its semantic result"));
    }
    let callbacks = PackagePolicyCallbacks {
        binders: Vec::new(),
        demands: Vec::new(),
        materializations: Vec::new(),
        layouts: Vec::new(),
    };
    let native_parameters =
        super::native_parameters::project(materialized, &semantic_parameters, &callbacks)?;
    Ok(PackagePolicyCallingPlan {
        abi: materialized.abi().clone(),
        boundary_trait: nominal(input, owner.symbol)?,
        boundary_arguments: Vec::new(),
        boundary_lifetime_parameter_count: 0,
        boundary_lifetime_arguments: Vec::new(),
        application_lifetime_parameter_count: 0,
        requirement: requirement_identity(input, signature.symbol)?,
        requirement_trait: nominal(input, owner.symbol)?,
        requirement_arguments: Vec::new(),
        requirement_lifetime_arguments: Vec::new(),
        requirement_lifetime_parameter_count: 0,
        static_parameters: Vec::new(),
        target: super::published_projection::representation_target(
            input
                .custody
                .execution()
                .selected_target_profile
                .ok_or_else(|| rejected("calling application has no selected target profile"))?,
        )?,
        shape_graph: super::physical_geometry::project_boundary_shape_graph(materialized),
        semantic_parameters,
        semantic_result,
        native_parameters,
        callbacks,
        opaque_uses: Vec::new(),
        physical: PackagePolicyPhysicalCallingContract::from_validated_plan(&validated),
    })
}

pub(super) fn permissions(
    input: PublishedPackageReviewInput<'_>,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Review<PackagePolicyTerminalPermissions> {
    let inventory = input.custody.source_inspection().inventory();
    let catalog = inventory.retained_service_catalog();
    let declarations = provider_planning::service_declarations::ServiceDeclarations::prepare(
        catalog.declared_traits(),
        &input.custody.providers().boundary_calling_plan_realizations,
    );
    let mut services = Vec::new();
    for accepted in input.custody.execution().accepted_semantic_bindings() {
        let resolved = input
            .custody
            .execution()
            .resolved_semantic_binding(accepted.role())
            .ok_or_else(|| rejected("accepted service binding lost its exact declaration"))?;
        if resolved.accepted() != accepted {
            return Err(rejected("service binding changed after acceptance"));
        }
        let declaration = declarations
            .rejoin(resolved.declaration())
            .map_err(|reason| vec![reason])?;
        let service = nominal(input, declaration.symbol)?;
        if service.owner != PackageReviewNominalOwner::Package(package) {
            continue;
        }
        let schema = &declaration.declaration.schema;
        let digest = package_compilation::accepted_service_schema_digest(accepted.role(), schema);
        if digest != accepted.normalized_schema_digest() {
            return Err(rejected("service permission changed its normalized schema"));
        }
        let mut permissions = Vec::new();
        for permission in accepted.terminal_authority_permissions() {
            if permission.service_schema() != digest {
                return Err(rejected(
                    "permission names a different accepted service schema",
                ));
            }
            let mut matching = schema
                .declaration
                .methods
                .iter()
                .zip(declaration.requirements)
                .filter(|(method, _)| {
                    accepted
                        .terminal_authority_permission_identities()
                        .text(permission.requirement_identity())
                        == Some(schema.identity_text(method.requirement_identity))
                });
            let (Some((_, requirement)), None) = (matching.next(), matching.next()) else {
                return Err(rejected("permission lost its unique complete requirement"));
            };
            permissions.push(PackagePolicyTerminalPermission {
                requirement: requirement_identity(input, *requirement)?,
                permitted: permission.permitted().clone(),
            });
        }
        if permissions.is_empty() {
            continue;
        }
        let methods = schema
            .declaration
            .methods
            .iter()
            .zip(declaration.requirements)
            .map(|(row, requirement)| method(input, declaration.symbol, *requirement, schema, row))
            .collect::<Review<Vec<_>>>()?;
        permissions.sort_by(|a, b| a.requirement.cmp(&b.requirement));
        services.push(PackagePolicyTerminalService {
            service,
            static_parameters: Vec::new(),
            lifetime_parameter_count: 0,
            methods,
            permissions,
        });
    }
    services.sort_by(|a, b| a.service.cmp(&b.service));
    Ok(PackagePolicyTerminalPermissions {
        package,
        target,
        services,
    })
}
