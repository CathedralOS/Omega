//! Ordinary control retains block identity and exact result-bearing exits.
use crate::{
    ScalarAbiValue, TargetBooleanExpression, TargetStructuralParameter, TargetUnitOperation,
};
use abstract_operations::ValueBinding;
use calling_conventions::CallPlan;
use semantic_vocabulary::{BlockId, EdgeId, ScalarType, ValueId};
use terminal_psi::{StructuralTypeDeclaration, TerminalAffineCleanupAction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetControlGraph {
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub call_plan: CallPlan,
    pub scalar_parameters: Vec<ScalarAbiValue>,
    pub parameters: Vec<TargetStructuralParameter>,
    pub entry: BlockId,
    pub blocks: Vec<TargetControlBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetControlBlock {
    pub block: BlockId,
    pub parameters: Vec<TargetScalarBlockParameter>,
    pub structural_parameters: Vec<terminal_psi::StructuralParameterDeclaration>,
    /// Nonterminal operations only; control is owned by the terminator.
    pub operations: Vec<TargetUnitOperation>,
    pub terminator: TargetControlTerminator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetScalarBlockParameter {
    pub value: ValueId,
    pub scalar_type: ScalarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetControlTerminator {
    ReturnScalar {
        psi_edge: EdgeId,
        source_value: ValueId,
        expression: crate::TargetScalarExpression,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    StructuralCase {
        source: crate::TargetStructuralHomeRequirement,
        cases: Vec<TargetControlCaseSuccessor>,
    },
    Return {
        psi_edge: EdgeId,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    Jump {
        successor: TargetControlSuccessor,
    },
    Conditional {
        condition_source: ValueId,
        condition: TargetBooleanExpression,
        when_true: TargetControlSuccessor,
        when_false: TargetControlSuccessor,
    },
}

/// Ordered sum alternative and the exact destination telescope it produces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetControlCaseSuccessor {
    pub psi_edge: EdgeId,
    pub case: semantic_vocabulary::StructuralCaseId,
    pub case_tag: i32,
    pub target: BlockId,
    pub payloads: Vec<TargetControlCasePayload>,
    pub trivial_affine_discards: Vec<semantic_vocabulary::PlaceId>,
}

/// A payload is defined by its destination parameter, not the sum producer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetControlCasePayload {
    pub field: semantic_vocabulary::StructuralFieldId,
    pub field_byte_offset: u32,
    pub parameter: crate::TargetScalarBlockValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetControlSuccessor {
    pub psi_edge: EdgeId,
    pub target: BlockId,
    pub bindings: Vec<ValueBinding>,
    pub structural_bindings: Vec<abstract_operations::AbstractStructuralBinding>,
    pub cleanup_actions: Vec<TerminalAffineCleanupAction>,
}
