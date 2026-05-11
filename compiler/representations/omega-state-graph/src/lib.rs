mod runtime_flow;

use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionTable};
use omega_typed_trees::expression::NamePath;
use omega_typed_trees::name::ProgramName;
use omega_typed_trees::statement::TransitionGuard;

pub use runtime_flow::{
    RuntimeCycle, RuntimeEdge, RuntimeFlowPlan, RuntimeState, RuntimeTransitionTarget,
    build_runtime_flow_plan,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraph {
    pub expressions: ExpressionTable,
    pub machines: Arena<MachineGraph>,
    pub states: Arena<StateNode>,
    pub operations: Arena<Operation>,
    pub transitions: Arena<TransitionEdge>,
}

impl StateGraph {
    pub fn state_key_by_symbols(
        &self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    ) -> Option<StateKey> {
        let machine = self.machine_by_symbol(machine_symbol)?;

        self.states
            .span(machine.states)?
            .iter()
            .find(|state| state.key.machine == machine_symbol && state.key.state == state_symbol)
            .map(|state| state.key)
    }

    pub fn machine_by_symbol(&self, machine_symbol: SymbolHandle) -> Option<&MachineGraph> {
        self.machines
            .iter()
            .find(|(_, machine)| machine.symbol == machine_symbol)
            .map(|(_, machine)| machine)
    }

    pub fn state_by_key(&self, key: StateKey) -> Option<&StateNode> {
        let machine = self.machine_by_symbol(key.machine)?;

        self.states
            .span(machine.states)?
            .iter()
            .find(|state| state.key == key)
    }

    pub fn state_names_by_key(&self, key: StateKey) -> Option<(&ProgramName, &ProgramName)> {
        let machine = self.machine_by_symbol(key.machine)?;
        let state = self
            .states
            .span(machine.states)?
            .iter()
            .find(|state| state.key == key)?;

        Some((&machine.name, &state.name))
    }

    pub fn state_names_by_key_cloned(&self, key: StateKey) -> (ProgramName, ProgramName) {
        self.state_names_by_key(key)
            .map(|(machine, state)| (machine.clone(), state.clone()))
            .unwrap_or_default()
    }

    pub fn state_machine_name_by_key_cloned(&self, key: StateKey) -> ProgramName {
        self.state_names_by_key(key)
            .map(|(machine, _)| machine.clone())
            .unwrap_or_default()
    }

    pub fn state_name_by_key_cloned(&self, key: StateKey) -> ProgramName {
        self.state_names_by_key(key)
            .map(|(_, state)| state.clone())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StateKey {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub segment_index: usize,
}

impl StateKey {
    pub fn is_valid(self) -> bool {
        self.machine.is_valid() && self.state.is_valid()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineGraph {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub contains: Vec<ContainedGraph>,
    pub states: HandleSpan<StateNode>,
}

impl Default for MachineGraph {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::default(),
            contains: Vec::new(),
            states: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContainedGraph {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub type_symbol: SymbolHandle,
    pub type_name: ProgramName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateNode {
    pub key: StateKey,
    pub name: ProgramName,
    pub index: usize,
    pub parameters: Vec<StateParameterNode>,
    pub operations: HandleSpan<Operation>,
    pub transitions: HandleSpan<TransitionEdge>,
}

impl Default for StateNode {
    fn default() -> Self {
        Self {
            key: StateKey::default(),
            name: ProgramName::default(),
            index: 0,
            parameters: Vec::new(),
            operations: HandleSpan::empty(),
            transitions: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateParameterNode {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub statement_index: usize,
    pub kind: OperationKind,
    pub expressions: OperationExpressionRefs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationKind {
    Assignment,
    Call {
        receiver_symbol: SymbolHandle,
        target_symbol: SymbolHandle,
        receiver: Option<NamePath>,
        target: ProgramName,
    },
    ConstantIntegerAssignment,
    Expression,
    LocalData,
    StaticAssignment,
}

impl Default for Operation {
    fn default() -> Self {
        Self {
            statement_index: 0,
            kind: OperationKind::LocalData,
            expressions: OperationExpressionRefs::None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OperationExpressionRefs {
    Assignment {
        target: ExpressionHandle,
        value: ExpressionHandle,
    },
    Call {
        arguments: HandleSpan<ExpressionHandle>,
    },
    Expression(ExpressionHandle),
    #[default]
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionEdge {
    pub target: PlannedTransitionTarget,
    pub continuation: Option<PlannedTransitionTarget>,
    pub guard: TransitionGuard,
    pub expressions: TransitionExpressionRefs,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TransitionExpressionRefs {
    pub target_arguments: HandleSpan<ExpressionHandle>,
    pub target_value: Option<ExpressionHandle>,
    pub continuation_arguments: HandleSpan<ExpressionHandle>,
    pub continuation_value: Option<ExpressionHandle>,
    pub guard: Option<ExpressionHandle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedTransitionTarget {
    State {
        index: usize,
        key: StateKey,
        name: ProgramName,
    },
    Nested {
        receiver_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        receiver: ProgramName,
        state: ProgramName,
    },
    SelfTarget,
    Terminal,
}

impl Default for TransitionEdge {
    fn default() -> Self {
        Self {
            target: PlannedTransitionTarget::Terminal,
            continuation: None,
            guard: TransitionGuard::Always,
            expressions: TransitionExpressionRefs::default(),
        }
    }
}
