pub use typed_trees::statement::*;

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
