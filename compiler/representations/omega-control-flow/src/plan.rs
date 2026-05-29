use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::ExpressionTable;
use omega_typed_trees::name::Identifier;

use crate::{
    ContainedFlow, InvariantFact, MachineFlow, MachineOwnedDataFlow, Operation,
    ProofObligationFact, StateBorrowActivation, StateBorrowArgumentAccess, StateBorrowCall,
    StateBorrowLoan, StateBorrowWeakening, StateBorrowWritableRoot, StateContractCall,
    StateContractExit, StateContractFactRef, StateDropEvent, StateFlow, StateKey, StateMoveEvent,
    StateParameterFlow, StateValueFact, TransitionFlow,
};

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
    pub contract_fact_refs: Arena<StateContractFactRef>,
    pub contract_calls: Arena<StateContractCall>,
    pub contract_exits: Arena<StateContractExit>,
    pub values: Arena<StateValueFact>,
    pub borrow_writable_roots: Arena<StateBorrowWritableRoot>,
    pub borrow_access_segments: Arena<omega_facts::PlaceSegment>,
    pub borrow_argument_accesses: Arena<StateBorrowArgumentAccess>,
    pub borrow_calls: Arena<StateBorrowCall>,
    pub borrow_loans: Arena<StateBorrowLoan>,
    pub borrow_activations: Arena<StateBorrowActivation>,
    pub borrow_weakenings: Arena<StateBorrowWeakening>,
    pub ownership_segments: Arena<omega_facts::PlaceSegment>,
    pub move_events: Arena<StateMoveEvent>,
    pub drop_events: Arena<StateDropEvent>,
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
    ) -> Option<&Identifier> {
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
    ) -> Option<&Identifier> {
        let state = self.state_by_key(source_key)?;
        self.operations
            .span(state.operations)?
            .iter()
            .find_map(|operation| {
                if operation.statement_index != statement_index {
                    return None;
                }

                match &operation.kind {
                    crate::OperationKind::Call {
                        has_receiver: true,
                        receiver,
                        ..
                    } => Some(receiver),
                    _ => None,
                }
            })
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
