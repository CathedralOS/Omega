use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraphBoundaryRoots {
    pub edges: Arena<StateBoundaryEdge>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBoundaryEdge {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub boundary_trait_symbol: SymbolHandle,
    pub boundary_signature_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBoundarySummary {
    pub edges: HandleSpan<StateBoundaryEdge>,
}
