use super::{
    RuntimeBranchPreludeBinding, RuntimeBranchPreludeExpansion, RuntimeBranchPreludeOperation,
    RuntimeBranchingCall, RuntimeBranchingCallEdge, RuntimeLeafBranchBinding,
    RuntimeLeafBranchExpansion, RuntimeLeafBranchOperation, RuntimeStraightLineBranchBinding,
    RuntimeStraightLineBranchExpansion, RuntimeStraightLineBranchOperation,
};
use omega_checked_trees::expression::{ExpressionHandle, ExpressionTable};
use omega_core::arena::Arena;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeBranchingCallPlan {
    pub expressions: ExpressionTable,
    pub calls: Arena<RuntimeBranchingCall>,
    pub edges: Arena<RuntimeBranchingCallEdge>,
    pub target_arguments: Arena<ExpressionHandle>,
    pub prelude_expansions: Arena<RuntimeBranchPreludeExpansion>,
    pub prelude_operations: Arena<RuntimeBranchPreludeOperation>,
    pub prelude_bindings: Arena<RuntimeBranchPreludeBinding>,
    pub leaf_expansions: Arena<RuntimeLeafBranchExpansion>,
    pub leaf_operations: Arena<RuntimeLeafBranchOperation>,
    pub leaf_bindings: Arena<RuntimeLeafBranchBinding>,
    pub straight_line_expansions: Arena<RuntimeStraightLineBranchExpansion>,
    pub straight_line_operations: Arena<RuntimeStraightLineBranchOperation>,
    pub straight_line_bindings: Arena<RuntimeStraightLineBranchBinding>,
}
