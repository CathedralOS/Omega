use arena::HandleSpan;
use symbols::SymbolHandle;

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
    pub context: crate::fact_plan::FactContextHandle,
    pub fact: crate::fact_plan::FactHandle,
    pub mutated_root: crate::fact_plan::PlaceRoot,
    pub mutated_segments: HandleSpan<crate::fact_plan::PlaceSegment>,
    pub dependency_segments: HandleSpan<crate::fact_plan::PlaceSegment>,
}
