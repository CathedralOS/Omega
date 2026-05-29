use omega_control_flow::StateKey;

const INLINE_RUNTIME_STATE_COUNT: usize = 32;

pub(super) struct StateKeyBuffer {
    inline: [Option<StateKey>; INLINE_RUNTIME_STATE_COUNT],
    len: usize,
    overflow: Vec<StateKey>,
}

impl StateKeyBuffer {
    pub(super) fn with_capacity(state_capacity: usize) -> Self {
        Self {
            inline: [None; INLINE_RUNTIME_STATE_COUNT],
            len: 0,
            overflow: Vec::with_capacity(state_capacity.saturating_sub(INLINE_RUNTIME_STATE_COUNT)),
        }
    }

    pub(super) fn contains(&self, key: &StateKey) -> bool {
        self.inline
            .iter()
            .take(self.len.min(INLINE_RUNTIME_STATE_COUNT))
            .flatten()
            .any(|candidate| candidate == key)
            || self.overflow.contains(key)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &StateKey> {
        self.inline
            .iter()
            .take(self.len.min(INLINE_RUNTIME_STATE_COUNT))
            .filter_map(Option::as_ref)
            .chain(self.overflow.iter())
    }

    pub(super) fn push(&mut self, key: StateKey) {
        if self.len < INLINE_RUNTIME_STATE_COUNT {
            self.inline[self.len] = Some(key);
        } else {
            self.overflow.push(key);
        }

        self.len += 1;
    }

    pub(super) fn pop(&mut self) {
        if self.len == 0 {
            return;
        }

        self.len -= 1;
        if self.len < INLINE_RUNTIME_STATE_COUNT {
            self.inline[self.len] = None;
        } else {
            self.overflow.pop();
        }
    }

    pub(super) fn last(&self) -> Option<StateKey> {
        if self.len == 0 {
            return None;
        }

        if self.len <= INLINE_RUNTIME_STATE_COUNT {
            return self.inline[self.len - 1];
        }

        self.overflow.last().copied()
    }
}
