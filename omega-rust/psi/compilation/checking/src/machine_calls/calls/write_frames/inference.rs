//! Transient inference state. Active body guards and frozen local origin evidence
//! have separate lifetimes; neither is a source of borrow permission.

use super::StoredLocalOrigins;
#[cfg(test)]
use super::path_instantiation::aggregate_arguments::AggregateMove;
#[cfg(test)]
use facts::PlaceSegment;
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

    #[cfg(test)]
    pub(super) fn local_moves(&self, symbol: SymbolHandle) -> Option<&[AggregateMove]> {
        self.local_origins(symbol)
            .map(|local| local.moves.as_slice())
    }

    #[cfg(test)]
    pub(super) fn local_cases(&self, symbol: SymbolHandle) -> Option<&[Vec<PlaceSegment>]> {
        self.local_origins(symbol)
            .map(|local| local.cases.as_slice())
    }

    pub(super) fn with_local_scope<T>(&mut self, action: impl FnOnce(&mut Self) -> T) -> T {
        let start = self.local_shapes.len();
        let result = action(self);
        self.local_shapes.truncate(start);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::{AggregateMove, FrameInference, PlaceSegment, StoredLocalOrigins, SymbolHandle};

    fn local(symbol: SymbolHandle, variant: SymbolHandle) -> StoredLocalOrigins {
        StoredLocalOrigins {
            local_symbol: symbol,
            references: Vec::new(),
            cases: vec![vec![PlaceSegment::Case { variant }]],
            moves: Vec::new(),
            symbolic: false,
        }
    }

    #[test]
    fn empty_case_evidence_is_distinct_from_an_unknown_local() {
        let symbol = SymbolHandle::from_parts(1, 1);
        let mut inference = FrameInference::default();
        inference.record_local(&StoredLocalOrigins {
            local_symbol: symbol,
            references: Vec::new(),
            cases: Vec::new(),
            moves: Vec::new(),
            symbolic: false,
        });
        assert_eq!(inference.local_cases(symbol), Some([].as_slice()));
        assert_eq!(inference.local_cases(SymbolHandle::from_parts(1, 2)), None);
        assert_eq!(inference.local_cases(SymbolHandle::invalid()), None);
    }

    #[test]
    fn moved_input_identity_restores_with_its_local_case_scope() {
        let symbol = SymbolHandle::from_parts(1, 1);
        let mut outer = local(symbol, SymbolHandle::from_parts(2, 1));
        let source = SymbolHandle::from_parts(3, 1);
        outer.moves.push(AggregateMove {
            local_segments: Vec::new(),
            source: super::super::FrameSourcePlace {
                kind: Default::default(),
                root: source,
                segments: Vec::new(),
                builtin_coordinates: true,
            },
            type_reference: Default::default(),
        });
        let inner = local(symbol, SymbolHandle::from_parts(4, 1));
        let mut inference = FrameInference::default();
        inference.record_local(&outer);
        let result: Option<()> = inference.with_local_scope(|inference| {
            inference.record_local(&inner);
            assert!(
                inference
                    .local_moves(symbol)
                    .expect("known local")
                    .is_empty()
            );
            None
        });
        assert!(result.is_none());
        assert_eq!(
            inference.local_moves(symbol).expect("outer local")[0]
                .source
                .root,
            source
        );
        assert!(
            inference
                .local_moves(SymbolHandle::from_parts(1, 2))
                .is_none()
        );
    }
}
