use crate::BackendReportInput;
use crate::identity::BackendStringStorage;
use crate::identity::expressions::{
    count_control_flow_expression_span_strings, count_control_flow_expression_strings,
};
use crate::identity::targets::count_planned_target_strings;
use omega_control_flow::OperationExpressionRefs;

pub(in crate::identity) fn count_control_flow_strings(
    backend_plan: &BackendReportInput<'_>,
    storage: &mut BackendStringStorage,
) {
    for (_, machine) in backend_plan.control_flow.machines.iter() {
        storage.count_program_name_identity(&machine.name);
        for contained in backend_plan.control_flow.machine_contains(machine) {
            storage.count_program_name_identity(&contained.name);
            storage.count_program_name_identity(&contained.type_name);
        }
    }

    for (_, state) in backend_plan.control_flow.states.iter() {
        storage.count_program_name_identity(&state.name);
        for parameter in backend_plan.control_flow.state_parameters(state) {
            storage.count_program_name_identity(&parameter.name);
        }
    }

    for (_, operation) in backend_plan.control_flow.operations.iter() {
        match operation.expressions {
            OperationExpressionRefs::Assignment { target, value } => {
                count_control_flow_expression_strings(
                    &backend_plan.control_flow.expressions,
                    target,
                    storage,
                );
                count_control_flow_expression_strings(
                    &backend_plan.control_flow.expressions,
                    value,
                    storage,
                );
            }
            OperationExpressionRefs::Call { arguments } => {
                count_control_flow_expression_span_strings(
                    &backend_plan.control_flow.expressions,
                    arguments,
                    storage,
                );
            }
            OperationExpressionRefs::Expression(expression) => {
                count_control_flow_expression_strings(
                    &backend_plan.control_flow.expressions,
                    expression,
                    storage,
                );
            }
            OperationExpressionRefs::None => {}
        }
    }

    for (_, transition) in backend_plan.control_flow.transitions.iter() {
        count_planned_target_strings(&transition.target, storage);
        count_planned_target_strings(&transition.continuation, storage);
        count_control_flow_expression_span_strings(
            &backend_plan.control_flow.expressions,
            transition.expressions.target_arguments,
            storage,
        );
        count_control_flow_expression_span_strings(
            &backend_plan.control_flow.expressions,
            transition.expressions.continuation_arguments,
            storage,
        );
        if transition.expressions.guard.is_valid() {
            count_control_flow_expression_strings(
                &backend_plan.control_flow.expressions,
                transition.expressions.guard,
                storage,
            );
        }
    }
}
