use crate::context::*;
use crate::flow::{
    effective_member_symbol, resolve_member_symbol_from_type_symbol, symbol_type_symbol,
};
use crate::lookup::first_valid_name_path_symbol;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct BorrowAccessPlace {
    pub(crate) root_symbol: SymbolHandle,
    pub(crate) segments: Vec<omega_facts::PlaceSegment>,
}

pub(crate) fn borrow_access_place(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    machine_symbol: SymbolHandle,
) -> Option<BorrowAccessPlace> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            let mut place = borrow_access_place(
                program,
                state_symbol,
                statement_index,
                indexed.collection,
                machine_symbol,
            )?;
            place.segments.push(omega_facts::PlaceSegment::Index {
                expression: indexed.index,
            });
            Some(place)
        }
        ExpressionNode::Range(_) => None,
        ExpressionNode::Mutable(inner) => borrow_access_place(
            program,
            state_symbol,
            statement_index,
            *inner,
            machine_symbol,
        ),
        ExpressionNode::Member(member) => {
            match program.expression_table.expression(member.receiver) {
                ExpressionNode::Name(path)
                    if path.members.count() == 1
                        && contextual_name_root_symbol(
                            program,
                            state_symbol,
                            statement_index,
                            member.receiver,
                            path,
                        )
                        .is_some_and(|symbol| symbol == machine_symbol) =>
                {
                    let member_symbol = contextual_effective_member_symbol(
                        program,
                        state_symbol,
                        statement_index,
                        member.receiver,
                        member,
                        machine_symbol,
                    );
                    Some(BorrowAccessPlace {
                        root_symbol: member_symbol,
                        segments: Vec::new(),
                    })
                }
                _ => {
                    let mut place = borrow_access_place(
                        program,
                        state_symbol,
                        statement_index,
                        member.receiver,
                        machine_symbol,
                    )?;
                    place.segments.push(omega_facts::PlaceSegment::Field {
                        symbol: contextual_effective_member_symbol(
                            program,
                            state_symbol,
                            statement_index,
                            member.receiver,
                            member,
                            machine_symbol,
                        ),
                    });
                    Some(place)
                }
            }
        }
        ExpressionNode::Name(path) => {
            let root_symbol = contextual_name_root_symbol(
                program,
                state_symbol,
                statement_index,
                expression,
                path,
            )?;
            let member_symbols = program
                .expression_table
                .name_path_member_symbols(path.member_symbols);
            let skip = member_symbols
                .iter()
                .position(|member_symbol| *member_symbol == root_symbol)
                .map(|index| index + 1)
                .unwrap_or(1);
            Some(BorrowAccessPlace {
                root_symbol,
                segments: member_symbols
                    .iter()
                    .skip(skip)
                    .copied()
                    .map(|symbol| omega_facts::PlaceSegment::Field { symbol })
                    .collect(),
            })
        }
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_) => None,
    }
}

fn contextual_effective_member_symbol(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    receiver: ExpressionHandle,
    member: &omega_typed_trees::expression::TableMemberExpression,
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
    program: &omega_typed_trees::TypedTrees,
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
    program: &omega_typed_trees::TypedTrees,
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

fn contextual_name_root_symbol(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    path: &omega_typed_trees::expression::TableNamePath,
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
