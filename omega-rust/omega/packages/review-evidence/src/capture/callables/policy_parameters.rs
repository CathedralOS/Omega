//! Scope-preserving static telescopes for complete callable policy.

use crate::capture::PackageReviewInput;
use crate::capture::calling::application::signature::instantiate_static_parameters;
use crate::capture::semantics::signatures::callable_declaration_scope;
use crate::capture::semantics::signatures::policy::project_type_parameters;
use crate::record::PackagePolicyTypeParameter;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

pub(super) fn type_parameters(
    compilation: &PackageReviewInput<'_>,
    machine: &typed_trees::machine::Machine,
    subject: &str,
    public_nominals: bool,
) -> Result<(Vec<(SymbolHandle, String)>, Vec<PackagePolicyTypeParameter>), Vec<Diagnostic>> {
    let scope = callable_declaration_scope(compilation, machine)?;
    let mut parameters = scope.type_parameters.to_vec();
    if parameters.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    // Owner telescope handles belong to immutable checked custody. Equal
    // candidate handles alone cannot authenticate their nested type/contracts.
    let mut projected = validation::CheckingProjection::new(scope.source.checking_input);
    let lifetimes = scope
        .lifetime_parameters
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
        scope.lifetime_parameters,
        &mut scopes,
        0,
    )?;
    project_type_parameters(
        &projected,
        &scope.source,
        &parameters,
        scope.type_parameters,
        subject,
        &[],
        0,
        scope.lifetime_parameters,
        &[],
        &scopes,
        public_nominals,
        crate::capture::contracts::facts::callable_contract_exposure(machine),
    )
}
