use crate::context::*;
use crate::flow::{
    effective_member_symbol, resolve_member_symbol_from_type_symbol, symbol_type_symbol,
};
use crate::lookup::first_valid_name_path_symbol;

pub(super) fn contextual_effective_member_symbol(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    receiver: ExpressionHandle,
    member: &psi_typed_trees::expression::TableMemberExpression,
    machine_symbol: SymbolHandle,
) -> SymbolHandle {
    if let Some(type_symbol) = contextual_expression_type_symbol(
        program,
        state_symbol,
        statement_index,
        receiver,
        machine_symbol,
    ) && let Some(symbol) =
        resolve_member_symbol_from_type_symbol(program, type_symbol, member.member.as_str())
    {
        return symbol;
    }

    effective_member_symbol(program, receiver, member)
}

fn contextual_expression_type_symbol(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    machine_symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => contextual_expression_type_symbol(
            program,
            state_symbol,
            statement_index,
            *inner,
            machine_symbol,
        ),
        ExpressionNode::Name(path) => {
            let symbol = contextual_name_root_symbol(
                program,
                state_symbol,
                statement_index,
                expression,
                path,
            )?;
            contextual_symbol_type_symbol(program, state_symbol, statement_index, symbol)
        }
        ExpressionNode::Indexed(indexed) => contextual_expression_type_symbol(
            program,
            state_symbol,
            statement_index,
            indexed.collection,
            machine_symbol,
        ),
        ExpressionNode::Member(member) => {
            let symbol = contextual_effective_member_symbol(
                program,
                state_symbol,
                statement_index,
                member.receiver,
                member,
                machine_symbol,
            );
            contextual_symbol_type_symbol(program, state_symbol, statement_index, symbol)
        }
        _ => None,
    }
}

fn contextual_symbol_type_symbol(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    if let Some(type_symbol) = symbol_type_symbol(program, symbol)
        && type_symbol.is_valid()
    {
        return Some(type_symbol);
    }

    let state = crate::semantic_calls::find_state(program, state_symbol)?;
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| match statement {
            StatementNode::LocalData(local_data) if local_data.symbol == symbol => {
                let type_symbol = program
                    .type_reference_table
                    .type_symbol(local_data.type_reference);
                type_symbol.is_valid().then_some(type_symbol)
            }
            _ => None,
        })
}

pub(super) fn contextual_name_root_symbol(
    program: &psi_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    path: &psi_typed_trees::expression::TableNamePath,
) -> Option<SymbolHandle> {
    if let Some(symbol) = first_valid_name_path_symbol(path, &program.expression_table) {
        return Some(symbol);
    }

    let state = crate::semantic_calls::find_state(program, state_symbol)?;
    let name = program.expression_table.display_name(expression);
    if name == "self" {
        return program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.is_self)
            .map(|parameter| parameter.symbol);
    }

    if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| !parameter.is_self && parameter.name.as_str() == name)
    {
        return Some(parameter.symbol);
    }

    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| match statement {
            StatementNode::LocalData(local_data) if local_data.name.as_str() == name => {
                Some(local_data.symbol)
            }
            _ => None,
        })
}
