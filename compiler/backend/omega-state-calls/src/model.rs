use omega_checked_trees::expression::{ExpressionHandle, ExpressionTable};
use omega_checked_trees::name::Identifier;
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateCallPlan {
    pub expressions: ExpressionTable,
    pub calls: Arena<StateCall>,
    pub arguments: Arena<StateCallArgument>,
    pub required_states: Arena<StateKey>,
}

impl StateCallPlan {
    pub fn with_capacity(
        call_capacity: usize,
        argument_capacity: usize,
        required_state_capacity: usize,
    ) -> Self {
        Self {
            expressions: ExpressionTable::with_expression_capacity(
                call_capacity.saturating_add(argument_capacity),
            ),
            calls: Arena::with_capacity(call_capacity),
            arguments: Arena::with_capacity(argument_capacity),
            required_states: Arena::with_capacity(required_state_capacity),
        }
    }

    fn source_matches(expected: StateKey, actual: StateKey) -> bool {
        expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
    }

    pub fn calls_for_statement(
        &self,
        source_key: StateKey,
        statement_index: usize,
    ) -> impl Iterator<Item = &StateCall> {
        self.calls.iter().filter_map(move |(_, state_call)| {
            (Self::source_matches(state_call.source_key, source_key)
                && state_call.statement_index == statement_index)
                .then_some(state_call)
        })
    }

    pub fn call_for_role(
        &self,
        source_key: StateKey,
        statement_index: usize,
        role: StateCallRole,
    ) -> Option<&StateCall> {
        self.calls_for_statement(source_key, statement_index)
            .find(|state_call| state_call.role == role)
    }

    pub fn statement_call(
        &self,
        source_key: StateKey,
        statement_index: usize,
    ) -> Option<&StateCall> {
        self.call_for_role(source_key, statement_index, StateCallRole::Statement)
    }

    pub fn assignment_value_call(
        &self,
        source_key: StateKey,
        statement_index: usize,
    ) -> Option<&StateCall> {
        self.call_for_role(source_key, statement_index, StateCallRole::AssignmentValue)
    }

    pub fn assignment_value_call_by_ordinal(
        &self,
        source_key: StateKey,
        statement_index: usize,
        call_ordinal: usize,
    ) -> Option<&StateCall> {
        self.calls_for_statement(source_key, statement_index)
            .find(|state_call| {
                state_call.role == StateCallRole::AssignmentValue
                    && state_call.call_ordinal == call_ordinal
            })
    }

    pub fn transition_guard_call(
        &self,
        source_key: StateKey,
        statement_index: usize,
    ) -> Option<&StateCall> {
        self.call_for_role(source_key, statement_index, StateCallRole::TransitionGuard)
    }

    pub fn transition_argument_call(
        &self,
        source_key: StateKey,
        statement_index: usize,
    ) -> Option<&StateCall> {
        self.call_for_role(
            source_key,
            statement_index,
            StateCallRole::TransitionArgument,
        )
    }

    pub fn transition_argument_call_by_ordinal(
        &self,
        source_key: StateKey,
        statement_index: usize,
        call_ordinal: usize,
    ) -> Option<&StateCall> {
        self.calls_for_statement(source_key, statement_index)
            .find(|state_call| {
                state_call.role == StateCallRole::TransitionArgument
                    && state_call.call_ordinal == call_ordinal
            })
    }

    pub fn required_source_or_target(&self, state_key: StateKey) -> bool {
        self.calls.iter().any(|(_, state_call)| {
            state_call.required
                && (state_call.source_key == state_key || state_call.target_key == state_key)
        })
    }

    pub fn required_state(&self, state_key: StateKey) -> bool {
        self.required_states
            .iter()
            .any(|(_, required_state)| *required_state == state_key)
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
    pub call_ordinal: usize,
    pub role: StateCallRole,
    pub receiver_symbol: SymbolHandle,
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
            call_ordinal: 0,
            role: StateCallRole::Statement,
            receiver_symbol: SymbolHandle::invalid(),
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
    pub parameter_name: Identifier,
    pub expression: ExpressionHandle,
    pub kind: StateCallArgumentKind,
    pub required: bool,
}

impl Default for StateCallArgument {
    fn default() -> Self {
        Self {
            index: 0,
            parameter_symbol: SymbolHandle::invalid(),
            parameter_name: Identifier::default(),
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
