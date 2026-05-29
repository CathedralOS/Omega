mod borrow;
mod contracts;
mod invariants;
mod operations;
mod ownership;
mod proof;
mod transitions;

pub use borrow::*;
pub use contracts::*;
pub use invariants::*;
pub use operations::*;
pub use ownership::*;
pub use proof::*;
pub use transitions::*;

use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::ExpressionTable;
use omega_typed_trees::name::Identifier;
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
                    OperationKind::Call {
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
    pub name: Identifier,
    pub attached_data: Option<Identifier>,
    pub direct_effects: omega_effects::EffectBits,
    pub reached_effects: omega_effects::EffectBits,
    pub contains: HandleSpan<ContainedFlow>,
    pub owned_data: HandleSpan<MachineOwnedDataFlow>,
    pub states: HandleSpan<StateFlow>,
}

impl Default for MachineFlow {
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
pub struct ContainedFlow {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_symbol: SymbolHandle,
    pub type_name: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MachineOwnedDataFlow {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateFlow {
    pub key: StateKey,
    pub name: Identifier,
    pub index: usize,
    pub direct_effects: omega_effects::EffectBits,
    pub reached_effects: omega_effects::EffectBits,
    pub parameters: HandleSpan<StateParameterFlow>,
    pub contracts: StateContractSummary,
    pub borrow: StateBorrowSummary,
    pub ownership: StateOwnershipSummary,
    pub operations: HandleSpan<Operation>,
    pub transitions: HandleSpan<TransitionFlow>,
}

impl Default for StateFlow {
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
            ownership: StateOwnershipSummary::default(),
            operations: HandleSpan::empty(),
            transitions: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateParameterFlow {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
    pub type_symbol: SymbolHandle,
    pub type_name: Identifier,
    pub is_mutable_reference: bool,
}
