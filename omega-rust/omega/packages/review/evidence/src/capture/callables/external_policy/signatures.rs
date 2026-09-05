use super::rejected;
use crate::capture::calling::application::signature::instantiate_static_parameters;
use crate::capture::semantics::{
    conformances::project_conformance_bounds, signatures::policy::project_type_parameters,
    types::review_signature_type_identity_with_binders,
};
use crate::record::*;
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::machine::Machine;

pub(super) fn project(
    compilation: &CheckedCompilation,
    machine: &Machine,
) -> Result<
    (
        Vec<(SymbolHandle, String)>,
        PackagePolicyExternalCallableSignature,
    ),
    Vec<Diagnostic>,
> {
    let mut prepared = compilation.clone();
    let source_parameters = compilation.machine_type_parameters(machine);
    let mut parameters = source_parameters.to_vec();
    let lifetimes = machine
        .lifetime_parameters
        .iter()
        .cloned()
        .map(|name| (name.clone(), name))
        .collect::<Vec<_>>();
    let mut scopes = Vec::new();
    instantiate_static_parameters(
        &mut prepared,
        &mut parameters,
        &[],
        &lifetimes,
        &machine.lifetime_parameters,
        &mut scopes,
        0,
    )?;
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure;
    let exposure = if machine.is_public || machine.supply_mode.is_boundary_declaration() {
        AuthoredDeclarationSelectionExposure::PublicInterface
    } else {
        AuthoredDeclarationSelectionExposure::PrivateImplementation
    };
    let (binders, static_parameters) = project_type_parameters(
        &prepared,
        compilation,
        &parameters,
        source_parameters,
        machine.name.as_str(),
        &[],
        0,
        &machine.lifetime_parameters,
        &[],
        &scopes,
        false,
        exposure,
    )?;
    let conformance_bounds = project_conformance_bounds(
        compilation,
        &machine.conformance_bounds,
        source_parameters,
        &binders,
        &machine.lifetime_parameters,
        "external policy",
        machine.name.as_str(),
    )?;
    let entry = compilation
        .machine_states(machine)
        .first()
        .ok_or_else(|| rejected("external leaf has no entry signature"))?;
    let type_identity = |reference| {
        review_signature_type_identity_with_binders(
            compilation,
            reference,
            &binders,
            &machine.lifetime_parameters,
        )
    };
    let parameters = compilation
        .state_parameters(entry)
        .iter()
        .map(|parameter| {
            Ok(PackageReviewExternalCallableParameter {
                type_identity: type_identity(parameter.type_reference)?,
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
                is_self: parameter.is_self,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let return_type = entry
        .return_type
        .is_valid()
        .then(|| type_identity(entry.return_type))
        .transpose()?;
    Ok((
        binders,
        PackagePolicyExternalCallableSignature {
            lifetime_parameter_count: machine.lifetime_parameters.len(),
            static_parameters,
            conformance_bounds,
            parameters,
            return_type,
        },
    ))
}
