use omega_checked_trees::CheckedValueOrigin;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(super) fn expression_type_reference_for_origin(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
) -> Option<TypeReferenceHandle> {
    let (state_symbol, statement_index) = match origin {
        CheckedValueOrigin::StateStatement {
            state_symbol,
            statement_index,
            ..
        } => (state_symbol, statement_index),
        CheckedValueOrigin::NestedExpression { .. }
        | CheckedValueOrigin::MachineDecrease { .. }
        | CheckedValueOrigin::MachineOwnedDataInitializer { .. } => return None,
    };
    expression_type_reference_in_state(program, state_symbol, statement_index, expression)
}

fn expression_type_reference_in_state(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            expression_type_reference_in_state(program, state_symbol, statement_index, *inner)
        }
        ExpressionNode::Name(path) => {
            symbol_type_reference_in_state(program, state_symbol, statement_index, path.symbol)
        }
        ExpressionNode::Member(member) => expression_type_reference_in_state(
            program,
            state_symbol,
            statement_index,
            member.receiver,
        )
        .and_then(|receiver| field_type_reference(program, receiver, member.member_symbol)),
        ExpressionNode::Indexed(indexed) => expression_type_reference_in_state(
            program,
            state_symbol,
            statement_index,
            indexed.collection,
        )
        .and_then(|collection| indexed_element_type_reference(program, collection)),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::Unary(_) => None,
    }
}

fn symbol_type_reference_in_state(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !symbol.is_valid() {
        return None;
    }

    let state = crate::semantic_calls::find_state(program, state_symbol)?;
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| local_type_reference_before_statement(program, state, statement_index, symbol))
}

fn local_type_reference_before_statement(
    program: &TypedTrees,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| {
            let omega_typed_trees::statement::StatementNode::LocalData(local) = statement else {
                return None;
            };
            (local.symbol == symbol).then_some(local.type_reference)
        })
}

fn field_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    field_symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => field_type_reference(program, *referee, field_symbol),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            ..
        }
        | TypeReferenceNode::Named {
            symbol: base_symbol,
            name: base_name,
        } => program
            .data_definitions()
            .iter()
            .find(|data| {
                (base_symbol.is_valid() && data.symbol == *base_symbol) || data.name == *base_name
            })
            .and_then(|data| data_field_type_reference(program, data, field_symbol)),
        TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => None,
    }
}

fn data_field_type_reference(
    program: &TypedTrees,
    data: &omega_typed_trees::data::DataDefinition,
    field_symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !field_symbol.is_valid() {
        return None;
    }
    program.data_members(data).iter().find_map(|member| {
        let omega_typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };
        (field.symbol == field_symbol).then_some(field.type_reference)
    })
}

fn indexed_element_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => indexed_element_type_reference(program, *referee),
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => Some(*element_type),
        TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => None,
    }
}
