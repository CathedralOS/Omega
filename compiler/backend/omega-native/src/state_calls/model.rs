use crate::control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateCallPlan {
    pub calls: Arena<StateCall>,
    pub arguments: Arena<StateCallArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateCall {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub receiver: ProgramName,
    pub target_key: StateKey,
    pub target_state: ProgramName,
    pub argument_count: usize,
    pub arguments: HandleSpan<StateCallArgument>,
    pub reachable: bool,
    pub required: bool,
    pub resolution: StateCallResolution,
    pub lowering: StateCallLowering,
}

impl Default for StateCall {
    fn default() -> Self {
        Self {
            source_key: StateKey::default(),
            statement_index: 0,
            receiver: ProgramName::default(),
            target_key: StateKey::default(),
            target_state: ProgramName::default(),
            argument_count: 0,
            arguments: HandleSpan::empty(),
            reachable: false,
            required: false,
            resolution: StateCallResolution::Unresolved,
            lowering: StateCallLowering::Unresolved,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateCallArgument {
    pub index: usize,
    pub parameter_name: ProgramName,
    pub expression: Expression,
    pub kind: StateCallArgumentKind,
    pub required: bool,
}

impl Default for StateCallArgument {
    fn default() -> Self {
        Self {
            index: 0,
            parameter_name: ProgramName::default(),
            expression: Expression::Integer(0),
            kind: StateCallArgumentKind::Value,
            required: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateCallArgumentKind {
    #[default]
    Value,
    MutableAlias,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StateCallResolution {
    Local,
    ContainedMachine,
    NamedMachine,
    #[default]
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateCallLowering {
    InlineLeaf,
    InlineBranching,
    InlineExpansion,
    #[default]
    Unresolved,
}
