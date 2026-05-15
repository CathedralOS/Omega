use crate::name::DiagnosticName;
use crate::types::TypeReference;
use omega_core::symbols::SymbolHandle;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSignature {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub storage: StateSignatureStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateSignatureStorage {
    pub parameters: Vec<StateParameter>,
    pub return_type: Option<TypeReference>,
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
