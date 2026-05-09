use crate::identity::NativeStringStorage;
use crate::identity::expressions::count_expression_strings;
use crate::runtime_flow::RuntimeTransitionTarget;
use omega_control_flow::PlannedTransitionTarget;

pub(in crate::identity) fn count_planned_target_strings(
    target: &PlannedTransitionTarget,
    storage: &mut NativeStringStorage,
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
    storage: &mut NativeStringStorage,
) {
    match target {
        RuntimeTransitionTarget::Unknown { name } => storage.count_identity(name),
        RuntimeTransitionTarget::State { .. }
        | RuntimeTransitionTarget::Terminal
        | RuntimeTransitionTarget::None => {}
    }
}
