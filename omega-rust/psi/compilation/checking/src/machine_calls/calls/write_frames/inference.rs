//! Transient inference state. Active body guards and frozen local origin evidence
//! have separate lifetimes; neither is a source of borrow permission.

use super::StoredLocalOrigins;
use symbols::SymbolHandle;

#[derive(Clone, Default)]
pub(super) struct FrameInference {
    pub(super) active_states: Vec<SymbolHandle>,
    local_shapes: Vec<StoredLocalOrigins>,
}

impl FrameInference {
    pub(super) fn for_state(symbol: SymbolHandle) -> Self {
        Self {
            active_states: vec![symbol],
            ..Self::default()
        }
    }
    pub(super) fn record_local(&mut self, local: &StoredLocalOrigins) {
        self.local_shapes.push(local.clone());
    }

    pub(super) fn local_origins(&self, symbol: SymbolHandle) -> Option<&StoredLocalOrigins> {
        self.local_shapes
            .iter()
            .rev()
            .find(|local| local.local_symbol == symbol)
    }



    pub(super) fn with_local_scope<T>(&mut self, action: impl FnOnce(&mut Self) -> T) -> T {
        let start = self.local_shapes.len();
        let result = action(self);
        self.local_shapes.truncate(start);
        result
    }
}

