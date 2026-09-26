pub use typed_trees::statement::{
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
    When(crate::expression::Expression),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionTarget {
    Named {
        path: crate::expression::NamePath,
        head_symbol: SymbolHandle,
        symbol: SymbolHandle,
        arguments: HandleSpan<crate::expression::Expression>,
    },
    Value(crate::expression::Expression),
    SelfTarget,
    Terminal,
}
