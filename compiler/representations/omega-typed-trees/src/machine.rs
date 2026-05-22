use crate::expression::ExpressionHandle;
use crate::name::ProgramName;
use crate::state::State;
use crate::types::TypeReferenceHandle;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub attached_data: Option<ProgramName>,
    pub contains: HandleSpan<ContainedObject>,
    pub owned_data: HandleSpan<OwnedData>,
    pub satisfies: HandleSpan<TraitConformance>,
    pub effects: HandleSpan<ProgramName>,
    pub states: HandleSpan<State>,
}

impl Default for Machine {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::default(),
            attached_data: None,
            contains: HandleSpan::empty(),
            owned_data: HandleSpan::empty(),
            satisfies: HandleSpan::empty(),
            effects: HandleSpan::empty(),
            states: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainedObject {
    pub symbol: SymbolHandle,
    pub type_symbol: SymbolHandle,
    pub name: ProgramName,
    pub type_name: ProgramName,
}

impl Default for ContainedObject {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            type_symbol: SymbolHandle::invalid(),
            name: ProgramName::default(),
            type_name: ProgramName::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedData {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub type_reference: TypeReferenceHandle,
    pub initial_value: ExpressionHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitConformance {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
}

impl Default for TraitConformance {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::default(),
        }
    }
}

impl Default for OwnedData {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::default(),
            type_reference: TypeReferenceHandle::invalid(),
            initial_value: ExpressionHandle::invalid(),
        }
    }
}
