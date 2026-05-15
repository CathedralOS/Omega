use crate::name::ProgramName;
use crate::signature::StateSignature;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub states: Vec<StateSignature>,
}

impl Default for Platform {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::default(),
            states: Vec::new(),
        }
    }
}
