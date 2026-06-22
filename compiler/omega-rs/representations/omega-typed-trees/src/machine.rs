use crate::expression::ExpressionHandle;
use crate::name::Identifier;
use crate::signature::SignatureContract;
use crate::state::State;
use crate::types::TypeReferenceHandle;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub attached_data: Option<Identifier>,
    pub abi: Option<String>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub contains: HandleSpan<ContainedObject>,
    pub owned_data: HandleSpan<OwnedData>,
    pub satisfies: HandleSpan<TraitConformance>,
    pub terminates: bool,
    pub decreases: HandleSpan<ExpressionHandle>,
    pub decrease_order: HandleSpan<Identifier>,
    pub effects: HandleSpan<Identifier>,
    pub contracts: HandleSpan<SignatureContract>,
    pub states: HandleSpan<State>,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            attached_data: None,
            abi: None,
            type_parameters: HandleSpan::empty(),
            contains: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            satisfies: HandleSpan::empty(),
            terminates: false,
            decreases: HandleSpan::empty(),
            decrease_order: HandleSpan::empty(),
            effects: HandleSpan::empty(),
            contracts: HandleSpan::empty(),
            states: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainedObject {
    pub symbol: SymbolHandle,
    pub type_symbol: SymbolHandle,
    pub name: Identifier,
    pub type_name: Identifier,
}

impl Default for ContainedObject {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            type_symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            type_name: Identifier::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedData {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
    pub initial_value: ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitConformance {
    pub symbol: SymbolHandle,
    pub name: Identifier,
}

impl Default for TraitConformance {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
        }
    }
}

impl Default for OwnedData {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            type_reference: TypeReferenceHandle::invalid(),
            initial_value: ExpressionHandle::invalid(),
        }
    }
}
