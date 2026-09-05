//! Semantic successor edges and case-scoped structural payload bindings.

use crate::ValueBinding;
use semantic_vocabulary::{BlockId, EdgeId, PlaceId, ScalarType, StructuralCaseId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractSuccessor {
    pub psi_edge: EdgeId,
    pub target: BlockId,
    pub bindings: Vec<ValueBinding>,
    /// Exact Terminal-Psi cleanup order for this conditional edge.
    pub trivial_affine_discards: Vec<PlaceId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractStructuralCaseSuccessor {
    pub psi_edge: EdgeId,
    pub target: BlockId,
    pub case: StructuralCaseId,
    pub payloads: Vec<AbstractStructuralCasePayloadBinding>,
    pub trivial_affine_discards: Vec<PlaceId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractStructuralCasePayloadBinding {
    pub parameter: ValueId,
    pub field: semantic_vocabulary::StructuralFieldId,
    pub scalar_type: ScalarType,
}
