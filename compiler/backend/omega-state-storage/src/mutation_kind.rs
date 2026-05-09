use super::{StateMutationKind, StateMutationLowering};
use crate::StateStoragePlanningContext;
use omega_control_flow::StateKey;
use omega_core::symbols::SymbolHandle;
use omega_typed_program::expression::{Expression, NamePath};
use omega_typed_program::machine::Machine;
use omega_typed_program::statement::Statement;

pub(super) fn mutation_lowering(
    context: &StateStoragePlanningContext,
    source_key: StateKey,
    statement_index: usize,
    mutation_kind: StateMutationKind,
) -> StateMutationLowering {
    if context.state_mutation_is_already_lowered_by_key(source_key, statement_index) {
        return StateMutationLowering::AlreadyLowered;
    }

    match mutation_kind {
        StateMutationKind::Local => StateMutationLowering::NeedsLocalWrite,
        StateMutationKind::MachineOwned => StateMutationLowering::NeedsMachineOwnedWrite,
        StateMutationKind::ParameterOrAlias => StateMutationLowering::NeedsAliasWrite,
        StateMutationKind::Unknown => StateMutationLowering::Unknown,
    }
}

pub(super) fn mutation_kind(
    machine: &Machine,
    state: &omega_typed_program::state::State,
    target: &Expression,
) -> StateMutationKind {
    let Some(place) = place_symbols(target) else {
        return StateMutationKind::Unknown;
    };

    if machine
        .owned_data
        .iter()
        .any(|owned_data| owned_data.symbol == place.symbol)
    {
        return StateMutationKind::MachineOwned;
    }

    if state
        .parameters
        .iter()
        .any(|parameter| parameter.symbol == place.head_symbol)
    {
        return StateMutationKind::ParameterOrAlias;
    }

    if state.statements.iter().any(|statement| {
        matches!(statement, Statement::LocalData(local_data) if local_data.symbol == place.head_symbol)
    }) {
        return StateMutationKind::Local;
    }

    StateMutationKind::Unknown
}

#[derive(Debug, Clone, Copy)]
struct PlaceSymbols {
    head_symbol: SymbolHandle,
    symbol: SymbolHandle,
}

fn place_symbols(expression: &Expression) -> Option<PlaceSymbols> {
    match expression {
        Expression::Name(path) => name_path_symbols(path),
        Expression::Indexed(indexed) => place_symbols(&indexed.collection),
        Expression::Mutable(expression) => place_symbols(expression),
        _ => None,
    }
}

fn name_path_symbols(path: &NamePath) -> Option<PlaceSymbols> {
    let head_symbol = path.head_symbol();
    if !head_symbol.is_valid() {
        return None;
    }

    Some(PlaceSymbols {
        head_symbol,
        symbol: path.symbol(),
    })
}
