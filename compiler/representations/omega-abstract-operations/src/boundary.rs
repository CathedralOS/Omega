use omega_calling_conventions::HostOperationKey;
use omega_control_flow::StateKey;
use omega_core::arena::Arena;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AbstractBoundaryEdge {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub operation_key: HostOperationKey,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbstractBoundarySummary {
    pub edges: Arena<AbstractBoundaryEdge>,
}

impl AbstractBoundarySummary {
    pub fn with_capacity(edge_capacity: usize) -> Self {
        Self {
            edges: Arena::with_capacity(edge_capacity),
        }
    }
}
