use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::ExpressionHandle;

pub type StateValueHandle = Handle<StateValueFact>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraphValueRoots {
    pub values: Arena<StateValueFact>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StateValueStatementRole {
    #[default]
    Expression,
    AssignmentTargetSubexpression,
    AssignmentValue,
    CallArgument,
    LocalInitializer,
    TransitionGuard,
    TransitionTargetArgument,
    TransitionTargetValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateValueOrigin {
    Statement {
        statement_index: usize,
        role: StateValueStatementRole,
    },
}

impl Default for StateValueOrigin {
    fn default() -> Self {
        Self::Statement {
            statement_index: 0,
            role: StateValueStatementRole::Expression,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateValueFact {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub expression: ExpressionHandle,
    pub origin: StateValueOrigin,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateValueSummary {
    pub values: HandleSpan<StateValueFact>,
}
