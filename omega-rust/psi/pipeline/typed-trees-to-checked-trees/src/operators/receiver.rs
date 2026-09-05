use checked_trees::CheckedValueOrigin;
use numerics::literals::FloatFormat;
use symbols::{BuiltinFunction, SymbolHandle};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn expression_type_reference_for_origin(
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
        ExpressionNode::Atomic(atomic) => {
            expression_type_reference_in_state(program, state_symbol, statement_index, atomic.value)
        }
        ExpressionNode::Borrow(inner) => {
            expression_type_reference_in_state(program, state_symbol, statement_index, inner.target)
        }
        ExpressionNode::Name(path) => {
            let name = program
                .expression_table
                .name_path_members(path.members)
                .first()
                .map(|name| name.as_str());
            symbol_type_reference_in_state(
                program,
                state_symbol,
                statement_index,
                path.symbol,
                name,
            )
        }
        ExpressionNode::Member(member) => expression_type_reference_in_state(
            program,
            state_symbol,
            statement_index,
            member.receiver,
        )
        .and_then(|receiver| {
            field_type_reference(program, receiver, member.member_symbol, &member.member)
        })
        .or_else(|| self_field_type_reference(program, state_symbol, member)),
        ExpressionNode::Indexed(indexed) => expression_type_reference_in_state(
            program,
            state_symbol,
            statement_index,
            indexed.collection,
        )
        .and_then(|collection| indexed_element_type_reference(program, collection)),
        ExpressionNode::Cast(cast) => Some(cast.target_type),
        ExpressionNode::Call(call) => {
            typed_trees::operator::resolve_named_expression_call(program, call)
                .map(|operator| operator.return_type)
                .or_else(|| {
                    [
                        BuiltinFunction::Min,
                        BuiltinFunction::Max,
                        BuiltinFunction::Sqrt,
                    ]
                    .into_iter()
                    .any(|function| {
                        program.symbols.builtin_function_symbol(function)
                            == Some(call.target_symbol)
                    })
                    .then(|| {
                        program
                            .expression_table
                            .expression_handles(call.arguments)
                            .iter()
                            .find_map(|argument| {
                                expression_type_reference_in_state(
                                    program,
                                    state_symbol,
                                    statement_index,
                                    *argument,
                                )
                            })
                    })
                    .flatten()
                    .or_else(|| {
                        contextual_type_reference_in_state(program, state_symbol, statement_index)
                    })
                })
        }
        ExpressionNode::Binary(binary) => {
            expression_type_reference_in_state(program, state_symbol, statement_index, binary.left)
                .or_else(|| {
                    expression_type_reference_in_state(
                        program,
                        state_symbol,
                        statement_index,
                        binary.right,
                    )
                })
                .or_else(|| {
                    contextual_type_reference_in_state(program, state_symbol, statement_index)
                })
        }
        ExpressionNode::Float(literal) => literal
            .landing()
            .and_then(|format| float_type_reference(program, format))
            .or_else(|| {
                contextual_type_reference_in_state(program, state_symbol, statement_index).filter(
                    |type_reference| {
                        program
                            .primitive_type_reference(*type_reference)
                            .is_some_and(|primitive| primitive.accepts_float_literal())
                    },
                )
            }),
        ExpressionNode::ZeroValue(type_reference) => Some(*type_reference),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::String(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::Unary(_) => None,
    }
}

/// Recover the declared landing type of a context-typed expression. Anonymous
/// float literals and compiler-synthesized `min`/`max` trees deliberately do
/// not invent standalone type references; their assignment/local declaration
/// is the checker-owned source of truth.
fn contextual_type_reference_in_state(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
) -> Option<TypeReferenceHandle> {
    let state = crate::semantic_calls::find_state(program, state_symbol)?;
    match program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?
    {
        typed_trees::statement::StatementNode::Assignment(assignment) => {
            expression_type_reference_in_state(
                program,
                state_symbol,
                statement_index,
                assignment.target,
            )
        }
        typed_trees::statement::StatementNode::LocalData(local) => Some(local.type_reference),
        _ => None,
    }
}

