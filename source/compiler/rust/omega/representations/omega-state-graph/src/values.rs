use psi_arena::{Arena, Handle, HandleSpan};
use psi_numerics::arithmetic::ArithmeticPolicyAdapter;
use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::ExpressionHandle;

pub type StateValueHandle = Handle<StateValueFact>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraphValueRoots {
    pub values: Arena<StateValueFact>,
}

impl StateGraphValueRoots {
    pub fn with_roots(values: Arena<StateValueFact>) -> Self {
        Self { values }
    }
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
    /// Present only when checked operator resolution produced explicit
    /// adapter evidence for this expression. `Some(None)` means the operator
    /// was checked and selected no result adapter; `None` means this is an
    /// ordinary value fact rather than operator-adapter evidence.
    pub arithmetic_policy_adapter: Option<ArithmeticPolicyAdapter>,
    /// Selected boundary-operator ProviderPlan identity, when this operation
    /// has left the bootstrap lowering path.
    pub operator_provider_plan_identity: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateValueSummary {
    pub values: HandleSpan<StateValueFact>,
}
