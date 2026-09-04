pub use psi_typed_trees::statement::*;

use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

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
