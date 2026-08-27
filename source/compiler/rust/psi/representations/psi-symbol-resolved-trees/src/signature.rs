use crate::name::DiagnosticName;
use crate::types::TypeReference;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;
use std::ops::{Deref, DerefMut};

/// One exact authored member of a service-reach ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthoredServiceReachTarget {
    pub service: SymbolHandle,
    pub source_span: psi_source::SourceSpan,
}

/// Provenance-only source custody for one callable's authored `reaches`
/// clauses. Clause keyword occurrences preserve an explicit empty ceiling;
/// targets bind exact boundary-trait identity to exact member spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredServiceReachRow {
    pub owner: SymbolHandle,
    pub keyword_source_spans: Vec<psi_source::SourceSpan>,
    pub targets: Vec<AuthoredServiceReachTarget>,
    pub installation_bound: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateSignature {
    pub symbol: SymbolHandle,
    pub name: DiagnosticName,
    pub storage: StateSignatureStorage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateSignatureStorage {
    /// Fixed token owned by a trait requirement. Structural machine-parameter
    /// signatures and ordinary state signatures leave this empty.
    pub spelling: Option<psi_language_core::OperatorSpelling>,
    pub lifetime_parameters: Vec<DiagnosticName>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub is_default: bool,
    pub parameters: HandleSpan<StateParameter>,
    pub return_type: Option<TypeReference>,
    pub invokes: HandleSpan<DiagnosticName>,
    /// EFX: normalized symbol-resolved boundary-service row.
    pub service_reach_row: psi_language_semantics::ServiceReachRowId,
    /// The published row is an installation-selected upper bound rather than
    /// a fixed callable ceiling.
    pub service_reach_is_installation_bound: bool,
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: HandleSpan<SignatureContract>,
    /// TPR4 (decision 23): the bodyless requirement's authored PUBLIC
    /// guarantee (bare `terminates;`); implementations inherit it at
    /// conformance. Populated at the syntax->resolved lowering, copied --
    /// never re-derived -- downstream.
    pub terminates_guarantee: bool,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SignatureContract {
    pub kind: SignatureContractKind,
    /// Exact authored clause keyword retained independently from semantic facts.
    pub keyword_source_span: Option<psi_source::SourceSpan>,
    pub binding: Option<DiagnosticName>,
    pub facts: psi_arena::HandleSpan<crate::domain::ProofFact>,
    pub token_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SignatureContractKind {
    #[default]
    Requires,
    Ensures,
    EnsuresForResultCase {
        result_data: SymbolHandle,
        result_case: SymbolHandle,
    },
    Crashes {
        cause: CrashCause,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashCause {
    Trap,
    Abort,
}
