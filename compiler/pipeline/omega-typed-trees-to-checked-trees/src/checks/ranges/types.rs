use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn expression_is_slice(
    program: &omega_typed_trees::TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    expression_type_reference(program, machine, state, expression)
        .is_some_and(|type_reference| type_reference_is_slice(program, type_reference))
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
