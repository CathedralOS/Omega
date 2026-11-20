use crate::ValueDeclaration;
use crate::terminal_module::TerminalIdentityIndex;
use semantic_vocabulary::{EvidenceTermId, MachineId, Proposition};

/// A transparent scalar predicate, not evidence that its body holds.
/// Its terms must form under these arbitrary inputs before any use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarPredicateDefinition {
    pub declaration: TerminalIdentityIndex,
    pub parameters: Vec<ValueDeclaration>,
    pub body: Proposition,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropositionDeclaration {
    pub id: semantic_vocabulary::PropositionId,
    pub name: String,
    pub binders: Vec<PropositionBinderDeclaration>,
    pub parameter_types: Vec<String>,
    pub evidence: PropositionEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropositionBinderDeclaration {
    pub name: String,
    pub kind: PropositionBinderKind,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropositionBinderKind {
    Type,
    Const {
        type_identity: TerminalIdentityIndex,
    },
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropositionEvidence {
    FactOnly,
    Witness { evidence_type: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropositionApplicationIdentity {
    pub id: semantic_vocabulary::PropositionId,
    pub declaration: semantic_vocabulary::PropositionId,
    pub binder_arguments: Vec<PropositionBinderArgumentIdentity>,
    pub arguments: Vec<String>,
    pub evidence_interface: Option<EvidenceInterfaceIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropositionBinderArgumentIdentity {
    pub kind: PropositionBinderArgumentKind,
    pub identity: TerminalIdentityIndex,
    pub evidence_projection: Option<EvidenceProjectionIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceProjectionIdentity {
    pub term: EvidenceTermId,
    pub declaring_trait_identity: TerminalIdentityIndex,
    pub declaring_trait_arguments: Vec<String>,
    pub requirement_identity: TerminalIdentityIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceTermDeclaration {
    pub id: EvidenceTermId,
    pub proposition: semantic_vocabulary::PropositionId,
    pub interface: EvidenceInterfaceIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceInterfaceIdentity {
    pub trait_identity: TerminalIdentityIndex,
    pub arguments: Vec<String>,
    pub requirements: Vec<EvidenceRequirementIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceRequirementIdentity {
    pub declaring_trait_identity: TerminalIdentityIndex,
    pub declaring_trait_arguments: Vec<String>,
    pub requirement_identity: TerminalIdentityIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceContractLaneKind {
    Requires,
    Ensures,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceArgumentSubject {
    Parameter(super::contracts::OutcomeSpecificCallArgumentSubstitution),
    Result { argument_position: u32 },
    Closed { argument_position: u32 },
    Unknown { argument_position: u32 },
}

impl EvidenceArgumentSubject {
    pub fn argument_position(&self) -> u32 {
        match self {
            Self::Parameter(parameter) => parameter.argument_position,
            Self::Result { argument_position }
            | Self::Closed { argument_position }
            | Self::Unknown { argument_position } => *argument_position,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceCallableOwner {
    Machine(MachineId),
    Runtime(semantic_vocabulary::CallableContractId),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceContractLane {
    pub owner: EvidenceCallableOwner,
    pub kind: EvidenceContractLaneKind,
    pub position: u32,
    pub term: EvidenceTermId,
    pub output_field: Option<String>,
    pub argument_subjects: Vec<EvidenceArgumentSubject>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropositionBinderArgumentKind {
    Type,
    Const,
    Machine,
}
