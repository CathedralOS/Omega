use omega_calling_conventions::HostOperationKey;
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, Handle};
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AbstractSourceBoundaryEdge {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub boundary_trait_symbol: SymbolHandle,
    pub boundary_signature_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AbstractBoundaryEdge {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub operation_key: HostOperationKey,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AbstractBoundaryLink {
    pub source_edge: Handle<AbstractSourceBoundaryEdge>,
    pub lowered_edge: Handle<AbstractBoundaryEdge>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbstractBoundarySummary {
    pub source_edges: Arena<AbstractSourceBoundaryEdge>,
    pub edges: Arena<AbstractBoundaryEdge>,
    pub links: Arena<AbstractBoundaryLink>,
}

impl AbstractBoundarySummary {
    pub fn with_capacity(edge_capacity: usize) -> Self {
        Self::with_source_and_host_capacity(0, edge_capacity)
    }

    pub fn with_source_and_host_capacity(
        source_edge_capacity: usize,
        edge_capacity: usize,
    ) -> Self {
        Self {
            source_edges: Arena::with_capacity(source_edge_capacity),
            edges: Arena::with_capacity(edge_capacity),
            links: Arena::new(),
        }
    }
}
