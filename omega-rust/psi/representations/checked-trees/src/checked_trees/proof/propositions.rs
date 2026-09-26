use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedPropositionVocabulary {
    pub declarations: Vec<CheckedPropositionDeclaration>,
    pub applications: Vec<CheckedPropositionApplication>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPropositionDeclaration {
    pub symbol: SymbolHandle,
    pub name: String,
    pub is_public: bool,
    pub binders: Vec<CheckedPropositionBinder>,
    pub parameter_types: Vec<String>,
    pub evidence: CheckedPropositionEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPropositionBinder {
    pub name: String,
    pub kind: CheckedPropositionBinderKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedPropositionBinderKind {
    Type,
    Const { type_identity: String },
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedPropositionEvidence {
    FactOnly,
    Witness { evidence_type: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedPropositionApplication {
    pub declaration: SymbolHandle,
    pub binder_arguments: Vec<CheckedPropositionBinderArgument>,
    pub arguments: Vec<String>,
    /// Exact instantiated carrierless interface for witness-bearing
    /// applications. Fact-only applications carry no interface.
    pub evidence_interface: Option<CheckedEvidenceInterfaceIdentity>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedEvidenceInterfaceIdentity {
    pub trait_symbol: SymbolHandle,
    pub arguments: Vec<String>,
    pub requirements: Vec<CheckedEvidenceRequirementIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedEvidenceRequirementIdentity {
    pub declaring_trait: SymbolHandle,
    pub declaring_trait_arguments: Vec<String>,
    pub requirement: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPropositionBinderArgument {
    pub kind: CheckedPropositionBinderArgumentKind,
    /// Diagnostic rendering for ordinary static arguments. Projection
    /// identity is carried structurally below and never derives from this
    /// spelling.
    pub identity: String,
    pub evidence_projection: Option<CheckedEvidenceProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedEvidenceProjection {
    pub term: arena::Handle<crate::CheckedEvidenceTerm>,
    pub declaring_trait: SymbolHandle,
    pub declaring_trait_arguments: Vec<String>,
    pub requirement: SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedPropositionBinderArgumentKind {
    Type,
    Const,
    Machine,
}
