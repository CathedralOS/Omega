mod runtime_flow;

use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionTable, ExpressionTableCapacity};
use omega_typed_trees::name::Identifier;
use omega_typed_trees::types::TypeReferenceHandle;

pub use runtime_flow::{
    RuntimeCycle, RuntimeEdge, RuntimeFlowPlan, RuntimeState, RuntimeStateCallEdge,
    RuntimeTransitionTarget, build_runtime_flow_plan, build_runtime_flow_plan_with_state_calls,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateGraph {
    pub expressions: ExpressionTable,
    pub machines: Arena<MachineGraph>,
    pub contained_machines: Arena<ContainedGraph>,
    pub machine_owned_data: Arena<MachineOwnedDataGraph>,
    pub states: Arena<StateNode>,
    pub state_parameters: Arena<StateParameterNode>,
    pub proof_obligations: Arena<ProofObligationFact>,
    pub invariants: Arena<InvariantFact>,
    pub contract_fact_refs: Arena<StateContractFactRef>,
    pub contract_calls: Arena<StateContractCall>,
    pub contract_exits: Arena<StateContractExit>,
    pub borrow_writable_roots: Arena<StateBorrowWritableRoot>,
    pub borrow_access_segments: Arena<omega_facts::PlaceSegment>,
    pub borrow_argument_accesses: Arena<StateBorrowArgumentAccess>,
    pub borrow_calls: Arena<StateBorrowCall>,
    pub borrow_loans: Arena<StateBorrowLoan>,
    pub borrow_activations: Arena<StateBorrowActivation>,
    pub borrow_weakenings: Arena<StateBorrowWeakening>,
    pub operations: Arena<Operation>,
    pub transitions: Arena<TransitionEdge>,
}

impl StateGraph {
    pub fn with_capacity(
        expression_capacity: ExpressionTableCapacity,
        machine_capacity: usize,
        contained_machine_capacity: usize,
        machine_owned_data_capacity: usize,
        state_capacity: usize,
        state_parameter_capacity: usize,
        proof_obligation_capacity: usize,
        invariant_capacity: usize,
        contract_fact_ref_capacity: usize,
        contract_call_capacity: usize,
        contract_exit_capacity: usize,
        borrow_writable_root_capacity: usize,
        borrow_access_segment_capacity: usize,
        borrow_argument_access_capacity: usize,
        borrow_call_capacity: usize,
        borrow_loan_capacity: usize,
        borrow_activation_capacity: usize,
        borrow_weakening_capacity: usize,
        operation_capacity: usize,
        transition_capacity: usize,
    ) -> Self {
        Self {
            expressions: ExpressionTable::with_capacities(expression_capacity),
            machines: Arena::with_capacity(machine_capacity),
            contained_machines: Arena::with_capacity(contained_machine_capacity),
            machine_owned_data: Arena::with_capacity(machine_owned_data_capacity),
            states: Arena::with_capacity(state_capacity),
            state_parameters: Arena::with_capacity(state_parameter_capacity),
            proof_obligations: Arena::with_capacity(proof_obligation_capacity),
            invariants: Arena::with_capacity(invariant_capacity),
            contract_fact_refs: Arena::with_capacity(contract_fact_ref_capacity),
            contract_calls: Arena::with_capacity(contract_call_capacity),
            contract_exits: Arena::with_capacity(contract_exit_capacity),
            borrow_writable_roots: Arena::with_capacity(borrow_writable_root_capacity),
            borrow_access_segments: Arena::with_capacity(borrow_access_segment_capacity),
            borrow_argument_accesses: Arena::with_capacity(borrow_argument_access_capacity),
            borrow_calls: Arena::with_capacity(borrow_call_capacity),
            borrow_loans: Arena::with_capacity(borrow_loan_capacity),
            borrow_activations: Arena::with_capacity(borrow_activation_capacity),
            borrow_weakenings: Arena::with_capacity(borrow_weakening_capacity),
            operations: Arena::with_capacity(operation_capacity),
            transitions: Arena::with_capacity(transition_capacity),
        }
    }

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

    pub fn machine_contains(&self, machine: &MachineGraph) -> &[ContainedGraph] {
        self.contained_machines.span_or_empty(machine.contains)
    }

    pub fn machine_owned_data(&self, machine: &MachineGraph) -> &[MachineOwnedDataGraph] {
        self.machine_owned_data.span_or_empty(machine.owned_data)
    }

    pub fn state_by_key(&self, key: StateKey) -> Option<&StateNode> {
        let machine = self.machine_by_symbol(key.machine)?;

        self.states
            .span(machine.states)?
            .iter()
            .find(|state| state.key == key)
    }

    pub fn state_parameters(&self, state: &StateNode) -> &[StateParameterNode] {
        self.state_parameters.span_or_empty(state.parameters)
    }

    pub fn state_names_by_key(&self, key: StateKey) -> Option<(&Identifier, &Identifier)> {
        let machine = self.machine_by_symbol(key.machine)?;
        let state = self
            .states
            .span(machine.states)?
            .iter()
            .find(|state| state.key == key)?;

        Some((&machine.name, &state.name))
    }

    pub fn state_names_by_key_cloned(&self, key: StateKey) -> (Identifier, Identifier) {
        self.state_names_by_key(key)
            .map(|(machine, state)| (machine.clone(), state.clone()))
            .unwrap_or_default()
    }

    pub fn state_machine_name_by_key_cloned(&self, key: StateKey) -> Identifier {
        self.state_names_by_key(key)
            .map(|(machine, _)| machine.clone())
            .unwrap_or_default()
    }

    pub fn state_name_by_key_cloned(&self, key: StateKey) -> Identifier {
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
    pub name: Identifier,
    pub attached_data: Option<Identifier>,
    pub direct_effects: omega_effects::EffectBits,
    pub reached_effects: omega_effects::EffectBits,
    pub contains: HandleSpan<ContainedGraph>,
    pub owned_data: HandleSpan<MachineOwnedDataGraph>,
    pub states: HandleSpan<StateNode>,
}

impl Default for MachineGraph {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            attached_data: None,
            direct_effects: 0,
            reached_effects: 0,
            contains: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            states: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContainedGraph {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_symbol: SymbolHandle,
    pub type_name: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MachineOwnedDataGraph {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateNode {
    pub key: StateKey,
    pub name: Identifier,
    pub index: usize,
    pub direct_effects: omega_effects::EffectBits,
    pub reached_effects: omega_effects::EffectBits,
    pub parameters: HandleSpan<StateParameterNode>,
    pub contracts: StateContractSummary,
    pub borrow: StateBorrowSummary,
    pub operations: HandleSpan<Operation>,
    pub transitions: HandleSpan<TransitionEdge>,
}

impl Default for StateNode {
    fn default() -> Self {
        Self {
            key: StateKey::default(),
            name: Identifier::default(),
            index: 0,
            direct_effects: 0,
            reached_effects: 0,
            parameters: HandleSpan::empty(),
            contracts: StateContractSummary::default(),
            borrow: StateBorrowSummary::default(),
            operations: HandleSpan::empty(),
            transitions: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StateContractFactKind {
    #[default]
    Requires,
    Ensures,
    Trusted,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateContractFactRef {
    pub kind: StateContractFactKind,
    pub fact: omega_core::arena::Handle<omega_typed_trees::domain::ProofFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateContractCall {
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub target_machine_symbol: SymbolHandle,
    pub target_state_symbol: SymbolHandle,
    pub requires: HandleSpan<StateContractFactRef>,
    pub ensures: HandleSpan<StateContractFactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateContractExit {
    pub statement_index: usize,
    pub ensures: HandleSpan<StateContractFactRef>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateContractSummary {
    pub calls: HandleSpan<StateContractCall>,
    pub exits: HandleSpan<StateContractExit>,
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
    pub name: Identifier,
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
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
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
pub struct StateBorrowLoan {
    pub statement_index: usize,
    pub last_use_statement_index: usize,
    pub owner_symbol: SymbolHandle,
    pub root_symbol: SymbolHandle,
    pub segments: HandleSpan<omega_facts::PlaceSegment>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateBorrowEventSource {
    Statement {
        statement_index: usize,
    },
    Call {
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: SymbolHandle,
    },
}

impl Default for StateBorrowEventSource {
    fn default() -> Self {
        Self::Statement { statement_index: 0 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowActivation {
    pub source: StateBorrowEventSource,
    pub loan: Handle<StateBorrowLoan>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StateBorrowWeakeningReason {
    #[default]
    LastUseExpired,
    StateExit,
    LocalReassigned,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowWeakening {
    pub source: StateBorrowEventSource,
    pub loan: Handle<StateBorrowLoan>,
    pub reason: StateBorrowWeakeningReason,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateBorrowSummary {
    pub writable_roots: HandleSpan<StateBorrowWritableRoot>,
    pub mutable_parameter_count: usize,
    pub calls: HandleSpan<StateBorrowCall>,
    pub active_loans: HandleSpan<StateBorrowLoan>,
    pub activations: HandleSpan<StateBorrowActivation>,
    pub weakenings: HandleSpan<StateBorrowWeakening>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateParameterNode {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
    pub type_symbol: SymbolHandle,
    pub type_name: Identifier,
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
        receiver: Identifier,
        target: Identifier,
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
        name: Identifier,
    },
    Nested {
        receiver_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        receiver: Identifier,
        state: Identifier,
    },
    SelfTarget,
    Terminal,
}

impl Default for TransitionEdge {
    fn default() -> Self {
        Self {
            statement_index: 0,
            target: PlannedTransitionTarget::Terminal,
            continuation: PlannedTransitionTarget::None,
            expressions: TransitionExpressionRefs::default(),
        }
    }
}
