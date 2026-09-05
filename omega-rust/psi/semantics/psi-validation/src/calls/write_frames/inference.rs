//! Transient inference state. Active body guards and frozen local case evidence
//! have separate lifetimes; neither is a source of borrow permission.

use super::StoredLocalOrigins;
use psi_facts::PlaceSegment;
use psi_symbols::SymbolHandle;

#[derive(Clone, Default)]
pub(super) struct FrameInference {
    pub(super) active_states: Vec<SymbolHandle>,
    local_cases: Vec<(SymbolHandle, Vec<Vec<PlaceSegment>>)>,
}

impl FrameInference {
    pub(super) fn for_state(symbol: SymbolHandle) -> Self {
        Self {
            active_states: vec![symbol],
            ..Self::default()
        }
    }
    pub(super) fn record_local(&mut self, local: &StoredLocalOrigins) {
        self.local_cases
            .push((local.local_symbol, local.cases.clone()));
    }

    pub(super) fn local_cases(&self, symbol: SymbolHandle) -> Option<&[Vec<PlaceSegment>]> {
        self.local_cases
            .iter()
            .rev()
            .find_map(|(local, cases)| (*local == symbol).then_some(cases.as_slice()))
    }

    pub(super) fn with_local_scope<T>(&mut self, action: impl FnOnce(&mut Self) -> T) -> T {
        let start = self.local_cases.len();
        let result = action(self);
        self.local_cases.truncate(start);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(symbol: SymbolHandle, variant: SymbolHandle) -> StoredLocalOrigins {
        StoredLocalOrigins {
            local_symbol: symbol,
            references: Vec::new(),
            cases: vec![vec![PlaceSegment::Case { variant }]],
        }
    }

    #[test]
    fn local_case_scope_restores_outer_evidence_on_failure() {
        let symbol = SymbolHandle::from_parts(1, 1);
        let outer = local(symbol, SymbolHandle::from_parts(2, 1));
        let inner = local(symbol, SymbolHandle::from_parts(3, 1));
        let mut inference = FrameInference::for_state(symbol);
        inference.record_local(&outer);
        let result: Option<()> = inference.with_local_scope(|inference| {
            inference.record_local(&inner);
            assert_eq!(inference.local_cases(symbol), Some(inner.cases.as_slice()));
            None
        });
        assert!(result.is_none());
        assert_eq!(inference.local_cases(symbol), Some(outer.cases.as_slice()));
        assert_eq!(inference.active_states, vec![symbol]);
    }

    #[test]
    fn empty_case_evidence_is_distinct_from_an_unknown_local() {
        let symbol = SymbolHandle::from_parts(1, 1);
        let mut inference = FrameInference::default();
        inference.record_local(&StoredLocalOrigins {
            local_symbol: symbol,
            references: Vec::new(),
            cases: Vec::new(),
        });
        assert_eq!(inference.local_cases(symbol), Some([].as_slice()));
        assert_eq!(inference.local_cases(SymbolHandle::from_parts(1, 2)), None);
        assert_eq!(inference.local_cases(SymbolHandle::invalid()), None);
    }
}
