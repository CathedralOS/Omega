use super::static_values::{PlaceKey, StaticValue};
use omega_control_flow::StateKey;

const INLINE_VISITED_STATE_COUNT: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledState {
    pub key: StateKey,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScheduledStateCollector {
    states: Vec<ScheduledState>,
}

impl ScheduledStateCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_key(&mut self, key: StateKey) {
        if !self.states.iter().any(|state| state.key == key) {
            self.states.push(ScheduledState { key });
        }
    }

    pub fn finish(self) -> Vec<ScheduledState> {
        self.states
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ScheduleCheckpoint {
    visited_len: usize,
    aliases_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StateScheduleWorkspace {
    schedule: Vec<ScheduledState>,
    visited: VisitedStates,
    values: Vec<(PlaceKey, StaticValue)>,
    aliases: Vec<(PlaceKey, PlaceKey)>,
}

impl StateScheduleWorkspace {
    pub(super) fn with_capacities(
        state_capacity: usize,
        static_value_capacity: usize,
        alias_capacity: usize,
    ) -> Self {
        Self {
            schedule: Vec::with_capacity(state_capacity),
            visited: VisitedStates::with_capacity(state_capacity),
            values: Vec::with_capacity(static_value_capacity),
            aliases: Vec::with_capacity(alias_capacity),
        }
    }

    pub(super) fn finish(self) -> Vec<ScheduledState> {
        self.schedule
    }

    pub(super) fn schedule_mut(&mut self) -> &mut Vec<ScheduledState> {
        &mut self.schedule
    }

    pub(super) fn contains_visited(&self, state: ScheduledState) -> bool {
        self.visited.contains(state)
    }

    pub(super) fn visited_iter(&self) -> impl Iterator<Item = ScheduledState> + '_ {
        self.visited.iter()
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct VisitedStates {
    inline: [Option<ScheduledState>; INLINE_VISITED_STATE_COUNT],
    len: usize,
    overflow: Vec<ScheduledState>,
}

impl VisitedStates {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            inline: [None; INLINE_VISITED_STATE_COUNT],
            len: 0,
            overflow: Vec::with_capacity(capacity.saturating_sub(INLINE_VISITED_STATE_COUNT)),
        }
    }

    fn len(&self) -> usize {
        self.len
    }

    fn contains(&self, state: ScheduledState) -> bool {
        self.iter().any(|visited| visited == state)
    }

    fn iter(&self) -> impl Iterator<Item = ScheduledState> + '_ {
        self.inline
            .iter()
            .take(self.len.min(INLINE_VISITED_STATE_COUNT))
            .filter_map(|state| *state)
            .chain(self.overflow.iter().copied())
    }

    fn push(&mut self, state: ScheduledState) {
        if self.len < INLINE_VISITED_STATE_COUNT {
            self.inline[self.len] = Some(state);
        } else {
            self.overflow.push(state);
        }

        self.len += 1;
    }

    fn truncate(&mut self, target_len: usize) {
        while self.len > target_len {
            self.len -= 1;

            if self.len < INLINE_VISITED_STATE_COUNT {
                self.inline[self.len] = None;
            } else {
                self.overflow.pop();
            }
        }
    }
}
