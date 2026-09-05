//! Scope-preserving static telescopes for complete callable policy.

use crate::capture::calling::application::signature::instantiate_static_parameters;
use crate::capture::semantics::signatures::policy::project_type_parameters;
use crate::record::PackagePolicyTypeParameter;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(super) fn type_parameters(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    subject: &str,
    public_nominals: bool,
) -> Result<(Vec<(SymbolHandle, String)>, Vec<PackagePolicyTypeParameter>), Vec<Diagnostic>> {
    let mut parameters = compilation.machine_type_parameters(machine).to_vec();
    if parameters.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut projected = compilation.clone();
    let lifetimes = machine
        .lifetime_parameters
        .iter()
        .cloned()
        .map(|name| (name.clone(), name))
        .collect::<Vec<_>>();
    let mut scopes = Vec::new();
    instantiate_static_parameters(
        &mut projected,
        &mut parameters,
        &[],
        &lifetimes,
        &machine.lifetime_parameters,
        &mut scopes,
        0,
    )?;
    project_type_parameters(
        &projected,
        compilation,
        &parameters,
        compilation.machine_type_parameters(machine),
        subject,
        &[],
        0,
        &machine.lifetime_parameters,
        &[],
        &scopes,
        public_nominals,
        if machine.is_public || machine.supply_mode.is_boundary_declaration() {
            psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
        } else {
            psi_language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
        },
    )
}
