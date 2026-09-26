pub use symbol_resolved_trees_to_typed_trees::typed_trees::statement::{
    AssemblyFactKind, OutcomeProofSelector, RootBinding, StatementHandle, StatementNode,
    StatementTable, TableAssemblyFact, TableAssignment, TableCall, TableLocalData, TableNamePath,
    TableTransition, TransitionExit, TransitionGuardNode, TransitionTargetHandle,
    TransitionTargetNode,
};

use arena::HandleSpan;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionGuard {
    Always,
    When(crate::checked_trees::expression::Expression),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionTarget {
    Named {
        path: crate::checked_trees::expression::NamePath,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
        arguments: HandleSpan<crate::checked_trees::expression::Expression>,
    },
    Value(crate::checked_trees::expression::Expression),
    SelfTarget,
    Terminal,
}
