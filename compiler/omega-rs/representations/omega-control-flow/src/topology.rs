use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::name::Identifier;
use omega_typed_trees::types::TypeReferenceHandle;

use crate::{
    Operation, StateBorrowSummary, StateBoundarySummary, StateContractSummary,
    StateOwnershipSummary, StateValueSummary, TransitionFlow,
};

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
    pub values: StateValueSummary,
    pub boundaries: StateBoundarySummary,
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
            values: StateValueSummary::default(),
            boundaries: StateBoundarySummary::default(),
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
    /// For a `dyn Trait` parameter with MULTIPLE satisfying impls: every impl's
    /// data type name (the trait's closed world), in data-definition order.
    /// A method call through this parameter resolves to one candidate per impl;
    /// the receiver's static type at each call site selects among them. Empty
    /// for non-`dyn` parameters and single-impl `dyn` (devirtualized upstream).
    pub dyn_impl_type_names: Vec<Identifier>,
}
