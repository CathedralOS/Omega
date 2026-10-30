//! Scope-preserving static telescopes for complete callable policy.

use crate::capture::PackageReviewInput;
use crate::capture::calling::application::signature::instantiate_static_parameters;
use crate::capture::semantics::signatures::callable_declaration_scope;
use crate::capture::semantics::signatures::policy::project_type_parameters;
use crate::record::PackagePolicyTypeParameter;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

/// The complete lexical telescope already accepted during source formation.
/// Owner operands retain their resolved symbols; they are not inferred from
/// type text or replaced by method-local binders with the same spelling.
pub(super) struct CallableParameters {
    pub binders: Vec<(SymbolHandle, String)>,
    pub lifetime_parameters: Vec<typed_trees::name::Identifier>,
    pub policy_type_parameters: Vec<PackagePolicyTypeParameter>,
}

pub(super) fn prepare(
    compilation: &PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
    subject: &str,
    public_nominals: bool,
) -> Result<CallableParameters, Vec<Diagnostic>> {
    let scope = callable_declaration_scope(compilation, machine)?;
    let source_parameters = scope.type_parameters.as_ref();
    let lifetime_parameters = scope.lifetime_parameters.as_ref();
    let mut parameters = source_parameters.to_vec();
    if parameters.is_empty() {
        return Ok(CallableParameters {
            binders: Vec::new(),
            lifetime_parameters: lifetime_parameters.to_vec(),
            policy_type_parameters: Vec::new(),
        });
    }
    let mut projected = validation::CheckingProjection::new(scope.source.checking_input);
    let lifetimes = lifetime_parameters
        .iter()
        .cloned()
        .map(|name| (name.clone(), name))
        .collect::<Vec<_>>();
    let mut scopes = Vec::new();
    instantiate_static_parameters(
        projected.program_mut(),
        &mut parameters,
        &[],
        &lifetimes,
        lifetime_parameters,
        &mut scopes,
        0,
    )?;
    let (binders, policy_type_parameters) = project_type_parameters(
        &projected,
        &scope.source,
        &parameters,
        source_parameters,
        subject,
        &[],
        0,
        lifetime_parameters,
        &[],
        &scopes,
        public_nominals,
        crate::capture::contracts::facts::callable_contract_exposure(machine),
    )?;
    Ok(CallableParameters {
        binders,
        lifetime_parameters: lifetime_parameters.to_vec(),
        policy_type_parameters,
    })
}
