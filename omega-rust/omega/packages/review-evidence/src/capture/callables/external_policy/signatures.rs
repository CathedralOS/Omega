//! Exact external signatures borrow checked source unless their static
//! telescope needs private instantiation. Requesting mutable projection storage
//! clones the whole program, so an empty telescope must remain a read-only
//! projection, just as it does for ordinary callable policy.

use super::rejected;
use crate::capture::PackageReviewInput;
use crate::capture::semantics::{
    conformances::project_conformance_bounds, types::review_signature_type_identity_with_binders,
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
        Vec<typed_trees::name::Identifier>,
        PackagePolicyExternalCallableSignature,
    ),
    Vec<Diagnostic>,
> {
    let super::super::policy_parameters::CallableParameters {
        binders,
        lifetime_parameters,
        policy_type_parameters: static_parameters,
    } = super::super::policy_parameters::prepare(
        compilation,
        machine,
        machine.name.as_str(),
        false,
    )?;
    let conformance_bounds = project_conformance_bounds(
        compilation,
        &machine.conformance_bounds,
        &binders,
        &lifetime_parameters,
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
            &lifetime_parameters,
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
    let lifetime_parameter_count = lifetime_parameters.len();
    Ok((
        binders,
        lifetime_parameters,
        PackagePolicyExternalCallableSignature {
            lifetime_parameter_count,
            static_parameters,
            conformance_bounds,
            parameters,
            return_type,
        },
    ))
}
