use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionTable};
use omega_typed_trees::name::ProgramName;
use omega_typed_trees::types::TypeReferenceHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlFlowPlan {
    pub expressions: ExpressionTable,
    pub machines: Arena<MachineFlow>,
    pub contained_machines: Arena<ContainedFlow>,
    pub machine_owned_data: Arena<MachineOwnedDataFlow>,
    pub states: Arena<StateFlow>,
    pub state_parameters: Arena<StateParameterFlow>,
    pub proof_obligations: Arena<ProofObligationFact>,
    pub invariants: Arena<InvariantFact>,
    pub borrow_writable_roots: Arena<StateBorrowWritableRoot>,
    pub borrow_argument_accesses: Arena<StateBorrowArgumentAccess>,
    pub borrow_calls: Arena<StateBorrowCall>,
    pub operations: Arena<Operation>,
    pub transitions: Arena<TransitionFlow>,
}

impl ControlFlowPlan {
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

    pub fn machine_by_symbol(&self, machine_symbol: SymbolHandle) -> Option<&MachineFlow> {
        self.machines
            .iter()
            .find(|(_, machine)| machine.symbol == machine_symbol)
            .map(|(_, machine)| machine)
    }

    pub fn machine_contains(&self, machine: &MachineFlow) -> &[ContainedFlow] {
        self.contained_machines.span_or_empty(machine.contains)
    }

    pub fn machine_owned_data(&self, machine: &MachineFlow) -> &[MachineOwnedDataFlow] {
        self.machine_owned_data.span_or_empty(machine.owned_data)
    }

    pub fn machine_owned_data_by_symbol(
        &self,
        machine_symbol: SymbolHandle,
        data_symbol: SymbolHandle,
    ) -> Option<&MachineOwnedDataFlow> {
        let machine = self.machine_by_symbol(machine_symbol)?;
        self.machine_owned_data(machine)
            .iter()
            .find(|data| data.symbol == data_symbol)
    }

    pub fn state_by_key(&self, key: StateKey) -> Option<&StateFlow> {
        let machine = self.machine_by_symbol(key.machine)?;

        self.states
            .span(machine.states)?
            .iter()
            .find(|state| state.key == key)
    }

    pub fn state_parameters(&self, state: &StateFlow) -> &[StateParameterFlow] {
        self.state_parameters.span_or_empty(state.parameters)
    }

    pub fn receiver_name_by_symbol(
        &self,
        source_key: StateKey,
        receiver_symbol: SymbolHandle,
    ) -> Option<&ProgramName> {
        if !receiver_symbol.is_valid() {
            return None;
        }

        let machine = self.machine_by_symbol(source_key.machine)?;
        if receiver_symbol == machine.symbol {
            return Some(&machine.name);
        }

        if let Some(contained) = self
            .machine_contains(machine)
            .iter()
            .find(|contained| contained.symbol == receiver_symbol)
        {
            return Some(&contained.name);
        }

        if let Some(owned_data) = self
            .machine_owned_data(machine)
            .iter()
            .find(|owned_data| owned_data.symbol == receiver_symbol)
        {
            return Some(&owned_data.name);
        }

        self.state_by_key(source_key)
            .and_then(|state| {
                self.state_parameters(state)
                    .iter()
                    .find(|parameter| parameter.symbol == receiver_symbol)
            })
            .map(|parameter| &parameter.name)
    }

