//! Ordinary Unit control retains block identity rather than arm-layout templates.
use crate::{
    ScalarAbiValue, TargetBooleanExpression, TargetStructuralParameter, TargetUnitOperation,
};
use abstract_operations::ValueBinding;
use calling_conventions::CallPlan;
use semantic_vocabulary::{BlockId, EdgeId, ScalarType, ValueId};
use terminal_psi::{StructuralTypeDeclaration, TerminalAffineCleanupAction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetUnitGraph {
    pub structural_types: Vec<StructuralTypeDeclaration>,
    pub call_plan: CallPlan,
    pub scalar_parameters: Vec<ScalarAbiValue>,
    pub parameters: Vec<TargetStructuralParameter>,
    pub entry: BlockId,
    pub blocks: Vec<TargetUnitBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetUnitBlock {
    pub block: BlockId,
    pub parameters: Vec<TargetScalarBlockParameter>,
    pub structural_parameters: Vec<terminal_psi::StructuralParameterDeclaration>,
    /// Nonterminal operations only; control is owned by the terminator.
    pub operations: Vec<TargetUnitOperation>,
    pub terminator: TargetUnitTerminator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetScalarBlockParameter {
    pub value: ValueId,
    pub scalar_type: ScalarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetUnitTerminator {
    Return {
        psi_edge: EdgeId,
        cleanup_actions: Vec<TerminalAffineCleanupAction>,
    },
    Jump {
        successor: TargetUnitSuccessor,
    },
    Conditional {
        condition_source: ValueId,
        condition: TargetBooleanExpression,
        when_true: TargetUnitSuccessor,
        when_false: TargetUnitSuccessor,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetUnitSuccessor {
    pub psi_edge: EdgeId,
    pub target: BlockId,
    pub bindings: Vec<ValueBinding>,
    pub structural_bindings: Vec<abstract_operations::AbstractStructuralBinding>,
    pub cleanup_actions: Vec<TerminalAffineCleanupAction>,
}
