use super::static_values::{PlaceKey, StaticValue};
use omega_control_flow::StateKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledState {
    pub key: StateKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ScheduleCheckpoint {
    visited_len: usize,
    aliases_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StateScheduleWorkspace {
    schedule: Vec<ScheduledState>,
    visited: Vec<ScheduledState>,
    values: Vec<(PlaceKey, StaticValue)>,
    aliases: Vec<(PlaceKey, PlaceKey)>,
}

impl StateScheduleWorkspace {
    pub(super) fn with_state_capacity(state_capacity: usize) -> Self {
        Self {
            schedule: Vec::with_capacity(state_capacity),
            visited: Vec::with_capacity(state_capacity),
            values: Vec::new(),
            aliases: Vec::new(),
        }
    }

    pub(super) fn finish(self) -> Vec<ScheduledState> {
        self.schedule
    }

    pub(super) fn schedule_mut(&mut self) -> &mut Vec<ScheduledState> {
        &mut self.schedule
    }

    pub(super) fn visited(&self) -> &[ScheduledState] {
        &self.visited
    }

    pub(super) fn push_visited(&mut self, state: ScheduledState) {
        self.visited.push(state);
    }

    pub(super) fn values(&self) -> &[(PlaceKey, StaticValue)] {
        &self.values
    }

    pub(super) fn values_mut(&mut self) -> &mut Vec<(PlaceKey, StaticValue)> {
        &mut self.values
    }

    pub(super) fn static_bindings_mut(
        &mut self,
    ) -> (&[(PlaceKey, PlaceKey)], &mut Vec<(PlaceKey, StaticValue)>) {
        (&self.aliases, &mut self.values)
    }

    pub(super) fn aliases(&self) -> &[(PlaceKey, PlaceKey)] {
        &self.aliases
    }

    pub(super) fn set_alias(&mut self, parameter: PlaceKey, target: PlaceKey) {
        if let Some((_, existing_target)) = self
            .aliases
            .iter_mut()
            .find(|(existing_parameter, _)| existing_parameter == &parameter)
        {
            *existing_target = target;
        } else {
            self.aliases.push((parameter, target));
        }
    }

    pub(super) fn checkpoint(&self) -> ScheduleCheckpoint {
        ScheduleCheckpoint {
            visited_len: self.visited.len(),
            aliases_len: self.aliases.len(),
        }
    }

    pub(super) fn restore(&mut self, checkpoint: ScheduleCheckpoint) {
        self.visited.truncate(checkpoint.visited_len);
        self.aliases.truncate(checkpoint.aliases_len);
    }
}