fn float_type_reference(program: &TypedTrees, format: FloatFormat) -> Option<TypeReferenceHandle> {
    let primitive = match format {
        FloatFormat::F32 => typed_trees::types::PrimitiveType::F32,
        FloatFormat::F64 => typed_trees::types::PrimitiveType::F64,
    };
    (1..=program.type_reference_table.type_reference_count())
        .map(|index| TypeReferenceHandle::from_arena_index(index as u32))
        .find(|type_reference| program.primitive_type_reference(*type_reference) == Some(primitive))
}

fn symbol_type_reference_in_state(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    symbol: SymbolHandle,
    name: Option<&str>,
) -> Option<TypeReferenceHandle> {
    if !symbol.is_valid() && name.is_none() {
        return None;
    }

    let state = crate::semantic_calls::find_state(program, state_symbol)?;
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| {
            (symbol.is_valid() && parameter.symbol == symbol)
                || name.is_some_and(|name| parameter.name.as_str() == name)
        })
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            local_type_reference_before_statement(program, state, statement_index, symbol, name)
        })
}

fn local_type_reference_before_statement(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    symbol: SymbolHandle,
    name: Option<&str>,
) -> Option<TypeReferenceHandle> {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| {
            let typed_trees::statement::StatementNode::LocalData(local) = statement else {
                return None;
            };
            ((symbol.is_valid() && local.symbol == symbol)
                || name.is_some_and(|name| local.name.as_str() == name))
            .then_some(local.type_reference)
        })
}

/// Field type for a `self.field` receiver. The `self` parameter often carries
/// no usable declared type reference, so resolve the field through the
/// machine's attached data definition instead.
fn self_field_type_reference(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> Option<TypeReferenceHandle> {
    let ExpressionNode::Name(path) = program.expression_table.expression(member.receiver) else {
        return None;
    };
    let state = crate::semantic_calls::find_state(program, state_symbol)?;
    let self_parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.is_self)?;
    let path_members = program.expression_table.name_path_members(path.members);
    let names_self = path_members.len() == 1
        && (path_members
            .first()
            .is_some_and(|name| name.as_str() == "self")
            || (path.symbol.is_valid() && path.symbol == self_parameter.symbol)
            || (path.head_symbol.is_valid() && path.head_symbol == self_parameter.symbol));
    if !names_self {
        return None;
    }

    let machine = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|machine_state| machine_state.symbol == state_symbol)
    })?;
    let attached_data = machine.attached_data.as_ref()?;
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name == *attached_data)?;
    program.data_members(data).iter().find_map(|data_member| {
        let typed_trees::data::DataMember::Field(field) = data_member else {
            return None;
        };
        ((member.member_symbol.is_valid() && field.symbol == member.member_symbol)
            || field.name == member.member)
            .then_some(field.type_reference)
    })
}

fn field_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    field_symbol: SymbolHandle,
    field_name: &typed_trees::name::Identifier,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => field_type_reference(program, *referee, field_symbol, field_name),
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
            .and_then(|data| data_field_type_reference(program, data, field_symbol, field_name)),
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Unit => None,
    }
}

fn data_field_type_reference(
    program: &TypedTrees,
    data: &typed_trees::data::DataDefinition,
    field_symbol: SymbolHandle,
    field_name: &typed_trees::name::Identifier,
) -> Option<TypeReferenceHandle> {
    program.data_members(data).iter().find_map(|member| {
        let typed_trees::data::DataMember::Field(field) = member else {
            return None;
        };
        if field_symbol.is_valid() {
            (field.symbol == field_symbol).then_some(field.type_reference)
        } else {
            (field.name == *field_name).then_some(field.type_reference)
        }
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
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => None,
    }
}
