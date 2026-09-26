//! Semantic operations and edges attributed to exact native byte intervals.

use semantic_vocabulary::{EdgeId, OperationId};

/// Semantic operation or edge owning one exact emitted byte interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SemanticCodeSite {
    Operation(OperationId),
    Edge(EdgeId),
}

/// Source-free semantic-to-code custody used by independent object replay.
/// It carries no runtime budget, charge, meter, or execution policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticCodeAttribution {
    pub site: SemanticCodeSite,
    pub operation_ordinal: usize,
    pub code_offset: usize,
    pub byte_count: usize,
}
