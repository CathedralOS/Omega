//! A receiving policy names exact requirements; review keeps their complete
//! service declaration, including methods to which the receiver grants nothing.
use super::telescope::SignatureScope;
use super::{
    calling, declarations::unavailable, input::PolicySource, providers::service_method_policy,
    semantic_bindings,
};
use compiler::SelectedProviderResult;
use diagnostics::Diagnostic;
use provider_acceptance::AcceptedSemanticBinding;
use provider_plans::{ServiceMethod, ServiceSchema};
use review_evidence::record::{
    PackagePolicyCallable, PackagePolicyCallableRole, PackagePolicyServiceAuthority,
    PackagePolicyServiceSignature, PackagePolicyTerminalPermission,
    PackagePolicyTerminalPermissions, PackagePolicyTerminalService, PackagePolicyTermination,
    PackageReviewSignatureParameter,
};
use review_evidence::record::{PackageReviewNominalIdentity, PackageReviewTypeIdentity};
use semantic_vocabulary::PackageKeyIdentity;
use source_compiler::representations::source_state::signature::StateSignature;
use std::collections::BTreeMap;
use target::TargetProfile;

pub(super) fn project(
    input: &PolicySource<'_>,
    selected: &SelectedProviderResult,
    target: TargetProfile,
    package: PackageKeyIdentity,
    callables: &[PackagePolicyCallable],
) -> Result<PackagePolicyTerminalPermissions, Vec<Diagnostic>> {
    if input.sources().package_identity() != Some(package)
        || selected.target_profile() != Some(target)
    {
        return Err(unavailable(
            "the exact package and target for receiver permissions",
        ));
    }
    let mut policy = PackagePolicyTerminalPermissions {
        package,
        target,
        services: Vec::new(),
    };
    let mut grants = input
        .sources()
        .accepted_semantic_bindings()
        .iter()
        .filter(|binding| !binding.terminal_authority_permissions().is_empty())
        .peekable();
    if grants.peek().is_none() {
        return Ok(policy);
    }

    let native = selected.realization();
    // A compiler primitive can carry its own receiver policy. Only an explicitly
    // supplied binding contributes to the package's review rows.
    let mut realized = BTreeMap::new();
    for row in native.terminal_authority_permissions() {
        let requirement = native
            .terminal_authority_permission_identities()
            .text(row.requirement_identity())
            .ok_or_else(|| unavailable("a realized permission's requirement identity"))?;
        if realized
            .insert((row.service_schema(), requirement), row.permitted())
            .is_some()
        {
            return Err(unavailable(
                "a unique realized receiver permission for each requirement",
            ));
        }
    }
    let mut services = BTreeMap::new();
    for binding in grants {
        let (symbol, schema) = semantic_bindings::checked_service(input, selected, binding)?;
        let service = input.nominal(symbol)?;
        let (existing_schema, reviewed) = match services.entry(service.clone()) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                let declared =
                    project_service(input, selected, target, callables, symbol, &schema)?;
                entry.insert((schema.clone(), declared))
            }
        };
        if *existing_schema != schema {
            return Err(unavailable(
                "one complete service schema for receiver permissions",
            ));
        }
        for grant in binding.terminal_authority_permissions() {
            let requirement = permission_requirement(binding, grant)?;
            if realized
                .get(&(grant.service_schema(), requirement))
                .copied()
                != Some(grant.permitted())
            {
                return Err(unavailable(
                    "the supplied receiver disposition in the selected target",
                ));
            }
            let mut methods = schema
                .declaration
                .methods
                .iter()
                .zip(&reviewed.methods)
                .filter(|(method, _)| {
                    schema.identity_text(method.requirement_identity) == requirement
                });
            let (Some((_, method)), None) = (methods.next(), methods.next()) else {
                return Err(unavailable(
                    "one exact declaration for the permitted requirement",
                ));
            };
            reviewed.permissions.push(PackagePolicyTerminalPermission {
                requirement: method.requirement.clone(),
                permitted: grant.permitted().clone(),
            });
        }
    }
    for (_, mut service) in services.into_values() {
        service
            .permissions
            .sort_by(|a, b| a.requirement.cmp(&b.requirement));
        if service
            .permissions
            .windows(2)
            .any(|pair| pair[0].requirement == pair[1].requirement)
        {
            return Err(unavailable(
                "distinct explicitly supplied receiver permissions",
            ));
        }
        policy.services.push(service);
    }
    Ok(policy)
}

