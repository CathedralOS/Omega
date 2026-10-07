use super::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperatorDefinition {
    pub is_public: bool,
    pub is_boundary: bool,
    pub name: HandleSpan<Identifier>,
    /// Erased borrow-region parameters declared in the shared `<>` list.
    pub lifetime_parameters: Vec<Identifier>,
    pub type_parameters: HandleSpan<TypeParameter>,
    pub parameters: HandleSpan<StateParameterHandle>,
    pub return_type: crate::types::TypeReferenceHandle,
    pub contracts: HandleSpan<CapabilityContract>,
    /// Optional compiler-owned fixed token from the declaration head.
    pub spelling: Option<OperatorSpelling>,
    pub token_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDefinition {
    pub name: Identifier,
    pub members: HandleSpan<CapabilityMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityMember {
    Field(CapabilityField),
    State(CapabilityState),
}

impl Default for CapabilityMember {
    fn default() -> Self {
        Self::Field(CapabilityField::default())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapabilityField {
    pub name: Identifier,
    pub type_reference: crate::types::TypeReferenceHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityState {
    pub signature: StateSignature,
    pub contracts: HandleSpan<CapabilityContract>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityContract {
    pub kind: CapabilityContractKind,
    /// Exact authored `requires`, `ensures`, or `crashes` keyword anchor.
    /// Generated contracts have no authored clause keyword.
    pub keyword_source_span: Option<source::SourceSpan>,
    /// An explicit erased evidence-term binding (`requires proof: P` or
    /// `ensures proof: P`). Empty clauses remain ordinary ambient facts.
    pub binding: Option<Identifier>,
    pub facts: HandleSpan<ProofFact>,
    pub token_count: usize,
}

impl Default for CapabilityContract {
    fn default() -> Self {
        Self {
            kind: CapabilityContractKind::default(),
            keyword_source_span: None,
            binding: None,
            facts: HandleSpan::empty(),
            token_count: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CapabilityContractKind {
    Ensures,
    /// Outcome-specific guarantee row. The path is authored syntax only;
    /// resolution must bind it exclusively against the machine's declared
    /// result sum. Rows share no source-level group or package identity.
    EnsuresForResultCase {
        result_case: HandleSpan<Identifier>,
    },
    #[default]
    Requires,
    Crashes {
        cause: CrashCause,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashCause {
    Trap,
    Abort,
}
