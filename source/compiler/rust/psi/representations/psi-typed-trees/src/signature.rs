use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;

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

/// Exact semantic target selected by one authored `invokes` occurrence.
/// Spelling is never used to reselect this target after typed lowering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthoredInvocationTarget {
    Unresolved,
    Parameter { ordinal: u32, symbol: SymbolHandle },
    Service(SymbolHandle),
}

/// One source-backed synchronous-invocation declaration. The source span and
/// exact target travel as one compiler-owned record so review provenance can
/// never be paired with a target by position or spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredInvocation {
    pub name: Identifier,
    pub source_span: psi_source::SourceSpan,
    pub target: AuthoredInvocationTarget,
}

impl Default for AuthoredInvocation {
    fn default() -> Self {
        Self {
            name: Identifier::default(),
            source_span: psi_source::SourceSpan::default(),
            target: AuthoredInvocationTarget::Unresolved,
        }
    }
}

impl AuthoredInvocation {
    pub fn as_str(&self) -> &str {
        self.name.as_str()
    }
}

impl std::fmt::Display for AuthoredInvocation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.name.fmt(formatter)
    }
}

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
    pub invokes: HandleSpan<AuthoredInvocation>,
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
