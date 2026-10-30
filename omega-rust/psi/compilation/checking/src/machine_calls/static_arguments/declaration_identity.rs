//! Declaration-identity operands name exact static declarations. Their role
//! carries no callable signature or permission to execute the selected name.

use diagnostics::Diagnostic;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn validate_declaration_identity_argument(
    program: &dyn crate::CheckingInput,
    parameter: &TypeParameter,
    argument: TypeReferenceHandle,
    application_label: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(argument)
    else {
        diagnostics.push(Diagnostic::error(format!(
                "{application_label} declaration-identity argument `{}` must be one exact machine or trait-requirement name",
                parameter.name,
            )));
        return;
    };

    if symbol.is_valid()
        && program.symbols.get(*symbol).kind == symbols::SymbolKind::MachineParameter
        && program.data_type_parameters.iter().any(|(_, candidate)| {
            candidate.symbol == *symbol
                && matches!(
                    candidate.kind,
                    TypeParameterKind::Machine {
                        contract: typed_trees::data::MachineParameterContract::RequirementIdentity
                    }
                )
        })
    {
        return;
    }

    if !symbol.is_valid() || program.symbols.get(*symbol).kind != symbols::SymbolKind::State {
        diagnostics.push(Diagnostic::error(format!(
                "{application_label} argument `{name}` for declaration-identity parameter `{}` is not an exact machine or trait requirement",
                parameter.name,
            )));
        return;
    }

    // Trait requirement paths are signature-free. An overload addition
    // therefore invalidates every such identity use rather than allowing
    // an expected call shape or a visible satisfier to choose one row.
    if let Some((declaring_trait, selected_requirement)) = program.trait_machine_signature(*symbol)
    {
        let same_name_count = program
            .trait_machine_signatures(declaring_trait)
            .iter()
            .filter(|requirement| requirement.name == selected_requirement.name)
            .count();
        if same_name_count != 1 {
            diagnostics.push(Diagnostic::error(format!(
                    "{application_label} declaration-identity argument `{name}` does not resolve to one exact trait requirement; signature-free references reject overloads",
                )));
        }
    }
}
