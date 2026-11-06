//! Immutable reaching-storage observations at executable evaluation cuts.
//! Negative footprints cannot establish replacement contents. Stores retain
//! actual completed operands; joins retain every predecessor, including paths
//! that leave storage unchanged. These records grant no access or custody.

use super::{FlowOperandFact, FlowOperandReferent};
use arena::{Handle, HandleSpan};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowStorageSnapshot {
    pub previous: Handle<FlowStorageSnapshot>,
    pub operation: FlowStorageOperation,
}

/// The checked endpoint changed by this assignment's own write. Calls and
/// unknown effects keep their independent conservative footprints.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowStorageWriteEndpoint {
    pub state: symbols::SymbolHandle,
    pub statement_index: usize,
    pub receiving_type: typed_trees::types::TypeReferenceHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum FlowStorageOperation {
    #[default]
    Unknown,
    /// The real beginning of this state's storage history, before any operand
    /// or statement writes. Missing snapshot handles remain unavailable.
    Entry,
    Writes {
        bindings: HandleSpan<FlowOperandReferent>,
        contents: HandleSpan<FlowOperandReferent>,
        bindings_complete: bool,
        contents_complete: bool,
        endpoint: Option<FlowStorageWriteEndpoint>,
    },
    Store {
        /// The immutable declaration namespace owning the performed store.
        state: symbols::SymbolHandle,
        statement_index: usize,
        /// Selected receiving endpoint at the performed store. A pointee
        /// payload and its reference descriptor may share source coordinates.
        receiving_type: typed_trees::types::TypeReferenceHandle,
        /// The original pre-RHS destination, when this is an assignment.
        target: Handle<FlowOperandFact>,
        destination: FlowOperandReferent,
        value: Handle<FlowOperandFact>,
        projection: HandleSpan<facts::PlaceSegment>,
    },
    /// A complete call relation preserved this exact descriptor binding.
    /// Rejoin its actual entry cut, including unavailable/unchanged incoming;
    /// no value, pointer, pointee contents or access is manufactured.
    PreserveBinding {
        destination: FlowOperandReferent,
        source: Handle<FlowStorageSnapshot>,
    },
    Join {
        predecessors: HandleSpan<Handle<FlowStorageSnapshot>>,
    },
}

/// A normal call's installed reference keeps the actual destination that was
/// evaluated before its writes. This also identifies the old descriptor when
/// a conditional store leaves that destination unchanged. It grants no access.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct FlowCallStoredOutput {
    pub actual: Handle<FlowOperandFact>,
    /// Destination path below the actual's outer reference, including empty.
    pub path: HandleSpan<facts::PlaceSegment>,
    pub destination: FlowOperandReferent,
}
