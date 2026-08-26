use psi_symbols::SymbolHandle;

const INLINE_HELPER_STATE_STACK_COUNT: usize = 16;

pub(super) struct HelperStateStack {
    inline: [Option<SymbolHandle>; INLINE_HELPER_STATE_STACK_COUNT],
    len: usize,
    overflow: Vec<SymbolHandle>,
}

impl HelperStateStack {
    pub(super) fn with_capacity(state_capacity: usize) -> Self {
        Self {
            inline: [None; INLINE_HELPER_STATE_STACK_COUNT],
            len: 0,
            overflow: Vec::with_capacity(
                state_capacity.saturating_sub(INLINE_HELPER_STATE_STACK_COUNT),
            ),
        }
    }

    pub(super) fn contains(&self, symbol: SymbolHandle) -> bool {
        self.inline
            .iter()
            .take(self.len.min(INLINE_HELPER_STATE_STACK_COUNT))
            .flatten()
            .any(|candidate| *candidate == symbol)
            || self.overflow.contains(&symbol)
    }

    pub(super) fn push(&mut self, symbol: SymbolHandle) {
        if self.len < INLINE_HELPER_STATE_STACK_COUNT {
            self.inline[self.len] = Some(symbol);
        } else {
            self.overflow.push(symbol);
        }

        self.len += 1;
    }

    pub(super) fn pop(&mut self) {
        if self.len == 0 {
            return;
        }

        self.len -= 1;
        if self.len < INLINE_HELPER_STATE_STACK_COUNT {
            self.inline[self.len] = None;
        } else {
            self.overflow.pop();
        }
    }
}
