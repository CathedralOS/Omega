//! Canonical proof-artifact evidence and producer provenance model.

use proof_admission::{EvidenceRoute, RecursiveComponentCertificate};
use semantic_vocabulary::{EvidenceIdentity, EvidenceTermId, ObligationId, RecursiveComponentId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationEvidence {
    pub obligation: ObligationId,
    pub route: EvidenceRoute,
}

/// Grouped evidence for one exact verifier-reconstructed recursive component.
/// The semantic component key joins the certificate without relying on bundle
/// position or producer-selected topology.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecursiveComponentEvidence {
    pub component: RecursiveComponentId,
    pub certificate: RecursiveComponentCertificate,
}

/// Checked provenance for one freshly introduced carrierless evidence term.
/// This belongs to the proof artifact, not terminal-Psi semantic identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceProducerProvenance {
    pub id: EvidenceIdentity,
    pub term: EvidenceTermId,
    pub conformance_identity: String,
    pub evidence_trait_identity: String,
    pub rows: Vec<EvidenceProducerRealization>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceProducerRealization {
    pub declaring_trait_identity: String,
    pub declaring_trait_arguments: Vec<String>,
    pub requirement_identity: String,
    pub realization_machine_identity: String,
    pub realization_state_identity: String,
    pub source: EvidenceProducerRowSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceProducerRowSource {
    Inline,
    Reference,
    TraitDefault,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProofBundle {
    pub evidence: Vec<ObligationEvidence>,
    pub recursive_components: Vec<RecursiveComponentEvidence>,
    pub evidence_producers: Vec<EvidenceProducerProvenance>,
}
