//! Complete service meaning shared by provider and permission policy.

mod application;
use application::ApplicationSource;
mod authority;
mod calling;
mod signature;

use crate::capture::refusal;
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
        ApplicationSource::Provider(provider_type),
        requirement,
        method,
        identities,
        false,
    )
}

/// Project an exact admitted native requirement application without a
/// conformance. The receipt is joined before any source view is projected.
pub(crate) fn project_native(
    projection: &mut SignatureProjection<'_>,
    service_schema: &ServiceSchema,
    schema: ProviderSchemaDeclaration,
    requirement: SymbolHandle,
    method: &ServiceMethod,
    identities: &ProviderIdentityTable,
    native: &provider_planning::native_bindings::NativeBuildRowOrigin,
) -> Result<PackagePolicyServiceMethod, Vec<Diagnostic>> {
    let input = projection.input();
    let application = &native.selection.application;
    if application.requirement != requirement || native.requirement != requirement {
        return Err(rejected(
            "native application substituted its source requirement",
        ));
    }
    let actual = application
        .identity(input.checking_input.typed_program())
        .map_err(|reason| rejected(&reason))?;
    let original = application
        .identity(input.custody_checking_input.typed_program())
        .map_err(|reason| rejected(&reason))?;
    if actual != original {
        return Err(rejected(
            "native application differs from immutable checked source custody",
        ));
    }
    let receipt = native.origin.primary.evaluated.receipt().identity_digest();
    let retained = input
        .custody
        .providers()
        .native_build_plans
        .iter()
        .flat_map(|candidate| &candidate.provenance.row_realizations)
        .filter_map(|origin| origin.native())
        .any(|origin| {
            origin
                .selection
                .application
                .identity(input.custody_checking_input.typed_program())
                .ok()
                .as_ref()
                == Some(&original)
                && std::iter::once(&origin.origin.primary)
                    .chain(&origin.origin.equivalents)
                    .any(|production| production.evaluated.receipt().identity_digest() == receipt)
        });
    if !retained {
        return Err(rejected(
            "native application has no original admitted source and evaluation row",
        ));
    }
    let universal = application.trait_scope.is_none() && application.closed_requirement.is_none();
    project_inner(
        projection,
        service_schema,
        schema,
        ApplicationSource::Requirement(application),
        requirement,
        method,
        identities,
        universal,
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
        ApplicationSource::Provider(None),
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
    application: ApplicationSource<'_>,
    requirement: SymbolHandle,
    method: &ServiceMethod,
    identities: &ProviderIdentityTable,
    declaration_mode: bool,
) -> Result<PackagePolicyServiceMethod, Vec<Diagnostic>> {
    let input = projection.input();
    let compilation = &input;
    let declaration = provider_requirement_schema(compilation, schema, requirement)?;
    let checked_identity = provider_requirement_identity(compilation, declaration, requirement)?;
    let checked_callable = match application {
        ApplicationSource::Requirement(source) if source.closed_requirement.is_some() => {
            let closed = compilation
                .closed_requirement_application(source.closed_requirement.expect("matched above"))
                .filter(|closed| closed.template == requirement)
                .ok_or_else(|| rejected("native service lost its exact closed requirement"))?;
            let owner = compilation
                .trait_definition_by_symbol(closed.declaring_trait)
                .ok_or_else(|| rejected("native service lost its declaring trait"))?;
            compilation.trait_requirement_callable_identity_text(owner, closed.signature)
        }
        _ => checked_identity.path().to_owned(),
    };
    if checked_callable != service_schema.identity_text(method.requirement_identity) {
        return Err(rejected(
            "service method changes its exact requirement overload or application",
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
        calling::project(compilation, schema, application, requirement, method)?
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
            application,
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

refusal!(reason, "service policy rejects {reason}");
