use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperatorDefinition {
    pub is_boundary: bool,
    pub symbol: SymbolHandle,
    pub name: HandleSpan<crate::name::Identifier>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub parameters: HandleSpan<crate::signature::StateParameter>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub contracts: HandleSpan<crate::signature::SignatureContract>,
    pub token_count: usize,
}
