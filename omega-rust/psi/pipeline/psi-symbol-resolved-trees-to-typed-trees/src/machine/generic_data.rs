use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::{SymbolResolvedTrees, machine::Machine, types::TypeReference};

#[cfg(test)]
mod tests;

/// Authorize suppression only from the producer's exact template/closed-owner
/// association. Public visibility and operational meaning remain unchanged.
pub(super) fn is_derived(
    program: &SymbolResolvedTrees,
    machine: &Machine,
) -> Result<bool, Diagnostic> {
    let origin = &machine.generic_data_origin;
    if !origin.template_source.is_source_backed()
        && !origin.template.is_valid()
        && !origin.closed_owner.is_valid()
    {
        return Ok(false);
    }
    let template = program
        .machines
        .iter()
        .find(|candidate| candidate.symbol == origin.template);
    let owner = program
        .data_definitions
        .iter()
        .find(|candidate| candidate.symbol == origin.closed_owner);
    let valid = origin.template_source.is_source_backed()
        && origin.template.is_valid()
        && origin.closed_owner.is_valid()
        && machine.attached_data_symbol == origin.closed_owner
        && template.zip(owner).is_some_and(|(template, owner)| {
            template.symbol != machine.symbol
                && !template.generic_data_origin.template.is_valid()
                && template.name.source_span() == origin.template_source.source_span()
                && machine.name.source_span() == template.name.source_span()
                && machine.is_public == template.is_public
                && machine.supply_mode == template.supply_mode
                && machine.type_parameters.is_empty()
                && matches!(&owner.generic_instance, Some(TypeReference::Generic(application))
                    if application.base_symbol == template.attached_data_symbol)
        });
    if valid {
        Ok(true)
    } else {
        Err(Diagnostic::error(
            "generic-data method lost its exact template and closed-owner derivation",
        )
        .with_source_span(machine.name.source_span()))
    }
}
