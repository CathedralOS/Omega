use crate::name::DiagnosticName;
use crate::types::TypeReference;
use omega_core::arena::HandleSpan;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperatorDefinition {
    pub name: HandleSpan<DiagnosticName>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub parameters: HandleSpan<crate::signature::StateParameter>,
    pub return_type: Option<TypeReference>,
    pub contracts: HandleSpan<crate::signature::SignatureContract>,
    pub is_intrinsic: bool,
    pub token_count: usize,
}
