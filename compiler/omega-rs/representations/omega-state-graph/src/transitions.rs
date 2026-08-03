use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::name::Identifier;

use crate::StateKey;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionEdge {
    pub statement_index: usize,
    pub target: PlannedTransitionTarget,
    pub continuation: PlannedTransitionTarget,
    pub expressions: TransitionExpressionRefs,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TransitionExpressionRefs {
    pub target_arguments: HandleSpan<ExpressionHandle>,
    pub target_value: ExpressionHandle,
    pub continuation_arguments: HandleSpan<ExpressionHandle>,
    pub continuation_value: ExpressionHandle,
    pub guard: ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedTransitionTarget {
    None,
    State {
        index: usize,
        key: StateKey,
        name: Identifier,
    },
    Nested {
        receiver_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        receiver: Identifier,
        state: Identifier,
    },
    SelfTarget,
    Terminal,
}

impl Default for TransitionEdge {
    fn default() -> Self {
        Self {
            statement_index: 0,
            target: PlannedTransitionTarget::Terminal,
            continuation: PlannedTransitionTarget::None,
            expressions: TransitionExpressionRefs::default(),
        }
    }
}
