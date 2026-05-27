use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableRangeExpression,
};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

use super::facts::RangeFacts;

pub(super) fn provable_range_bounds(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    range: &TableRangeExpression,
) -> Option<(i64, Option<i64>)> {
    let start = if range.start.is_valid() {
        expression_integer_value(program, facts, range.start)?
    } else {
        0
    };
    let end = if range.end.is_valid() {
        Some(expression_integer_value(program, facts, range.end)?)
    } else {
        None
    };
    Some((start, end))
}

pub(super) fn expression_integer_value(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
) -> Option<i64> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let left = expression_integer_value(program, facts, binary.left)?;
            let right = expression_integer_value(program, facts, binary.right)?;
            folded_integer_binary(left, binary.operator, right)
        }
        ExpressionNode::Integer(value) => Some(*value),
        ExpressionNode::Name(_) => {
            let (symbol, name) = expression_name(program, expression)?;
            facts.local_integer(symbol, name)
        }
        _ => None,
    }
}

pub(super) fn expression_name(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<(SymbolHandle, Option<&str>)> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    Some((
        path.symbol,
        program
            .expression_table
            .name_path_members(path.members)
            .last()
            .map(|name| name.as_str()),
    ))
}

pub(super) fn expression_indexable_length(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
) -> Option<usize> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call)
            if matches!(call.target.as_str(), "as_slice" | "as_mut_slice") =>
        {
            fixed_array_expression_length(program, facts, call.receiver)
        }
        ExpressionNode::Indexed(indexed) => {
            let length = expression_indexable_length(program, facts, indexed.collection)?;
            range_result_length(program, facts, indexed.index, length)
        }
        ExpressionNode::Member(member) => {
            facts.field_length(member.member_symbol, Some(member.member.as_str()))
        }
        ExpressionNode::Name(path) => facts.local_length(
            path.symbol,
            program
                .expression_table
                .name_path_members(path.members)
                .last()
                .map(|name| name.as_str()),
        ),
        _ => None,
    }
}

pub(super) fn expression_is_slice(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    expression_type_reference(program, machine, state, expression)
        .is_some_and(|type_reference| type_reference_is_slice(program, type_reference))
}

fn folded_integer_binary(left: i64, operator: BinaryOperator, right: i64) -> Option<i64> {
    match operator {
        BinaryOperator::Add => left.checked_add(right),
        BinaryOperator::Divide => (right != 0).then(|| left.checked_div(right)).flatten(),
        BinaryOperator::Modulo => (right != 0).then(|| left.checked_rem(right)).flatten(),
        BinaryOperator::Multiply => left.checked_mul(right),
        BinaryOperator::Subtract => left.checked_sub(right),
        BinaryOperator::And
        | BinaryOperator::Equal
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => None,
    }
}

fn range_result_length(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    index: ExpressionHandle,
    length: usize,
) -> Option<usize> {
    let ExpressionNode::Range(range) = program.expression_table.expression(index) else {
        return None;
    };
    let (start, end) = provable_range_bounds(program, facts, range)?;
    let start = usize::try_from(start).ok()?;
    let end = end.map(usize::try_from).transpose().ok()?.unwrap_or(length);
    if start > end || end > length {
        return None;
    }
    Some(end.saturating_sub(start))
}

fn fixed_array_expression_length(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
) -> Option<usize> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            facts.field_length(member.member_symbol, Some(member.member.as_str()))
        }
        ExpressionNode::Name(path) => facts.local_length(
            path.symbol,
            program
                .expression_table
                .name_path_members(path.members)
                .last()
                .map(|name| name.as_str()),
        ),
        _ => None,
    }
}

fn expression_type_reference(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            expression_type_reference(program, machine, state, *inner)
        }
        ExpressionNode::Name(path) => {
            type_reference_for_symbol(program, machine, state, path.symbol).or_else(|| {
                let name = program
                    .expression_table
                    .name_path_members(path.members)
                    .last()?;
                type_reference_for_name(program, machine, state, name)
            })
        }
        ExpressionNode::Member(member) => expression_type_reference(
            program,
            machine,
            state,
            member.receiver,
        )
        .and_then(|receiver_type| {
            data_field_type_reference(program, receiver_type, member.member_symbol, &member.member)
        }),
        _ => None,
    }
}

fn type_reference_for_symbol(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !symbol.is_valid() {
        return None;
    }

    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| {
                    let omega_typed_trees::statement::StatementNode::LocalData(local) = statement
                    else {
                        return None;
                    };
                    (local.symbol == symbol).then_some(local.type_reference)
                })
        })
        .or_else(|| {
            program
                .machine_owned_data(machine)
                .iter()
                .find(|owned| owned.symbol == symbol)
                .map(|owned| owned.type_reference)
        })
}

fn type_reference_for_name(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    name: &omega_typed_trees::name::Identifier,
) -> Option<TypeReferenceHandle> {
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name == *name)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| {
                    let omega_typed_trees::statement::StatementNode::LocalData(local) = statement
                    else {
                        return None;
                    };
                    (local.name == *name).then_some(local.type_reference)
                })
        })
        .or_else(|| {
            program
                .machine_owned_data(machine)
                .iter()
                .find(|owned| owned.name == *name)
                .map(|owned| owned.type_reference)
        })
}

fn type_reference_is_slice(
    program: &omega_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => type_reference_is_slice(program, *referee),
        TypeReferenceNode::Slice { .. } => true,
        TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => false,
    }
}

fn data_field_type_reference(
    program: &omega_typed_trees::TypedTrees,
    type_reference: TypeReferenceHandle,
    member_symbol: SymbolHandle,
    member_name: &omega_typed_trees::name::Identifier,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => data_field_type_reference(program, *referee, member_symbol, member_name),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        } => data_definition_by_symbol_or_name(program, *base_symbol, base_name).and_then(
            |data_definition| {
                data_field_in_definition(program, data_definition, member_symbol, member_name)
            },
        ),
        TypeReferenceNode::Named { symbol, name } => {
            data_definition_by_symbol_or_name(program, *symbol, name).and_then(|data_definition| {
                data_field_in_definition(program, data_definition, member_symbol, member_name)
            })
        }
        TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => None,
    }
}

fn data_definition_by_symbol_or_name<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    symbol: SymbolHandle,
    name: &omega_typed_trees::name::Identifier,
) -> Option<&'program omega_typed_trees::data::DataDefinition> {
    program.data_definitions().iter().find(|data_definition| {
        (symbol.is_valid() && data_definition.symbol == symbol) || data_definition.name == *name
    })
}

fn data_field_in_definition(
    program: &omega_typed_trees::TypedTrees,
    data_definition: &omega_typed_trees::data::DataDefinition,
    member_symbol: SymbolHandle,
    member_name: &omega_typed_trees::name::Identifier,
) -> Option<TypeReferenceHandle> {
    program
        .data_members(data_definition)
        .iter()
        .find_map(|member| {
            let omega_typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };

            ((member_symbol.is_valid() && field.symbol == member_symbol)
                || field.name == *member_name)
                .then_some(field.type_reference)
        })
}
