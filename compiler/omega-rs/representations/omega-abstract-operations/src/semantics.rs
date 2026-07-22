use crate::{AbstractBoundarySummary, AbstractOwnershipSummary, AbstractValueSummary};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AbstractSemanticSummary {
    pub values: AbstractValueSummary,
    pub boundaries: AbstractBoundarySummary,
    pub ownership: AbstractOwnershipSummary,
}

impl AbstractSemanticSummary {
    pub fn with_roots(
        values: AbstractValueSummary,
        boundaries: AbstractBoundarySummary,
        ownership: AbstractOwnershipSummary,
    ) -> Self {
        Self {
            values,
            boundaries,
            ownership,
        }
    }

    pub fn with_capacity(
        value_capacity: usize,
        source_boundary_edge_capacity: usize,
        boundary_edge_capacity: usize,
        ownership_segment_capacity: usize,
        permission_capacity: usize,
    ) -> Self {
        Self::with_roots(
            AbstractValueSummary::with_capacity(value_capacity),
            AbstractBoundarySummary::with_source_and_host_capacity(
                source_boundary_edge_capacity,
                boundary_edge_capacity,
            ),
            AbstractOwnershipSummary::with_capacity(
                ownership_segment_capacity,
                permission_capacity,
            ),
        )
    }
}
