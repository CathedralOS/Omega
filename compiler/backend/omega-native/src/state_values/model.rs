use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_typed_program::expression::Expression;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateValuePlan {
    pub values: Arena<StateValueUse>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateValueUse {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub role: StateValueRole,
    pub kind: StateValueKind,
    pub expression: Expression,
    pub required: bool,
}

impl Default for StateValueUse {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            role: StateValueRole::AssignmentValue,
            kind: StateValueKind::Literal,
            expression: Expression::Integer(0),
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
    TransitionGuard,
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
