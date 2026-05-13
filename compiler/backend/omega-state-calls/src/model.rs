use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionTable};
use omega_checked_trees::name::ProgramName;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateCallPlan {
    pub expressions: ExpressionTable,
    pub calls: Arena<StateCall>,
    pub arguments: Arena<StateCallArgument>,
}

impl StateCallPlan {
    pub fn statement_call(
        &self,
        source_key: StateKey,
        statement_index: usize,
    ) -> Option<&StateCall> {
        self.calls
            .iter()
            .find(|(_, state_call)| {
                state_call.source_key == source_key
                    && state_call.statement_index == statement_index
                    && state_call.role == StateCallRole::Statement
            })
            .map(|(_, state_call)| state_call)
    }

    pub fn required_source_or_target(&self, state_key: StateKey) -> bool {
        self.calls.iter().any(|(_, state_call)| {
            state_call.required
                && (state_call.source_key == state_key || state_call.target_key == state_key)
        })
    }

    pub fn required_source_or_statement_target(&self, state_key: StateKey) -> bool {
        self.calls.iter().any(|(_, state_call)| {
            state_call.required
                && (state_call.source_key == state_key
                    || (state_call.role == StateCallRole::Statement
                        && state_call.target_key == state_key))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateCall {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub role: StateCallRole,
    pub receiver_display: ProgramName,
    pub target_key: StateKey,
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
            role: StateCallRole::Statement,
            receiver_display: ProgramName::default(),
            target_key: StateKey::default(),
            argument_count: 0,
            arguments: HandleSpan::empty(),
            reachable: false,
            required: false,
            resolution: StateCallResolution::Unresolved,
            lowering: StateCallLowering::Unresolved,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StateCallRole {
    #[default]
    Statement,
    AssignmentValue,
    CallArgument,
    TransitionArgument,
    TransitionGuard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateCallArgument {
    pub index: usize,
    pub parameter_symbol: SymbolHandle,
    pub parameter_name: ProgramName,
    pub expression: ExpressionHandle,
    pub kind: StateCallArgumentKind,
    pub required: bool,
}

impl Default for StateCallArgument {
    fn default() -> Self {
        Self {
            index: 0,
            parameter_symbol: SymbolHandle::invalid(),
            parameter_name: ProgramName::default(),
            expression: ExpressionHandle::invalid(),
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
