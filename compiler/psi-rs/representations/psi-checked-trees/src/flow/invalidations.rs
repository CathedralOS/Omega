use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowInvalidationSource {
    Statement {
        statement_index: usize,
    },
    Call {
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
    },
}

impl Default for FlowInvalidationSource {
    fn default() -> Self {
        Self::Statement { statement_index: 0 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowInvalidationFact {
    pub source: FlowInvalidationSource,
    pub context: psi_facts::FactContextHandle,
    pub fact: psi_facts::FactHandle,
    pub mutated_root: psi_facts::PlaceRoot,
    pub mutated_segments: HandleSpan<psi_facts::PlaceSegment>,
    pub dependency_segments: HandleSpan<psi_facts::PlaceSegment>,
}
