use crate::control_flow::OperationKind;
use crate::identity::NativeStringStorage;
use crate::identity::expressions::count_expression_strings;
use crate::identity::targets::count_planned_target_strings;
use crate::plan::NativePlan;

pub(in crate::identity) fn count_control_flow_strings(
    native_plan: &NativePlan,
    storage: &mut NativeStringStorage,
) {
    for (_, machine) in native_plan.control_flow.machines.iter() {
        storage.count_program_name_identity(&machine.name);
        for contained in &machine.contains {
            storage.count_program_name_identity(&contained.name);
            storage.count_program_name_identity(&contained.type_name);
        }
    }

    for (_, state) in native_plan.control_flow.states.iter() {
        storage.count_program_name_identity(&state.name);
        for parameter in &state.parameters {
            storage.count_program_name_identity(&parameter.name);
        }
    }

    for (_, operation) in native_plan.control_flow.operations.iter() {
        match &operation.kind {
            OperationKind::Assignment { target, value }
            | OperationKind::StaticAssignment { target, value } => {
                count_expression_strings(target, storage);
                count_expression_strings(value, storage);
            }
            OperationKind::Call {
                receiver: _,
                target: _,
                arguments,
            } => {
                for argument in arguments {
                    count_expression_strings(argument, storage);
                }
            }
            OperationKind::ConstantIntegerAssignment
            | OperationKind::Expression
            | OperationKind::LocalData => {}
        }
    }

    for (_, transition) in native_plan.control_flow.transitions.iter() {
        count_planned_target_strings(&transition.target, storage);
        if let Some(continuation) = &transition.continuation {
            count_planned_target_strings(continuation, storage);
        }
    }
}
