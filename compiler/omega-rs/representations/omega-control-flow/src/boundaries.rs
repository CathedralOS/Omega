use psi_arena::{Arena, HandleSpan};
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowBoundaryRoots {
    pub edges: Arena<StateBoundaryEdge>,
}

impl ControlFlowBoundaryRoots {
    pub fn with_roots(edges: Arena<StateBoundaryEdge>) -> Self {
        Self { edges }
    }
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
