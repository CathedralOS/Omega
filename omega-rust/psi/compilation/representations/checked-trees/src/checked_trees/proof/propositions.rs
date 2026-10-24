use symbols::SymbolHandle;
use typed_trees::type_identity::TypeIdentityId;

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
    Const { type_identity: TypeIdentityId },
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
    pub arguments: Vec<typed_trees::type_identity::TypeIdentityId>,
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
    pub diagnostic_label: String,
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

// Source production and exact source-requirement replay share one pure
// normalized-to-checked conversion. These constructors make no proof decision.
impl CheckedPropositionApplication {
    pub fn from_normalized(
    normalized: typed_trees::proposition::NormalizedPropositionApplicationIdentity,
) -> Self {
    let evidence_interface = match &normalized.classification {
        typed_trees::proposition::PropositionEvidenceClassification::FactOnly => None,
        typed_trees::proposition::PropositionEvidenceClassification::Witness {
            interface, ..
        } => interface.as_ref().map(CheckedEvidenceInterfaceIdentity::from_normalized),
    };
    CheckedPropositionApplication {
        declaration: normalized.declaration,
        binder_arguments: normalized
            .binder_arguments
            .into_iter()
            .map(|argument| CheckedPropositionBinderArgument {
                kind: match argument.kind {
                    typed_trees::proposition::PropositionBinderArgumentKind::Type => {
                        CheckedPropositionBinderArgumentKind::Type
                    }
                    typed_trees::proposition::PropositionBinderArgumentKind::Const => {
                        CheckedPropositionBinderArgumentKind::Const
                    }
                    typed_trees::proposition::PropositionBinderArgumentKind::Machine => {
                        CheckedPropositionBinderArgumentKind::Machine
                    }
                },
                diagnostic_label: argument.identity,
                evidence_projection: None,
            })
            .collect(),
        arguments: normalized.arguments,
        evidence_interface,
    }
}


}
impl CheckedEvidenceInterfaceIdentity {
    pub fn from_normalized(
    interface: &typed_trees::proposition::NormalizedEvidenceInterfaceIdentity,
) -> Self {
    CheckedEvidenceInterfaceIdentity {
        trait_symbol: interface.trait_symbol,
        arguments: interface.arguments.clone(),
        requirements: interface
            .requirements
            .iter()
            .map(
                |requirement| CheckedEvidenceRequirementIdentity {
                    declaring_trait: requirement.declaring_trait,
                    declaring_trait_arguments: requirement.declaring_trait_arguments.clone(),
                    requirement: requirement.requirement,
                },
            )
            .collect(),
    }
}


}
