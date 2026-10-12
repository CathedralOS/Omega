//! Join an original machine operand to its admitted occurrence selection.
//!
//! Retained tuple admission runs before call formation. Both consume this
//! query so an incomplete nested application cannot fall through to universal
//! declaration refinement merely because a provider declined to close it.
//! Declared evidence participates even when the provider body never uses it.

use diagnostics::Diagnostic;
use symbols::{JoinedPath, SymbolHandle};
use typed_trees::data::TypeParameter;
use typed_trees::expression::StaticMachineArgument;
use typed_trees::static_applications::StaticMachineApplicationHandle;

pub(super) fn resolve(
    program: &dyn crate::CheckingInput,
    application: StaticMachineApplicationHandle,
    ordinal: usize,
    authored: &StaticMachineArgument,
    parameter: &TypeParameter,
) -> Result<SymbolHandle, Diagnostic> {
    let selected = if application.is_valid() {
        let selections = program
            .expression_table
            .static_machine_application_selected_machines(application);
        if selections.is_empty() {
            authored
        } else {
            let arguments = program
                .expression_table
                .static_machine_application_arguments(application);
            if selections.len() != super::machine_selection_arguments(program, arguments).count() {
                return Err(Diagnostic::error(
                    "static callable tuple lost its exact selected machine roster",
                ));
            }
            let Some(selected) = selections.get(ordinal) else {
                return Err(Diagnostic::error(
                    "static callable tuple lost its exact selected machine ordinal",
                ));
            };
            selected
        }
    } else {
        authored
    };
    let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind else {
        return Err(Diagnostic::error(
            "static callable selection lost its machine demand",
        ));
    };
    let required = super::machine_parameter_signature(program, contract);
    if (authored.application.is_some() || selected.application.is_some())
        && program
            .applied_machine_callable(
                application,
                ordinal,
                selected.symbol,
                parameter.symbol,
                required,
            )
            .is_none()
    {
        return Err(Diagnostic::error(format!(
            "static machine argument `{}` for `{}` is a nested machine application without a completely admitted callable tuple",
            authored.path.joined_path(),
            parameter.name,
        )));
    }
    Ok(selected.symbol)
}
