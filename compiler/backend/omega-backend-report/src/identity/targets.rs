use crate::identity::BackendStringStorage;
use omega_control_flow::PlannedTransitionTarget;

pub(in crate::identity) fn count_planned_target_strings(
    target: &PlannedTransitionTarget,
    storage: &mut BackendStringStorage,
) {
    match target {
        PlannedTransitionTarget::State { name, .. } => {
            storage.count_program_name_identity(name);
        }
        PlannedTransitionTarget::Nested {
            receiver, state, ..
        } => {
            storage.count_program_name_identity(receiver);
            storage.count_program_name_identity(state);
        }
        PlannedTransitionTarget::None
        | PlannedTransitionTarget::SelfTarget
        | PlannedTransitionTarget::Terminal => {}
    }
}
