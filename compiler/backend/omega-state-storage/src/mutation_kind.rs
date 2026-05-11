use super::{StateMutationKind, StateMutationLowering};
use crate::StateStoragePlanningContext;
use omega_control_flow::StateKey;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableMemberExpression, TableNamePath,
};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::statement::StatementNode;

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
    state: &omega_typed_trees::state::State,
    statements: &[StatementNode],
    expressions: &ExpressionTable,
    target: ExpressionHandle,
) -> StateMutationKind {
    let Some(place) = place_symbols(expressions, target) else {
        return StateMutationKind::Unknown;
    };

    if state
        .parameters
        .iter()
        .any(|parameter| {
            parameter.is_self
                && (parameter.symbol == place.head_symbol || machine.symbol == place.head_symbol)
        })
    {
        return StateMutationKind::MachineOwned;
    }

    if state
        .parameters
        .iter()
        .any(|parameter| !parameter.is_self && parameter.symbol == place.head_symbol)
    {
        return StateMutationKind::ParameterOrAlias;
    }

    if statements.iter().any(|statement| {
        matches!(statement, StatementNode::LocalData(local_data) if local_data.symbol == place.head_symbol)
    }) {
        return StateMutationKind::Local;
    }

    if machine
        .owned_data
        .iter()
        .any(|owned_data| owned_data.symbol == place.head_symbol || owned_data.symbol == place.symbol)
    {
        return StateMutationKind::MachineOwned;
    }

    StateMutationKind::Unknown
}

#[derive(Debug, Clone, Copy)]
struct PlaceSymbols {
    head_symbol: SymbolHandle,
    symbol: SymbolHandle,
}

fn place_symbols(table: &ExpressionTable, expression: ExpressionHandle) -> Option<PlaceSymbols> {
    match table.expression(expression) {
        ExpressionNode::Name(path) => name_path_symbols(path),
        ExpressionNode::Member(member) => member_symbols(table, member),
        ExpressionNode::Indexed(indexed) => place_symbols(table, indexed.collection),
        ExpressionNode::Mutable(expression) => place_symbols(table, *expression),
        _ => None,
    }
}

fn member_symbols(
    table: &ExpressionTable,
    member: &TableMemberExpression,
) -> Option<PlaceSymbols> {
    let mut place = place_symbols(table, member.receiver)?;
    if member.member_symbol.is_valid() {
        place.symbol = member.member_symbol;
    }
    Some(place)
}

fn name_path_symbols(path: &TableNamePath) -> Option<PlaceSymbols> {
    let head_symbol = path.head_symbol;
    if !head_symbol.is_valid() {
        return None;
    }

    Some(PlaceSymbols {
        head_symbol,
        symbol: path.symbol,
    })
}