fn permission_requirement<'a>(
    binding: &'a AcceptedSemanticBinding,
    grant: &provider_acceptance::ServiceTerminalAuthorityPermission,
) -> Result<&'a str, Vec<Diagnostic>> {
    if grant.service_schema() != binding.normalized_schema_digest() {
        return Err(unavailable(
            "the exact accepted service for a receiver permission",
        ));
    }
    binding
        .terminal_authority_permission_identities()
        .text(grant.requirement_identity())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| unavailable("the explicit receiver permission's requirement"))
}

fn project_service(
    input: &PolicySource<'_>,
    selected: &SelectedProviderResult,
    target: TargetProfile,
    callables: &[PackagePolicyCallable],
    symbol: symbols::SymbolHandle,
    schema: &ServiceSchema,
) -> Result<PackagePolicyTerminalService, Vec<Diagnostic>> {
    let source = input.typed();
    let slot = source
        .trait_definition_by_symbol(symbol)
        .filter(|slot| slot.is_boundary)
        .ok_or_else(|| unavailable("a checked boundary service for receiver permissions"))?;
    let service = input.nominal(symbol)?;
    let mut signatures = BTreeMap::new();
    for signature in source.trait_machine_signatures(slot) {
        let identity = source.trait_requirement_callable_identity_text(slot, signature);
        if signatures.insert(identity, signature).is_some() {
            return Err(unavailable(
                "a unique callable identity in the declared service",
            ));
        }
    }
    let service_scope = SignatureScope::service(input, symbol)?;
    let static_parameters = input.scoped_parameters(symbol, &service_scope)?;
    let lifetime_parameter_count: u32 = slot
        .lifetime_parameters
        .len()
        .try_into()
        .map_err(|_| unavailable("a representable service lifetime count"))?;
    let schema_arguments = (0..static_parameters.len())
        .map(|ordinal| {
            review_evidence::record::PackageReviewTypeIdentity::service_parameter(ordinal)
                .map_err(|_| unavailable("the canonical identity of a service declaration binder"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if signatures.len() != schema.declaration.methods.len() {
        return Err(unavailable(
            "the complete service method inventory for receiver permissions",
        ));
    }
    let mut methods = Vec::with_capacity(signatures.len());
    for method in &schema.declaration.methods {
        let identity = schema.identity_text(method.requirement_identity);
        let signature = signatures.remove(identity).ok_or_else(|| {
            unavailable("the original declaration of every permission-service method")
        })?;
        let requirement = input.nominal(signature.symbol)?;
        let mut published = callables.iter().filter(|callable| {
            callable.role == PackagePolicyCallableRole::Boundary && callable.identity == requirement
        });
        let foreign;
        let callable = match (published.next(), published.next()) {
            (Some(callable), None) => callable,
            (None, None) => {
                // A receiver may accept a service from another package. Its own
                // public inventory then omits the supplier's callable.
                foreign = input.declared_boundary(&input.requirement(slot, signature)?)?;
                &foreign
            }
            _ => {
                return Err(unavailable(
                    "one checked contract for a receiver permission",
                ));
            }
        };
        let formals = source.state_signature_parameters(signature);
        if callable.identity != requirement
            || callable.parameters.len() != formals.len()
            || method.parameter_count != formals.len()
            || callable.return_type.is_some() != method.has_result
            || callable.declared_may_suspend != Some(method.may_suspend)
            || callable.declared_may_block != Some(method.may_block)
        {
            return Err(unavailable(
                "the same checked method in the service and source policies",
            ));
        }
        let authority = service_authority(callable, method, &service)?;
        let scope = service_scope.method(input, signature.symbol)?;
        let reviewed_signature = declaration_signature(
            input,
            signature,
            &scope,
            &schema_arguments,
            lifetime_parameter_count,
        )?;
        let mut reviewed = service_method_policy(
            method,
            schema,
            requirement,
            service.clone(),
            reviewed_signature,
            authority,
        );
        // Declaration-mode display uses the policy type identities, including
        // qualifications that the provider's runtime parameter names omit.
        reviewed.parameter_type_identities = reviewed
            .signature
            .parameters
            .iter()
            .map(|parameter| parameter.type_identity.canonical.clone())
            .collect();
        reviewed.result_type_identity = reviewed
            .signature
            .result
            .as_ref()
            .map(|ty| ty.canonical.clone());
        for retained in schema.calling_for_requirement(method.requirement_identity) {
            let mut matches =
                selected
                    .realization()
                    .calling_applications()
                    .iter()
                    .filter(|application| {
                        application.requirement_identity == identity
                            && application.abi == retained.abi
                            && application.report_fingerprint == retained.plan.report_fingerprint
                            && application.commitment == retained.plan.commitment.as_bytes()
                            && application.plan.contract_commitment_digest()
                                == retained.plan.promise.as_bytes()
                    });
            let (Some(application), None) = (matches.next(), matches.next()) else {
                return Err(unavailable(
                    "the exact checked calling application retained by the permission schema",
                ));
            };
            reviewed
                .calling
                .push(calling::project(application, &reviewed, target)?);
        }
        methods.push(reviewed);
    }
    Ok(PackagePolicyTerminalService {
        service,
        static_parameters,
        lifetime_parameter_count,
        methods,
        permissions: Vec::new(),
    })
}

fn service_authority(
    callable: &PackagePolicyCallable,
    method: &ServiceMethod,
    service: &PackageReviewNominalIdentity,
) -> Result<PackagePolicyServiceAuthority, Vec<Diagnostic>> {
    match (&callable.declared_termination, method.terminates_guarantee) {
        (Some(PackagePolicyTermination::Terminates { premises }), true) if premises.is_empty() => {}
        (Some(PackagePolicyTermination::NoGuarantee), false) => (),
        _ => {
            return Err(unavailable(
                "the checked progress contract of a permission-service method",
            ));
        }
    }
    if !method.termination_premises.is_empty() {
        return Err(unavailable(
            "the checked progress premises of a permission-service method",
        ));
    }
    let mut reach = callable.declared_service_reach.clone().ok_or_else(|| {
        unavailable("the exact declared authority of a permission-service method")
    })?;
    reach.push(service.clone());
    reach.sort_unstable();
    reach.dedup();
    Ok(PackagePolicyServiceAuthority {
        service_reach: reach,
        synchronous_invocations: callable
            .declared_synchronous_invocations
            .clone()
            .ok_or_else(|| {
                unavailable("the synchronous invocation contract of a permission-service method")
            })?,
        progress_premises: Vec::new(),
    })
}

fn declaration_signature(
    input: &PolicySource<'_>,
    signature: &StateSignature,
    scope: &SignatureScope,
    schema_arguments: &[PackageReviewTypeIdentity],
    lifetime_parameter_count: u32,
) -> Result<PackagePolicyServiceSignature, Vec<Diagnostic>> {
    let formals = input.typed().state_signature_parameters(signature);
    let parameters = formals
        .iter()
        .map(|formal| {
            Ok(PackageReviewSignatureParameter {
                name: formal.name.as_str().to_owned(),
                type_identity: input.scoped_type(
                    signature.symbol,
                    formal.type_reference,
                    scope,
                )?,
                is_const: formal.is_const,
                is_mutable: formal.is_mutable,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    Ok(PackagePolicyServiceSignature {
        schema_arguments: schema_arguments.to_vec(),
        schema_lifetime_parameter_count: lifetime_parameter_count,
        schema_lifetime_arguments: (0..lifetime_parameter_count).collect(),
        application_lifetime_parameter_count: 0,
        requirement_arguments: Vec::new(),
        requirement_lifetime_arguments: Vec::new(),
        requirement_lifetime_parameter_count: signature
            .lifetime_parameters
            .len()
            .try_into()
            .map_err(|_| unavailable("the number of requirement lifetime parameters"))?,
        static_parameters: input.scoped_parameters(signature.symbol, scope)?,
        parameters,
        result: signature
            .return_type
            .is_valid()
            .then(|| input.scoped_type(signature.symbol, signature.return_type, scope))
            .transpose()?,
    })
}
