//! Exact external signatures borrow checked source unless their static
//! telescope needs private instantiation. Requesting mutable projection storage
//! clones the whole program, so an empty telescope must remain a read-only
//! projection, just as it does for ordinary callable policy.

use super::rejected;
use crate::capture::PackageReviewInput;
use crate::capture::calling::application::signature::instantiate_static_parameters;
use crate::capture::semantics::signatures::callable_declaration_scope;
use crate::capture::semantics::{
    conformances::project_conformance_bounds, signatures::policy::project_type_parameters,
    types::review_signature_type_identity_with_binders,
};
use crate::record::{
    PackagePolicyExternalCallableSignature, PackageReviewExternalCallableParameter,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::machine::Machine;

pub(super) fn project(
    compilation: &PackageReviewInput<'_>,
    machine: &Machine,
) -> Result<
    (
        Vec<(SymbolHandle, String)>,
        PackagePolicyExternalCallableSignature,
    ),
    Vec<Diagnostic>,
> {
    let scope = callable_declaration_scope(compilation, machine)?;
    let mut prepared = validation::CheckingProjection::new(scope.source.checking_input);
    let source_parameters = scope.type_parameters;
    let mut parameters = source_parameters.to_vec();
    let lifetimes = scope
        .lifetime_parameters
        .iter()
        .cloned()
        .map(|name| (name.clone(), name))
        .collect::<Vec<_>>();
    let mut scopes = Vec::new();
    if !parameters.is_empty() {
        instantiate_static_parameters(
            prepared.program_mut(),
            &mut parameters,
            &[],
            &lifetimes,
            scope.lifetime_parameters,
            &mut scopes,
            0,
        )?;
    }
    let exposure = crate::capture::contracts::facts::callable_contract_exposure(machine);
    let (binders, static_parameters) = project_type_parameters(
        &prepared,
        &scope.source,
        &parameters,
        source_parameters,
        machine.name.as_str(),
        &[],
        0,
        scope.lifetime_parameters,
        &[],
        &scopes,
        false,
        exposure,
    )?;
    let conformance_bounds = project_conformance_bounds(
        compilation,
        &machine.conformance_bounds,
        &binders,
        scope.lifetime_parameters,
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
            scope.lifetime_parameters,
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
            lifetime_parameter_count: scope.lifetime_parameters.len(),
            static_parameters,
            conformance_bounds,
            parameters,
            return_type,
        },
    ))
}
