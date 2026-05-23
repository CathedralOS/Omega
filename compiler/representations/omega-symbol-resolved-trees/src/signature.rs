use crate::name::DiagnosticName;
use crate::types::TypeReference;
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateSignature {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub storage: StateSignatureStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateSignatureStorage {
    pub parameters: HandleSpan<StateParameter>,
    pub return_type: Option<TypeReference>,
    pub effects: HandleSpan<DiagnosticName>,
    pub contracts: HandleSpan<SignatureContract>,
}

impl Deref for StateSignature {
    type Target = StateSignatureStorage;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl DerefMut for StateSignature {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateParameter {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub type_reference: TypeReference,
    pub is_const: bool,
    pub is_mutable: bool,
    pub is_self: bool,
}

impl Default for StateParameter {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::default(),
            type_reference: TypeReference::Unit,
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
