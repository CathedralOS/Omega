use omega_control_flow::StateKey;
use omega_core::arena::{Arena, Handle};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::ExpressionHandle;

pub type AbstractValueFactHandle = Handle<AbstractValueFact>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AbstractValueStatementRole {
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
pub enum AbstractValueOrigin {
    Statement {
        statement_index: usize,
        role: AbstractValueStatementRole,
    },
}

impl Default for AbstractValueOrigin {
    fn default() -> Self {
        Self::Statement {
            statement_index: 0,
            role: AbstractValueStatementRole::Expression,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbstractValueFact {
    pub source_key: StateKey,
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub expression: ExpressionHandle,
    pub origin: AbstractValueOrigin,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbstractValueSummary {
    pub values: Arena<AbstractValueFact>,
}

impl AbstractValueSummary {
    pub fn with_capacity(value_capacity: usize) -> Self {
        Self {
            values: Arena::with_capacity(value_capacity),
        }
    }
}
