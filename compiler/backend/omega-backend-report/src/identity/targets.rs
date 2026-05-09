use crate::identity::BackendStringStorage;
use crate::identity::expressions::count_expression_strings;
use omega_control_flow::PlannedTransitionTarget;
use omega_state_graph::RuntimeTransitionTarget;

pub(in crate::identity) fn count_planned_target_strings(
    target: &PlannedTransitionTarget,
    storage: &mut BackendStringStorage,
) {
    match target {
        PlannedTransitionTarget::State {
            name, arguments, ..
        } => {
            storage.count_program_name_identity(name);
            for argument in arguments {
                count_expression_strings(argument, storage);
            }
        }
        PlannedTransitionTarget::Nested {
            receiver,
            state,
            arguments,
            ..
        } => {
            storage.count_program_name_identity(receiver);
            storage.count_program_name_identity(state);
            for argument in arguments {
                count_expression_strings(argument, storage);
            }
        }
        PlannedTransitionTarget::SelfTarget | PlannedTransitionTarget::Terminal => {}
    }
}

pub(in crate::identity) fn count_runtime_target_strings(
    target: &RuntimeTransitionTarget,
    storage: &mut BackendStringStorage,
) {
    match target {
        RuntimeTransitionTarget::Unknown { name } => storage.count_identity(name),
        RuntimeTransitionTarget::State { .. }
        | RuntimeTransitionTarget::Terminal
        | RuntimeTransitionTarget::None => {}
    }
}
