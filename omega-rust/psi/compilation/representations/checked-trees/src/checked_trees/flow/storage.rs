//! Immutable reaching-storage observations at executable evaluation cuts.
//! Negative footprints cannot establish replacement contents. Stores retain
//! actual completed operands; joins retain every predecessor, including paths
//! that leave storage unchanged. These records grant no access or custody.

use super::{FlowOperandFact, FlowOperandReferent};
use arena::{Handle, HandleSpan};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlowStorageSnapshot {
    pub previous: Handle<FlowStorageSnapshot>,
    pub operation: FlowStorageOperation,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlowStorageOperation {
    #[default]
    Unknown,
    Writes {
        bindings: HandleSpan<FlowOperandReferent>,
        contents: HandleSpan<FlowOperandReferent>,
        bindings_complete: bool,
        contents_complete: bool,
    },
    Store {
        destination: FlowOperandReferent,
        value: Handle<FlowOperandFact>,
        projection: HandleSpan<facts::PlaceSegment>,
    },
    Join {
        predecessors: HandleSpan<Handle<FlowStorageSnapshot>>,
    },
}