    pub fn call_receiver_name_by_statement(
        &self,
        source_key: StateKey,
        statement_index: usize,
    ) -> Option<&ProgramName> {
        let state = self.state_by_key(source_key)?;
        self.operations
            .span(state.operations)?
            .iter()
            .find_map(|operation| {
                if operation.statement_index != statement_index {
                    return None;
                }

                match &operation.kind {
                    OperationKind::Call {
                        has_receiver: true,
                        receiver,
                        ..
                    } => Some(receiver),
                    _ => None,
                }
            })
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
pub struct MachineFlow {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub contains: HandleSpan<ContainedFlow>,
    pub owned_data: HandleSpan<MachineOwnedDataFlow>,
    pub states: HandleSpan<StateFlow>,
}

impl Default for MachineFlow {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::default(),
            contains: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MachineOwnedDataFlow {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub type_reference: TypeReferenceHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateFlow {
    pub key: StateKey,
    pub name: ProgramName,
    pub index: usize,
    pub parameters: HandleSpan<StateParameterFlow>,
    pub borrow: StateBorrowSummary,
    pub operations: HandleSpan<Operation>,
    pub transitions: HandleSpan<TransitionFlow>,
}

impl Default for StateFlow {
    fn default() -> Self {
        Self {
            key: StateKey::default(),
            name: ProgramName::default(),
            index: 0,
            parameters: HandleSpan::empty(),
            borrow: StateBorrowSummary::default(),
            operations: HandleSpan::empty(),
            transitions: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProofFactKind {
    #[default]
    BoundedAssignment,
    BoundedCallArgument,
    BoundedInitializer,
    BoundedStateReturn,
    BoundedValue,
    BoundedTransitionArgument,
    GuardedTransition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofObligationFact {
    pub kind: ProofFactKind,
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub owner: ProofObligationOwner,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ProofObligationOwner {
    #[default]
    Unknown,
    MachineState {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    MachineOwnedData {
        machine_symbol: SymbolHandle,
        data_symbol: SymbolHandle,
    },
    StateParameter {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        parameter_symbol: SymbolHandle,
    },
    StateReturn {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
    },
    CallParameter {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        target_symbol: SymbolHandle,
        parameter_symbol: SymbolHandle,
    },
    TransitionParameter {
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        parameter_symbol: SymbolHandle,
    },
}

impl Default for ProofObligationFact {
    fn default() -> Self {
        Self {
            kind: ProofFactKind::default(),
            machine_symbol: SymbolHandle::invalid(),
            state_symbol: SymbolHandle::invalid(),
            owner: ProofObligationOwner::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvariantFact {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub constraint_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum StateBorrowRootKind {
    #[default]
    OwnedData,
    LocalData,
    MutableParameter,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowWritableRoot {
    pub symbol: SymbolHandle,
    pub kind: StateBorrowRootKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum StateBorrowAccessKind {
    #[default]
    Read,
    Mutable,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowArgumentAccess {
    pub root_symbol: SymbolHandle,
    pub kind: StateBorrowAccessKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowCall {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub has_receiver: bool,
    pub accesses: HandleSpan<StateBorrowArgumentAccess>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowSummary {
    pub writable_roots: HandleSpan<StateBorrowWritableRoot>,
    pub mutable_parameter_count: usize,
    pub calls: HandleSpan<StateBorrowCall>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateParameterFlow {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub type_reference: TypeReferenceHandle,
    pub type_symbol: SymbolHandle,
    pub type_name: ProgramName,
    pub is_mutable_reference: bool,
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
        has_receiver: bool,
        receiver: ProgramName,
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
pub struct TransitionFlow {
    pub statement_index: usize,
    pub target: PlannedTransitionTarget,
    pub continuation: PlannedTransitionTarget,
    pub expressions: TransitionExpressionRefs,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TransitionExpressionRefs {
    pub target_arguments: HandleSpan<ExpressionHandle>,
    pub target_value: ExpressionHandle,
    pub continuation_arguments: HandleSpan<ExpressionHandle>,
    pub continuation_value: ExpressionHandle,
    pub guard: ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedTransitionTarget {
    None,
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

impl Default for TransitionFlow {
    fn default() -> Self {
        Self {
            statement_index: 0,
            target: PlannedTransitionTarget::Terminal,
            continuation: PlannedTransitionTarget::None,
            expressions: TransitionExpressionRefs::default(),
        }
    }
}
