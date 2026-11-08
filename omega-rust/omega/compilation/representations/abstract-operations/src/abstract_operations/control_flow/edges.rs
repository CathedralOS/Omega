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
    pub cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
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
    pub structural_bindings: Vec<AbstractStructuralBinding>,
    pub cleanup_actions: Vec<terminal_psi::TerminalAffineCleanupAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbstractStructuralCasePayloadBinding {
    pub parameter: ValueId,
    pub field: semantic_vocabulary::StructuralFieldId,
    pub scalar_type: ScalarType,
}

impl AbstractSuccessor {
    /// The structural binding keyed `parameter`, when the successor declares
    /// it.
    pub fn structural_binding(&self, parameter: PlaceId) -> Option<&AbstractStructuralBinding> {
        self.structural_bindings
            .iter()
            .find(|binding| binding.parameter == parameter)
    }
}
