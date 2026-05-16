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
    pub contains: HandleSpan<ContainedObject>,
    pub owned_data: HandleSpan<OwnedData>,
    pub states: HandleSpan<State>,
}

impl Default for Machine {
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
