use super::super::semantics::types::{
    project_data_properties, review_signature_type_identity_with_binders,
};
use crate::capture::semantics::conformances::project_conformance_bounds;
use crate::record::{
    PackageReviewExternalCallableParameter, PackageReviewExternalCallableSignature,
    PackageReviewExternalStaticParameter,
};
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;

pub(super) fn project_external_callable_signature(
    compilation: &CheckedCompilation,
    machine: &psi_typed_trees::machine::Machine,
    binders: &[(psi_symbols::SymbolHandle, String)],
) -> Result<PackageReviewExternalCallableSignature, Vec<Diagnostic>> {
    let subject = machine.name.as_str();
    let type_parameters = compilation.machine_type_parameters(machine);
    let static_parameters = type_parameters
        .iter()
        .map(|parameter| match &parameter.kind {
            psi_typed_trees::data::TypeParameterKind::Type => {
                Ok(PackageReviewExternalStaticParameter::Type {
                    properties: project_data_properties(parameter.bounds),
                })
            }
            psi_typed_trees::data::TypeParameterKind::Const { type_reference } => {
                if parameter.bounds != psi_typed_trees::data::DataProperties::default() {
                    return Err(vec![Diagnostic::error(format!(
                        "reviewed callable `{subject}` gives a const parameter inapplicable type-property bounds"
                    ))]);
                }
                Ok(PackageReviewExternalStaticParameter::Const {
                    type_identity: review_signature_type_identity_with_binders(
                        compilation,
                        *type_reference,
                        binders,
                        &machine.lifetime_parameters,
                    )?,
                })
            }
            psi_typed_trees::data::TypeParameterKind::Machine { .. }
            | psi_typed_trees::data::TypeParameterKind::Proposition { .. } => {
                Err(vec![Diagnostic::error(format!(
                    "reviewed callable `{subject}` uses a static parameter kind not yet represented by its executable-supply signature"
                ))])
            }
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    let conformance_bounds = project_conformance_bounds(
        compilation,
        &machine.conformance_bounds,
        type_parameters,
        binders,
        &machine.lifetime_parameters,
        "reviewed external callable",
        subject,
    )?;
    let Some(entry) = compilation.machine_states(machine).first() else {
        return Err(vec![Diagnostic::error(format!(
            "reviewed callable `{subject}` has no canonical entry signature"
        ))]);
    };
    let parameters = compilation
        .state_parameters(entry)
        .iter()
        .map(|parameter| {
            Ok(PackageReviewExternalCallableParameter {
                type_identity: review_signature_type_identity_with_binders(
                    compilation,
                    parameter.type_reference,
                    binders,
                    &machine.lifetime_parameters,
                )?,
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
                is_self: parameter.is_self,
            })
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    Ok(PackageReviewExternalCallableSignature {
        lifetime_parameter_count: machine.lifetime_parameters.len(),
        static_parameters,
        conformance_bounds,
        parameters,
        return_type: review_signature_type_identity_with_binders(
            compilation,
            entry.return_type,
            binders,
            &machine.lifetime_parameters,
        )?,
    })
}
