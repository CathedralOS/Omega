use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateSignature {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    /// Fixed token owned by a trait requirement, retained as public
    /// compatibility surface rather than used as bare dispatch identity.
    pub spelling: Option<psi_language_core::OperatorSpelling>,
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub is_default: bool,
    pub parameters: HandleSpan<StateParameter>,
    pub return_type: TypeReferenceHandle,
    pub invokes: HandleSpan<Identifier>,
    /// EFX: normalized symbol-resolved boundary-service row.
    pub service_reach_row: psi_language_semantics::ServiceReachRowId,
    pub service_reach_is_installation_bound: bool,
    pub suspends: bool,
    pub blocks: bool,
    pub contracts: HandleSpan<SignatureContract>,
    /// The bodyless requirement's normalized PUBLIC guarantee, including its
    /// exact parameter-rooted progress-premise schemas. Implementations
    /// inherit this record rather than reconstructing it from their bodies.
    pub termination_guarantee: psi_language_semantics::TerminationGuarantee,
}

impl Default for StateSignature {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            spelling: None,
            lifetime_parameters: Vec::new(),
            type_parameters: HandleSpan::empty(),
            is_default: false,
            parameters: HandleSpan::empty(),
            return_type: TypeReferenceHandle::invalid(),
            invokes: HandleSpan::empty(),
            service_reach_row: psi_language_semantics::ServiceReachRowId::NULL,
            service_reach_is_installation_bound: false,
            suspends: false,
            blocks: false,
            contracts: HandleSpan::empty(),
            termination_guarantee: psi_language_semantics::TerminationGuarantee::NoGuarantee,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateParameter {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_reference: TypeReferenceHandle,
    pub is_const: bool,
    pub is_mutable: bool,
    pub is_self: bool,
}

impl Default for StateParameter {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            type_reference: TypeReferenceHandle::invalid(),
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
    pub binding: Option<Identifier>,
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
