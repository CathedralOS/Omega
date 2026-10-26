//! Parent links select the current declaration's type, never its template's.

use symbols::{SymbolHandle, SymbolKind};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Exact static assumptions of this declaration and its lexical ancestors.
/// Neither runtime inputs nor a template's current callers supply equations.
pub(crate) fn declaration_type_equations(
    program: &dyn crate::CheckingInput,
    mut owner: SymbolHandle,
) -> Vec<(SymbolHandle, TypeReferenceHandle)> {
    let mut substitutions = Vec::new();
    let mut visited = Vec::new();
    while owner.is_valid() && !visited.contains(&owner) {
        visited.push(owner);
        if let Some(machine) = program.machine_by_symbol(owner) {
            for equation in program
                .declared_type_equations
                .span_or_empty(machine.declared_type_equations)
            {
                let TypeReferenceNode::Named { symbol, .. } = program
                    .type_reference_table
                    .type_reference(equation.parameter)
                else {
                    continue;
                };
                if symbol.is_valid()
                    && program.symbols.get(*symbol).kind == SymbolKind::TypeParameter
                {
                    substitutions.push((*symbol, equation.definition));
                }
            }
        }
        owner = program.declaration_scope_parent(owner);
    }
    substitutions
}

pub fn declaration_type_reference(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let declaration = program.symbols.get(symbol);
    if declaration.kind == SymbolKind::Unknown {
        return None;
    }
    let parent = declaration.parent;
    let owner = program.symbols.get(parent);
    if declaration.kind == SymbolKind::Parameter && owner.kind == SymbolKind::Domain {
        let domain = typed_trees::domain::domain_by_symbol(program, parent)?;
        return (domain.subject_symbol == symbol).then_some(domain.target_type);
    }
    if declaration.kind == SymbolKind::Const {
        return program.const_declarations().iter().find_map(|declaration| {
            (declaration.symbol == symbol).then_some(declaration.declared_type)
        });
    }
    // Parent links select the declaration's current owner. generated_from is
    // provenance, not a fallback to a template's potentially different type.
    // The declaring row and both member kinds come from the representation's
    // indexed tables, whose first-match order the roster walk repeated.
    let data_owner = if owner.kind == SymbolKind::Variant {
        owner.parent
    } else {
        parent
    };
    if let Some(data) = program.data_definition_by_symbol(data_owner) {
        if let Some(field) = program.data_field(data, symbol) {
            return Some(field.type_reference);
        }
        if owner.kind == SymbolKind::Variant
            && let Some(variant) = program.data_variant(data, parent)
            && let Some(field) = program.data_payload_field_by_symbol(variant, symbol)
        {
            return Some(field.type_reference);
        }
    }
    let machine_owner = if owner.kind == SymbolKind::State {
        owner.parent
    } else {
        parent
    };
    if let Some(machine) = program.machine_by_symbol(machine_owner) {
        if let Some(type_reference) =
            program
                .machine_type_parameters(machine)
                .iter()
                .find_map(|parameter| {
                    if parameter.symbol != symbol {
                        return None;
                    }
                    match parameter.kind {
                        typed_trees::data::TypeParameterKind::Value { type_reference }
                        | typed_trees::data::TypeParameterKind::Const { type_reference } => {
                            Some(type_reference)
                        }
                        _ => None,
                    }
                })
        {
            return Some(type_reference);
        }
        if let Some(type_reference) = program
            .machine_owned_data(machine)
            .iter()
            .find_map(|owned| (owned.symbol == symbol).then_some(owned.type_reference))
        {
            return Some(type_reference);
        }
        for state in program
            .machine_states(machine)
            .iter()
            .filter(|state| state.symbol == parent)
        {
            if let Some(type_reference) =
                program
                    .state_parameters(state)
                    .iter()
                    .find_map(|parameter| {
                        (parameter.symbol == symbol).then_some(parameter.type_reference)
                    })
            {
                return Some(type_reference);
            }
            if let Some((_, local)) = program.state_local_declaration(state, symbol) {
                return Some(local.type_reference);
            }
        }
    }
    if let Some(parameters) = crate::call_target_parameters(program, parent)
        && let Some(parameter) = parameters
            .iter()
            .find(|parameter| parameter.symbol == symbol)
    {
        return Some(parameter.type_reference);
    }
    for proposition in program
        .propositions()
        .iter()
        .filter(|proposition| proposition.symbol == parent)
    {
        if let Some(type_reference) = program
            .proposition_parameters(proposition)
            .iter()
            .find_map(|parameter| (parameter.symbol == symbol).then_some(parameter.type_reference))
        {
            return Some(type_reference);
        }
    }
    None
}
