use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedPropositionVocabulary {
    pub declarations: Vec<CheckedPropositionDeclaration>,
    pub applications: Vec<CheckedPropositionApplication>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPropositionDeclaration {
    pub symbol: SymbolHandle,
    pub name: String,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPropositionApplication {
    pub declaration: SymbolHandle,
    pub binder_arguments: Vec<CheckedPropositionBinderArgument>,
    pub arguments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedPropositionBinderArgument {
    pub kind: CheckedPropositionBinderArgumentKind,
    pub identity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckedPropositionBinderArgumentKind {
    Type,
    Const,
    Machine,
}
