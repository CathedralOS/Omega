//! The existing scalar-return boundary operation with a whole parameter receiver.

use super::*;
use typed_trees::expression::TableCallExpression;

pub(super) fn is_supported(
    program: &TypedTrees,
    machine: &Machine,
    value: ExpressionHandle,
) -> bool {
    let [state] = program.machine_states(machine) else {
        return false;
    };
    let Some(result_type) = program.primitive_type_reference(state.return_type) else {
        return false;
    };
    if machine.attached_data.is_none()
        || !program.expression_table.expression_is_valid(value)
        || program.state_parameters(state).iter().any(|parameter| {
            parameter.is_const
                || program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
        })
    {
        return false;
    }
    let [
        StatementNode::LocalData(local),
        StatementNode::Expression(returned),
    ] = program.statement_table.statements(state.statement_nodes)
    else {
        return false;
    };
    if local.is_mutable
        || local.initial_value != value
        || program.primitive_type_reference(local.type_reference) != Some(result_type)
        || !program.expression_table.expression_is_valid(*returned)
        || !matches!(program.expression_table.expression(*returned), ExpressionNode::Name(name) if name.symbol == local.symbol)
    {
        return false;
    }
    initializer_target_is_supported(program, machine, local, false, true)
}

pub(super) fn has_parameter_receiver(
    program: &TypedTrees,
    machine: &Machine,
    call: &TableCallExpression,
    owner: &Machine,
    target: &State,
) -> bool {
    let parameters = program.state_parameters(target);
    let [receiver, rest @ ..] = parameters else {
        return false;
    };
    if !receiver.is_self || rest.iter().any(|parameter| parameter.is_self) {
        return false;
    }
    let arguments = program.expression_table.expression_handles(call.arguments);
    let source = if arguments.len() == rest.len() {
        call.receiver
    } else if arguments.len() == parameters.len() {
        if call.receiver.is_valid()
            && (!program.expression_table.expression_is_valid(call.receiver)
                || !matches!(program.expression_table.expression(call.receiver), ExpressionNode::Name(name) if name.symbol == owner.attached_data_symbol))
        {
            return false;
        }
        arguments[0]
    } else {
        return false;
    };
    if !program.expression_table.expression_is_valid(source) {
        return false;
    }
    let ExpressionNode::Name(name) = program.expression_table.expression(source) else {
        return false;
    };
    if program
        .expression_table
        .name_path_members(name.members)
        .len()
        != 1
    {
        return false;
    }
    let [state] = program.machine_states(machine) else {
        return false;
    };
    let mut parameters = program.state_parameters(state).iter().filter(|parameter| {
        parameter.symbol == name.symbol || (parameter.is_self && machine.symbol == name.symbol)
    });
    parameters.next().is_some_and(|parameter| {
        !parameter.is_const
            && program
                .primitive_type_reference(parameter.type_reference)
                .is_none()
    }) && parameters.next().is_none()
}
