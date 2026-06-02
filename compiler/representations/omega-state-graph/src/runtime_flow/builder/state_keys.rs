use crate::CallContext;
use omega_control_flow::StateKey;

const INLINE_RUNTIME_STATE_COUNT: usize = 32;

/// A runtime-flow node: a source state together with the call-context copy it
/// belongs to. Cloning specializes a called machine per context, so dedup and
/// cycle detection must distinguish `(key, context)` pairs rather than bare keys.
pub(super) type RuntimeNode = (StateKey, CallContext);

pub(super) struct StateKeyBuffer {
    inline: [Option<RuntimeNode>; INLINE_RUNTIME_STATE_COUNT],
    len: usize,
    overflow: Vec<RuntimeNode>,
}

impl StateKeyBuffer {
    pub(super) fn with_capacity(state_capacity: usize) -> Self {
        Self {
            inline: [None; INLINE_RUNTIME_STATE_COUNT],
            len: 0,
            overflow: Vec::with_capacity(state_capacity.saturating_sub(INLINE_RUNTIME_STATE_COUNT)),
        }
    }

    pub(super) fn contains(&self, node: &RuntimeNode) -> bool {
        self.inline
            .iter()
            .take(self.len.min(INLINE_RUNTIME_STATE_COUNT))
            .flatten()
            .any(|candidate| candidate == node)
            || self.overflow.contains(node)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &RuntimeNode> {
        self.inline
            .iter()
            .take(self.len.min(INLINE_RUNTIME_STATE_COUNT))
            .filter_map(Option::as_ref)
            .chain(self.overflow.iter())
    }

    pub(super) fn push(&mut self, node: RuntimeNode) {
        if self.len < INLINE_RUNTIME_STATE_COUNT {
            self.inline[self.len] = Some(node);
        } else {
            self.overflow.push(node);
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

    pub(super) fn last(&self) -> Option<RuntimeNode> {
        if self.len == 0 {
            return None;
        }

        if self.len <= INLINE_RUNTIME_STATE_COUNT {
            return self.inline[self.len - 1];
        }

        self.overflow.last().copied()
    }
}
