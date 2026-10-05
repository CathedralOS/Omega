//! The existing scalar-return boundary operation with a whole parameter receiver.
use super::{
    ExpressionHandle, ExpressionNode, Machine, StatementNode, TypedTrees,
    initializer_target_is_supported,
};

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
                || (program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
                    && (parameter.is_mutable))
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
    initializer_target_is_supported(
        program,
        machine,
        local.initial_value,
        local.type_reference,
        false,
    )
}
