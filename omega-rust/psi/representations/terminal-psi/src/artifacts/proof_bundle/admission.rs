use super::*;
use semantic_vocabulary::{AdmissionSiteId, ProfileDecisionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofSystemMarker;

impl ProofSystemMarker {
    pub const CURRENT: Self = Self;

    pub const fn new(raw: u16) -> Option<Self> {
        if raw == Self::CURRENT.get() {
            Some(Self::CURRENT)
        } else {
            None
        }
    }

    pub const fn get(self) -> u16 {
        1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AdmissionKind {
    ForeignBoundaryGuarantee,
    ProviderFact,
    CheckedAssemblyClaim,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateEnvelope {
    pub identity: EvidenceIdentity,
    pub proof_system_marker: ProofSystemMarker,
    pub proof: ProofNode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmissionEvidence {
    pub site: AdmissionSiteId,
    pub kind: AdmissionKind,
    pub authority_identity: EvidenceIdentity,
    pub evidence_identity: EvidenceIdentity,
    pub profile_decision: ProfileDecisionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceRoute {
    KernelDerived(PrimitiveJudgment),
    CertificateDerived(CertificateEnvelope),
    Admitted(AdmissionEvidence),
}
