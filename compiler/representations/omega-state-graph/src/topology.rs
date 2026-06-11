use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::name::Identifier;
use omega_typed_trees::types::TypeReferenceHandle;

use crate::{
    Operation, StateBorrowSummary, StateBoundarySummary, StateContractSummary,
    StateOwnershipSummary, StateValueSummary, TransitionEdge,
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
    pub values: StateValueSummary,
    pub boundaries: StateBoundarySummary,
    pub borrow: StateBorrowSummary,
    pub ownership: StateOwnershipSummary,
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
pub struct StateParameterNode {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
    pub type_symbol: SymbolHandle,
    pub type_name: Identifier,
    pub is_mutable_reference: bool,
    /// For a `dyn Trait` parameter with MULTIPLE satisfying impls: the data
    /// type names of every impl, in data-definition order (the trait's closed
    /// world). A method call through this parameter is monomorphized over these
    /// candidates; the receiver's static type at each call site selects one.
    /// Empty for non-`dyn` parameters and for a single-impl `dyn` (which
    /// devirtualizes directly to the unique impl instead).
    pub dyn_impl_type_names: Vec<Identifier>,
}
