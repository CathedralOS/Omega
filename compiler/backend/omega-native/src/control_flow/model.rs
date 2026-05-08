use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;
use omega_typed_program::statement::TransitionGuard;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowPlan {
    pub machines: Arena<MachineFlow>,
    pub states: Arena<StateFlow>,
    pub operations: Arena<Operation>,
    pub transitions: Arena<TransitionFlow>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StateKey {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub segment_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineFlow {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub contains: Vec<ContainedFlow>,
    pub states: HandleSpan<StateFlow>,
}

impl Default for MachineFlow {
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
pub struct ContainedFlow {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub type_symbol: SymbolHandle,
    pub type_name: ProgramName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateFlow {
    pub key: StateKey,
    pub name: ProgramName,
    pub index: usize,
    pub parameters: Vec<ProgramName>,
    pub operations: HandleSpan<Operation>,
    pub transitions: HandleSpan<TransitionFlow>,
}

impl Default for StateFlow {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub statement_index: usize,
    pub kind: OperationKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationKind {
    Assignment {
        target: Expression,
        value: Expression,
    },
    Call {
        receiver: Option<ProgramName>,
        target: ProgramName,
        arguments: Vec<Expression>,
    },
    ConstantIntegerAssignment,
    Expression,
    LocalData,
    StaticAssignment {
        target: Expression,
        value: Expression,
    },
}

impl Default for Operation {
    fn default() -> Self {
        Self {
            statement_index: 0,
            kind: OperationKind::LocalData,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionFlow {
    pub target: PlannedTransitionTarget,
    pub continuation: Option<PlannedTransitionTarget>,
    pub guard: TransitionGuard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedTransitionTarget {
    State {
        index: usize,
        key: StateKey,
        name: ProgramName,
        arguments: Vec<Expression>,
    },
    Nested {
        receiver: ProgramName,
        state: ProgramName,
        arguments: Vec<Expression>,
    },
    SelfTarget,
    Terminal,
}

impl Default for TransitionFlow {
    fn default() -> Self {
        Self {
            target: PlannedTransitionTarget::Terminal,
            continuation: None,
            guard: TransitionGuard::Always,
        }
    }
}
