use crate::{AbstractBoundarySummary, AbstractOwnershipSummary, AbstractValueSummary};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AbstractSemanticSummary {
    pub values: AbstractValueSummary,
    pub boundary_edges: AbstractBoundarySummary,
    pub ownership: AbstractOwnershipSummary,
}
