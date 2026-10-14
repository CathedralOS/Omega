//! Complete service meaning shared by provider and permission policy.

mod authority;
mod calling;
mod signature;

use crate::capture::semantics::declarations::{
    nominal_identity, policy_provider_requirement_identity, provider_requirement_identity,
    provider_requirement_schema,
};
use crate::record::PackagePolicyServiceMethod;
use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderIdentityTable, ServiceMethod, ServiceSchema};
use provider_planning::ProviderSchemaDeclaration;
use symbols::SymbolHandle;

pub(crate) use crate::capture::calling::application::signature::declaration_parameters;
pub(crate) use crate::capture::semantics::signatures::SignatureProjection;

pub(crate) fn project(
    projection: &mut SignatureProjection<'_>,
    service_schema: &ServiceSchema,
    schema: ProviderSchemaDeclaration,
    provider_type: Option<SymbolHandle>,
    requirement: SymbolHandle,
    method: &ServiceMethod,
    identities: &ProviderIdentityTable,
) -> Result<PackagePolicyServiceMethod, Vec<Diagnostic>> {
    project_inner(
        projection,
        service_schema,
        schema,
        provider_type,
        requirement,
        method,
        identities,
        false,
    )
}

/// Review an accepted service declaration without inventing a provider or a
/// closed generic calling application.
pub(crate) fn project_declaration(
    projection: &mut SignatureProjection<'_>,
    service_schema: &ServiceSchema,
    schema: SymbolHandle,
    requirement: SymbolHandle,
    method: &ServiceMethod,
    identities: &ProviderIdentityTable,
) -> Result<PackagePolicyServiceMethod, Vec<Diagnostic>> {
    project_inner(
        projection,
        service_schema,
        ProviderSchemaDeclaration::BoundaryTrait(schema),
        None,
        requirement,
        method,
        identities,
        true,
    )
}

fn project_inner(
    projection: &mut SignatureProjection<'_>,
    service_schema: &ServiceSchema,
    schema: ProviderSchemaDeclaration,
    provider_type: Option<SymbolHandle>,
    requirement: SymbolHandle,
    method: &ServiceMethod,
    identities: &ProviderIdentityTable,
    declaration_mode: bool,
) -> Result<PackagePolicyServiceMethod, Vec<Diagnostic>> {
    let input = projection.input();
    let compilation = &input;
    let declaration = provider_requirement_schema(compilation, schema, requirement)?;
    let checked_identity = provider_requirement_identity(compilation, declaration, requirement)?;
    if checked_identity.path() != service_schema.identity_text(method.requirement_identity) {
        return Err(rejected(
            "service method changes its exact requirement overload",
        ));
    }
    let identity = policy_provider_requirement_identity(compilation, declaration, requirement)?;
    let mut owner = nominal_identity(compilation, declaration.symbol())?;
    // Top-level requirements publish their enclosing namespace as owner; it
    // is checked by schema replay, not resolved by searching that spelling.
    if matches!(
        declaration,
        ProviderSchemaDeclaration::BoundaryRequirement(_)
    ) {
        owner.path = method.requirement_owner.clone();
    } else if matches!(declaration, ProviderSchemaDeclaration::BoundaryOperator(_)) {
        owner = identity.clone();
    }
    crate::capture::providers::selection::validate_selected_provider_declaration_owner(
        &owner,
        method.requirement_owner_package_identity,
        &method.name,
        "requirement owner",
    )?;
    let generic_declaration = declaration_mode
        && compilation
            .trait_definition_by_symbol(schema.symbol())
            .is_some_and(|owner| !compilation.trait_type_parameters(owner).is_empty());
    let calling = if generic_declaration {
        // A declaration publishes its open telescope. It has no supplying
        // conformance and cannot borrow a concrete application's calling plan.
        if method.calling_plan_report_fingerprint.is_some()
            || method.calling_plan_commitment.is_some()
        {
            return Err(rejected(
                "uninstantiated generic service carries a closed calling application",
            ));
        }
        None
    } else {
        calling::project(compilation, schema, provider_type, requirement, method)?
    };
    let signature = if declaration_mode {
        signature::project_declaration(
            projection,
            schema.symbol(),
            requirement,
            &identity,
            calling.as_ref(),
        )?
    } else {
        signature::project(
            projection,
            schema,
            provider_type,
            requirement,
            &identity,
            calling.as_ref(),
        )?
    };
    // Declaration and operator policies retain typed binder coordinates rather
    // than the source binder names in the replay schema's diagnostic strings.
    // The original strings were already checked above against the live schema.
    let normalized_signature_display =
        declaration_mode || matches!(declaration, ProviderSchemaDeclaration::BoundaryOperator(_));
    let parameter_type_identities = if normalized_signature_display {
        signature
            .parameters
            .iter()
            .map(|parameter| parameter.type_identity.canonical().to_owned())
            .collect()
    } else {
        method
            .parameter_type_identities
            .iter()
            .map(|identity| identities.text(*identity).to_owned())
            .collect()
    };
    let result_type_identity = if normalized_signature_display {
        signature
            .result
            .as_ref()
            .map(|identity| identity.canonical().to_owned())
    } else {
        method
            .has_result
            .then(|| identities.text(method.result_type_identity).to_owned())
    };
    Ok(PackagePolicyServiceMethod {
        completion: method.completion,
        authority: authority::project(compilation, declaration, requirement)?,
        name: method.name.clone(),
        requirement_owner: owner,
        requirement: identity,
        signature,
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
        identities: service_schema.identities.clone(),
        calling,
    })
}

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "service policy rejects {reason}"
    ))]
}
