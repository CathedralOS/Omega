//! Exact service declarations and selected calling applications.

mod authority;
mod calling;
mod signature;

use crate::capture::semantics::declarations::{
    nominal_identity, provider_requirement_identity, provider_requirement_schema,
};
use crate::record::PackagePolicyServiceMethod;
use omega_compiler::CheckedCompilation;
use omega_effects::provider_plan::ServiceMethod;
use omega_provider_planning::plans::ProviderSchemaDeclaration;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(super) fn project(
    compilation: &CheckedCompilation,
    schema: ProviderSchemaDeclaration,
    provider_type: Option<SymbolHandle>,
    requirement: SymbolHandle,
    method: &ServiceMethod,
) -> Result<PackagePolicyServiceMethod, Vec<Diagnostic>> {
    let declaration = provider_requirement_schema(compilation, schema, requirement)?;
    let identity = provider_requirement_identity(compilation, declaration, requirement)?;
    if identity.path() != method.requirement_identity {
        return Err(rejected(
            "service method changes its exact requirement overload",
        ));
    }
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
    let calling = calling::project(compilation, schema, provider_type, requirement, method)?;
    let signature = signature::project(
        compilation,
        schema,
        provider_type,
        requirement,
        &identity,
        calling.as_ref(),
    )?;
    Ok(PackagePolicyServiceMethod {
        authority: authority::project(compilation, declaration, requirement)?,
        name: method.name.clone(),
        requirement_owner: owner,
        requirement: identity,
        signature,
        parameter_count: method.parameter_count,
        parameter_type_identities: method.parameter_type_identities.clone(),
        entry_claims: method.entry_claims.clone(),
        has_result: method.has_result,
        result_type_identity: method.result_type_identity.clone(),
        result_claims: method.result_claims.clone(),
        service_reach: method.service_reach.clone(),
        synchronous_invocations: method.synchronous_invocations.clone(),
        may_suspend: method.may_suspend,
        may_block: method.may_block,
        terminates_guarantee: method.terminates_guarantee,
        termination_premises: method.termination_premises.clone(),
        calling,
    })
}

fn rejected(reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "selected provider policy rejects {reason}"
    ))]
}
