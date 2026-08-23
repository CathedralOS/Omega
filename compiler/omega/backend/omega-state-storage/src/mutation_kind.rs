use super::{StateMutationKind, StateMutationLowering};
use crate::StateStoragePlanningContext;
use omega_control_flow::StateKey;
use psi_checked_trees::CheckedTrees;
use psi_checked_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableMemberExpression, TableNamePath,
};
use psi_checked_trees::statement::StatementNode;
use psi_checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use psi_symbols::SymbolHandle;

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
    program: &CheckedTrees,
    context: &StateStoragePlanningContext,
    source_key: StateKey,
    expressions: &ExpressionTable,
    target: ExpressionHandle,
) -> StateMutationKind {
    let Some(place) = place_symbols(expressions, target) else {
        return StateMutationKind::Unknown;
    };

    if let Some(kind) = context
        .borrow_root_kind_by_symbol(source_key, place.head_symbol)
        .or_else(|| context.borrow_root_kind_by_symbol(source_key, place.symbol))
    {
        if kind == omega_control_flow::StateBorrowRootKind::LocalData
            && place.symbol != place.head_symbol
            && local_symbol_is_mutable_reference(program, source_key, place.head_symbol)
        {
            return StateMutationKind::ParameterOrAlias;
        }
        return mutation_kind_for_borrow_root(kind);
    }

    if place.head_symbol == source_key.machine {
        return StateMutationKind::MachineOwned;
    }

    StateMutationKind::Unknown
}

fn local_symbol_is_mutable_reference(
    program: &CheckedTrees,
    source_key: StateKey,
    symbol: SymbolHandle,
) -> bool {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == source_key.machine)
    else {
        return false;
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == source_key.state)
    else {
        return false;
    };

    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| match statement {
            StatementNode::LocalData(local_data) if local_data.symbol == symbol => {
                is_mutable_reference_type(program, local_data.type_reference)
            }
            _ => false,
        })
}

fn is_mutable_reference_type(program: &CheckedTrees, type_reference: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { access, .. } => access.is_exclusive(),
        TypeReferenceNode::Constrained { base_type, .. } => {
            is_mutable_reference_type(program, *base_type)
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => false,
    }
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

fn member_symbols(table: &ExpressionTable, member: &TableMemberExpression) -> Option<PlaceSymbols> {
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
