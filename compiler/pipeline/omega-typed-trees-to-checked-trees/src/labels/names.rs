use omega_core::symbols::SymbolHandle;

pub(crate) fn machine_name(
    program: &omega_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
) -> String {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .map(|machine| machine.name.as_str().to_owned())
        .unwrap_or_else(|| symbol_name(program, machine_symbol))
}

pub(crate) fn call_target_label(
    program: &omega_typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
) -> String {
    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine).iter())
        .find(|state| state.symbol == target_symbol)
        .map(|state| state.name.as_str().to_owned())
        .unwrap_or_else(|| symbol_name(program, target_symbol))
}

pub(crate) fn symbol_name(
    program: &omega_typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> String {
    if !symbol.is_valid() {
        return "unknown".to_owned();
    }

    if let Some(machine) = program.machines().iter().find(|machine| machine.symbol == symbol) {
        return machine.name.as_str().to_owned();
    }

    if let Some(state) = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine).iter())
        .find(|state| state.symbol == symbol)
    {
        return state.name.as_str().to_owned();
    }

    if let Some(field) = program
        .data_definitions()
        .iter()
        .flat_map(|data| program.data_members(data).iter())
        .find_map(|member| match member {
            omega_typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                Some(field.name.as_str().to_owned())
            }
            _ => None,
        })
    {
        return field;
    }

    program.symbols.name(symbol).to_string()
}
