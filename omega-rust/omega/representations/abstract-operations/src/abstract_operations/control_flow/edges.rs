//! Semantic successor edges and case-scoped structural payload bindings.

use crate::ValueBinding;
use semantic_vocabulary::{BlockId, EdgeId, PlaceId, ScalarType, StructuralCaseId, ValueId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractSuccessor {
    pub psi_edge: EdgeId,
    pub target: BlockId,
    pub bindings: Vec<ValueBinding>,
    pub structural_bindings: Vec<AbstractStructuralBinding>,
    /// Exact Terminal-Psi cleanup order for this conditional edge.
    pub trivial_affine_discards: Vec<PlaceId>,
}

/// Exact positional shared-place arrival, independent of the scalar telescope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractStructuralBinding {
    pub parameter: PlaceId,
    pub argument: terminal_psi::StructuralArgument,
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
