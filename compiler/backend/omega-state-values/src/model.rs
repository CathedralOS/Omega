use omega_checked_trees::expression::{ExpressionHandle, ExpressionTable};
use omega_control_flow::StateKey;
use omega_core::arena::Arena;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateValuePlan {
    pub expressions: ExpressionTable,
    pub values: Arena<StateValueUse>,
}

impl StateValuePlan {
    pub(crate) fn with_value_capacity(value_capacity: usize) -> Self {
        Self {
            expressions: ExpressionTable::with_expression_capacity(value_capacity),
            values: Arena::with_capacity(value_capacity),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateValueUse {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub role: StateValueRole,
    pub kind: StateValueKind,
    pub expression: ExpressionHandle,
    pub required: bool,
}

impl Default for StateValueUse {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            role: StateValueRole::AssignmentValue,
            kind: StateValueKind::Literal,
            expression: ExpressionHandle::invalid(),
            required: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateValueRole {
    AssignmentTarget,
    #[default]
    AssignmentValue,
    CallArgument,
    TransitionArgument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateValueKind {
    Array,
    Binary,
    Literal,
    MutablePlace,
    Place,
    Struct,
    #[default]
    Unknown,
}
