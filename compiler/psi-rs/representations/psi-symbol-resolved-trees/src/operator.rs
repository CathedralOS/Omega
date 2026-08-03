use crate::name::DiagnosticName;
use crate::types::TypeReference;
use psi_arena::HandleSpan;
use psi_language_core::operator_spelling::OperatorSpelling;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperatorDefinition {
    pub is_boundary: bool,
    pub symbol: SymbolHandle,
    pub name: HandleSpan<DiagnosticName>,
    pub lifetime_parameters: Vec<DiagnosticName>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub parameters: HandleSpan<crate::signature::StateParameter>,
    pub return_type: Option<TypeReference>,
    pub contracts: HandleSpan<crate::signature::SignatureContract>,
    /// Optional `spelling` clause carried from syntax (Wave 0 decision #3).
    pub spelling: Option<OperatorSpelling>,
    pub token_count: usize,
}
