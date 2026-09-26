use crate::symbol_resolved_trees::name::DiagnosticName;
use crate::symbol_resolved_trees::types::TypeReference;
use arena::HandleSpan;
use language_core::operator_spelling::OperatorSpelling;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperatorDefinition {
    pub is_public: bool,
    pub is_boundary: bool,
    pub symbol: SymbolHandle,
    pub name: HandleSpan<DiagnosticName>,
    pub lifetime_parameters: Vec<DiagnosticName>,
    pub type_parameters: HandleSpan<crate::symbol_resolved_trees::data::TypeParameter>,
    pub parameters: HandleSpan<crate::symbol_resolved_trees::signature::StateParameter>,
    pub return_type: Option<TypeReference>,
    pub contracts: HandleSpan<crate::symbol_resolved_trees::signature::SignatureContract>,
    /// Optional `spelling` clause carried from syntax (Wave 0 decision #3).
    pub spelling: Option<OperatorSpelling>,
    pub token_count: usize,
}
