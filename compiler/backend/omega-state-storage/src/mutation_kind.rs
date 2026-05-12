use super::{StateMutationKind, StateMutationLowering};
use crate::StateStoragePlanningContext;
use omega_control_flow::StateKey;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableMemberExpression, TableNamePath,
};

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
    context: &StateStoragePlanningContext,
    source_key: StateKey,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
) -> StateMutationKind {
    let state_flow = context.state_flow_by_key(source_key);

    let Some(place) = place_symbols(expressions, target) else {
        if let Some(root_name) = root_place_name(expressions, target) {
            if let Some(kind) = state_borrow_root_kind_by_name(context, state_flow, root_name) {
                return mutation_kind_for_borrow_root(kind);
            }
        }

        return StateMutationKind::Unknown;
    };

    if let Some(kind) = state_borrow_root_kind_by_symbol(context, state_flow, place.head_symbol)
        .or_else(|| state_borrow_root_kind_by_symbol(context, state_flow, place.symbol))
    {
        return mutation_kind_for_borrow_root(kind);
    }

    StateMutationKind::Unknown
}

fn state_borrow_root_kind_by_name<'a>(
    context: &'a StateStoragePlanningContext,
    state_flow: Option<&'a omega_control_flow::StateFlow>,
    root_name: &str,
) -> Option<omega_control_flow::StateBorrowRootKind> {
    let state_flow = state_flow?;
    context
        .control_flow
        .borrow_writable_roots
        .span(state_flow.borrow.writable_roots)?
        .iter()
        .find(|root| root.name.as_str() == root_name)
        .map(|root| root.kind.clone())
}

fn state_borrow_root_kind_by_symbol<'a>(
    context: &'a StateStoragePlanningContext,
    state_flow: Option<&'a omega_control_flow::StateFlow>,
    symbol: SymbolHandle,
) -> Option<omega_control_flow::StateBorrowRootKind> {
    if !symbol.is_valid() {
        return None;
    }

    let state_flow = state_flow?;
    context
        .control_flow
        .borrow_writable_roots
        .span(state_flow.borrow.writable_roots)?
        .iter()
        .find(|root| root.symbol == symbol)
        .map(|root| root.kind.clone())
}

fn mutation_kind_for_borrow_root(
    kind: omega_control_flow::StateBorrowRootKind,
) -> StateMutationKind {
    match kind {
        omega_control_flow::StateBorrowRootKind::OwnedData => StateMutationKind::MachineOwned,
        omega_control_flow::StateBorrowRootKind::LocalData => StateMutationKind::Local,
        omega_control_flow::StateBorrowRootKind::MutableParameter => {
            StateMutationKind::ParameterOrAlias
        }
    }
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

fn root_place_name(table: &ExpressionTable, expression: ExpressionHandle) -> Option<&str> {
    match table.expression(expression) {
        ExpressionNode::Name(path) => table
            .name_path_members(path.members)
            .first()
            .map(|name| name.as_str()),
        ExpressionNode::Member(member) => root_place_name(table, member.receiver),
        ExpressionNode::Indexed(indexed) => root_place_name(table, indexed.collection),
        ExpressionNode::Mutable(expression) => root_place_name(table, *expression),
        _ => None,
    }
}
