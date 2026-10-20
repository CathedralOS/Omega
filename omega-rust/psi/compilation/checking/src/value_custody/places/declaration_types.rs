//! Parent links select the current declaration's type, never its template's.

use symbols::{SymbolHandle, SymbolKind};

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
    let data_owner = if owner.kind == SymbolKind::Variant {
        owner.parent
    } else {
        parent
    };
    for data in program
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == data_owner)
    {
        for member in program.data_members(data) {
            match member {
                typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                    return Some(field.type_reference);
                }
                typed_trees::data::DataMember::Variant(variant) if variant.symbol == parent => {
                    if let Some(type_reference) = program
                        .data_payload_fields(variant)
                        .iter()
                        .find_map(|field| (field.symbol == symbol).then_some(field.type_reference))
                    {
                        return Some(type_reference);
                    }
                }
                _ => {}
            }
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
    for definition in program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == owner.parent)
    {
        for signature in program
            .trait_machine_signatures(definition)
            .iter()
            .filter(|signature| signature.symbol == parent)
        {
            if let Some(type_reference) = program
                .state_signature_parameters(signature)
                .iter()
                .find_map(|parameter| {
                    (parameter.symbol == symbol).then_some(parameter.type_reference)
                })
            {
                return Some(type_reference);
            }
        }
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
