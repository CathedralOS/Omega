use super::application::project_selected_conformance_application;
use crate::capture::semantics::declarations::nominal_identity;
use crate::capture::semantics::types::review_signature_type_identity_with_binders;
use crate::record::PackageReviewConformanceBound;
use omega_compiler::CheckedCompilation;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

pub(crate) fn project_conformance_bounds(
    compilation: &CheckedCompilation,
    bounds: &[psi_typed_trees::machine::GenericConformanceBound],
    parameters: &[psi_typed_trees::data::TypeParameter],
    binders: &[(SymbolHandle, String)],
    lifetime_binders: &[psi_typed_trees::name::Identifier],
    declaration_kind: &str,
    declaration_path: &str,
) -> Result<Vec<PackageReviewConformanceBound>, Vec<Diagnostic>> {
    let mut projected = Vec::with_capacity(bounds.len());
    let mut next_binder_ordinal = 0usize;
    for bound in bounds {
        let binder_ordinal = if let Some(binder) = bound.binder {
            if !binder.is_valid() {
                return Err(vec![Diagnostic::error(format!(
                    "{declaration_kind} `{declaration_path}` has an unresolved conformance evidence binder"
                ))]);
            }
            let ordinal = u32::try_from(next_binder_ordinal).map_err(|_| {
                vec![Diagnostic::error(format!(
                    "{declaration_kind} `{declaration_path}` has too many conformance binders for portable review evidence"
                ))]
            })?;
            next_binder_ordinal += 1;
            Some(ordinal)
        } else {
            None
        };
        let Some(subject_parameter) = parameters
            .iter()
            .position(|parameter| parameter.symbol == bound.subject)
        else {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` has a conformance subject outside its type-parameter telescope"
            ))]);
        };
        let (
            selected_conformance,
            selected_lifetime_arguments,
            selected_arguments,
            selected_subject,
            trait_symbol,
            trait_lifetime_arguments,
            trait_arguments,
        ) = match bound.selected_conformance.as_ref() {
            None => (
                None,
                Vec::new(),
                Vec::new(),
                None,
                bound.carrier,
                Vec::new(),
                bound
                    .arguments
                    .iter()
                    .map(|argument| {
                        review_signature_type_identity_with_binders(
                            compilation,
                            *argument,
                            binders,
                            lifetime_binders,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Some(selected) => {
                let selected = project_selected_conformance_application(
                    compilation,
                    selected,
                    binders,
                    lifetime_binders,
                    declaration_kind,
                    declaration_path,
                )?;
                (
                    Some(selected.declaration),
                    selected.lifetime_arguments,
                    selected.arguments,
                    Some(selected.subject),
                    selected.trait_symbol,
                    selected.trait_lifetime_arguments,
                    selected.trait_arguments,
                )
            }
        };
        let matching_traits = compilation
            .traits()
            .iter()
            .filter(|definition| definition.symbol == trait_symbol)
            .collect::<Vec<_>>();
        let [trait_definition] = matching_traits.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` conformance bound resolves to {} traits; expected exactly one",
                matching_traits.len()
            ))]);
        };
        if !trait_definition.is_public {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` exposes non-public conformance trait `{}`",
                trait_definition.name
            ))]);
        }
        if trait_lifetime_arguments.len() != trait_definition.lifetime_parameters.len() {
            return Err(vec![Diagnostic::error(format!(
                "{declaration_kind} `{declaration_path}` uses conformance trait `{}` with {} target lifetime argument(s), expected {}",
                trait_definition.name,
                trait_lifetime_arguments.len(),
                trait_definition.lifetime_parameters.len(),
            ))]);
        }
        projected.push(PackageReviewConformanceBound {
            binder_ordinal,
            subject_parameter: u32::try_from(subject_parameter).map_err(|_| {
                vec![Diagnostic::error(format!(
                    "{declaration_kind} `{declaration_path}` conformance subject exceeds the portable review parameter range"
                ))]
            })?,
            selected_conformance,
            selected_lifetime_arguments,
            selected_arguments,
            selected_subject,
            trait_identity: nominal_identity(compilation, trait_definition.symbol)?,
            trait_lifetime_arguments,
            arguments: trait_arguments,
        });
    }
    Ok(projected)
}
