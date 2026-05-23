use crate::name::ProgramName;
use crate::types::TypeReferenceHandle;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSignature {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub parameters: HandleSpan<StateParameter>,
    pub return_type: TypeReferenceHandle,
    pub effects: HandleSpan<ProgramName>,
    pub contracts: HandleSpan<SignatureContract>,
}

impl Default for StateSignature {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::default(),
            parameters: HandleSpan::empty(),
            return_type: TypeReferenceHandle::invalid(),
            effects: HandleSpan::empty(),
            contracts: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateParameter {
    pub symbol: SymbolHandle,
    pub name: ProgramName,
    pub type_reference: TypeReferenceHandle,
    pub is_const: bool,
    pub is_mutable: bool,
    pub is_self: bool,
}

impl Default for StateParameter {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::default(),
            type_reference: TypeReferenceHandle::invalid(),
            is_const: false,
            is_mutable: false,
            is_self: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SignatureContract {
    pub kind: SignatureContractKind,
    pub facts: omega_core::arena::HandleSpan<crate::domain::ProofFact>,
    pub token_count: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SignatureContractKind {
    #[default]
    Requires,
    Ensures,
    Trusted,
}
