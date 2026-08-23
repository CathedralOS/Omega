use super::RuntimeFlowBuilder;
use crate::{CallContext, RuntimeCycle, RuntimeState, RuntimeTransitionTarget};

impl RuntimeFlowBuilder<'_> {
    pub(super) fn target_is_active(&self, target: &RuntimeTransitionTarget) -> bool {
        let RuntimeTransitionTarget::State { key, context } = target else {
            return false;
        };

        self.active_states.contains(&(*key, *context))
    }

    pub(super) fn record_cycle_target(&mut self, target: &RuntimeTransitionTarget) {
        if let RuntimeTransitionTarget::State { key, context } = target {
            self.record_cycle_to(*key, *context);
        }
    }

    pub(super) fn record_cycle_to(
        &mut self,
        target_key: omega_control_flow::StateKey,
        target_context: CallContext,
    ) {
        let start_index = self
            .active_states
            .iter()
            .position(|(key, context)| *key == target_key && *context == target_context)
            .unwrap_or(0);
        let states = self.runtime_flow.cycle_states.insert_many(
            self.active_states
                .iter()
                .skip(start_index)
                .copied()
                .chain(std::iter::once((target_key, target_context)))
                .map(|(key, context)| RuntimeState { key, context }),
        );

        self.runtime_flow.cycles.insert(RuntimeCycle { states });
    }
}
