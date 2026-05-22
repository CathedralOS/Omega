use crate::name::ProgramName;
use crate::types::TypeReferenceHandle;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainDefinition {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub target_type: TypeReferenceHandle,
    pub body_token_count: usize,
}

impl Default for DomainDefinition {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::default(),
            target_type: TypeReferenceHandle::invalid(),
            body_token_count: 0,
        }
    }
}
