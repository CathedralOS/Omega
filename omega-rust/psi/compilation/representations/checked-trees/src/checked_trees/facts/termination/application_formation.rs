//! Decisions produced by one independently formed rank application. Raw
//! decisions live inside an exact admitted state row; an executable application
//! additionally binds them to its machine body and formal input telescope.

use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;
use typed_trees::{expression::ExpressionHandle, state::State};

/// One call admitted under its containing declaration application's bindings.
/// The application row, rather than this record alone, owns its runtime input.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedDeclarationCall {
    pub expression: ExpressionHandle,
    pub target_machine: SymbolHandle,
    pub target_state: SymbolHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedRankApplicationFacts {
    pub measure: SymbolHandle,
    pub definition_body: ExpressionHandle,
    pub casts: HandleSpan<crate::operators::ExactIntegerCastFact>,
    pub calls: HandleSpan<CheckedDeclarationCall>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedRankApplicationFormation {
    pub machine: SymbolHandle,
    pub machine_body: HandleSpan<State>,
    pub state: SymbolHandle,
    pub application: CheckedRankApplicationFacts,
    pub bindings: HandleSpan<super::CheckedRankCoordinateBinding>,
}

pub type CheckedRankApplicationFormationHandle = Handle<CheckedRankApplicationFormation>;
