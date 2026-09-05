use psi_core::{EvidenceTermId, MachineId};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropositionDeclaration {
    pub id: psi_core::PropositionId,
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
    Const { type_identity: String },
    Machine,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropositionEvidence {
    FactOnly,
    Witness { evidence_type: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropositionApplicationIdentity {
    pub id: psi_core::PropositionId,
    pub declaration: psi_core::PropositionId,
    pub binder_arguments: Vec<PropositionBinderArgumentIdentity>,
    pub arguments: Vec<String>,
    /// Exact instantiated carrierless interface. This is present exactly for
    /// witness-bearing applications and is their terminal identity authority.
    pub evidence_interface: Option<EvidenceInterfaceIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropositionBinderArgumentIdentity {
    pub kind: PropositionBinderArgumentKind,
    /// Canonical identity of an ordinary static argument. Evidence
    /// projections leave this empty and use the structured carrier below.
    pub identity: String,
    pub evidence_projection: Option<EvidenceProjectionIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceProjectionIdentity {
    pub term: EvidenceTermId,
    pub declaring_trait_identity: String,
    pub declaring_trait_arguments: Vec<String>,
    pub requirement_identity: String,
}

/// One exact carrierless witness identity retained independently of both its
/// nominal proposition and the proof provenance that established it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceTermDeclaration {
    pub id: EvidenceTermId,
    /// Exact normalized proposition application inhabited by this term.
    pub proposition: psi_core::PropositionId,
    /// Source-handle-free exact carrierless interface. This structured row,
    /// not `PropositionEvidence::Witness::evidence_type`, is the terminal
    /// identity authority for projection.
    pub interface: EvidenceInterfaceIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceInterfaceIdentity {
    pub trait_identity: String,
    pub arguments: Vec<String>,
    /// Complete canonical direct and inherited proof-static surface.
    pub requirements: Vec<EvidenceRequirementIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceRequirementIdentity {
    pub declaring_trait_identity: String,
    pub declaring_trait_arguments: Vec<String>,
    pub requirement_identity: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceContractLaneKind {
    Requires,
    Ensures,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceContractLane {
    pub machine: MachineId,
    pub kind: EvidenceContractLaneKind,
    pub position: u32,
    pub term: EvidenceTermId,
    /// Public named proof output. Present exactly on an
    /// `ensures` lane; `requires` names remain local input aliases.
    pub output_field: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropositionBinderArgumentKind {
    Type,
    Const,
    Machine,
}
