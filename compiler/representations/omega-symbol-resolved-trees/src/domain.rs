use crate::name::DiagnosticName;
use crate::types::TypeReference;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainDefinition {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub target_type: TypeReference,
    pub body_token_count: usize,
}
