use super::RuntimeFlowBuilder;
use crate::{RuntimeCycle, RuntimeState, RuntimeTransitionTarget};

impl RuntimeFlowBuilder<'_> {
    pub(super) fn target_is_active(&self, target: &RuntimeTransitionTarget) -> bool {
        let RuntimeTransitionTarget::State { key, .. } = target else {
            return false;
        };

        self.active_states
            .iter()
            .any(|active_state| active_state.key == *key)
    }

    pub(super) fn record_cycle_target(&mut self, target: &RuntimeTransitionTarget) {
        if let RuntimeTransitionTarget::State { key, .. } = target {
            self.record_cycle_to(&RuntimeState { key: *key });
        }
    }

    pub(super) fn record_cycle_to(&mut self, target: &RuntimeState) {
        let start_index = self
            .active_states
            .iter()
            .position(|active_state| active_state == target)
            .unwrap_or(0);
        let cycle_states = self
            .active_states
            .iter()
            .skip(start_index)
            .cloned()
            .chain(std::iter::once(target.clone()))
            .collect::<Vec<_>>();
        let states = self.runtime_flow.cycle_states.insert_many(cycle_states);

        self.runtime_flow.cycles.insert(RuntimeCycle { states });
    }
}
