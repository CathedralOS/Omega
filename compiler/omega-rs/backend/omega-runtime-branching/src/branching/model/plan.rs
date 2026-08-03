use super::{
    RuntimeBranchPreludeBinding, RuntimeBranchPreludeExpansion, RuntimeBranchPreludeOperation,
    RuntimeBranchingCall, RuntimeBranchingCallEdge, RuntimeLeafBranchBinding,
    RuntimeLeafBranchExpansion, RuntimeLeafBranchOperation, RuntimeStraightLineBranchBinding,
    RuntimeStraightLineBranchExpansion, RuntimeStraightLineBranchOperation,
};
use psi_arena::Arena;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionTable};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeBranchingCallPlan {
    pub expressions: ExpressionTable,
    pub calls: Arena<RuntimeBranchingCall>,
    pub edges: Arena<RuntimeBranchingCallEdge>,
    pub target_arguments: Arena<ExpressionHandle>,
    pub target_values: Arena<ExpressionHandle>,
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

impl RuntimeBranchingCallPlan {
    pub fn with_capacity(
        call_capacity: usize,
        edge_capacity: usize,
        argument_capacity: usize,
        expansion_capacity: usize,
        binding_capacity: usize,
        operation_capacity: usize,
    ) -> Self {
        Self {
            expressions: ExpressionTable::with_expression_capacity(
                call_capacity
                    .saturating_add(argument_capacity)
                    .saturating_add(binding_capacity)
                    .saturating_add(operation_capacity),
            ),
            calls: Arena::with_capacity(call_capacity),
            edges: Arena::with_capacity(edge_capacity),
            target_arguments: Arena::with_capacity(argument_capacity),
            target_values: Arena::with_capacity(call_capacity),
            prelude_expansions: Arena::with_capacity(expansion_capacity),
            prelude_operations: Arena::with_capacity(operation_capacity),
            prelude_bindings: Arena::with_capacity(binding_capacity),
            leaf_expansions: Arena::with_capacity(expansion_capacity),
            leaf_operations: Arena::with_capacity(operation_capacity),
            leaf_bindings: Arena::with_capacity(binding_capacity),
            straight_line_expansions: Arena::with_capacity(expansion_capacity),
            straight_line_operations: Arena::with_capacity(operation_capacity),
            straight_line_bindings: Arena::with_capacity(binding_capacity),
        }
    }
}
