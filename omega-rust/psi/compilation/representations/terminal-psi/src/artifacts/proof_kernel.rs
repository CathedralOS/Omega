//! Mathematical proof nodes and witness vocabulary shared with proof admission.

mod admission;
mod nodes;
mod witnesses;
pub use admission::{
    AdmissionEvidence, AdmissionKind, CertificateEnvelope, EvidenceRoute, ProofSystemMarker,
};
pub use nodes::{PrimitiveJudgment, ProofNode, ProofRule};
use semantic_vocabulary::{EvidenceIdentity, Proposition};
pub use witnesses::{
    CorrelatedAffineBranchWitness, CorrelatedAffineStepWitness, IntegerAffineWitness,
    IntegerCastChainWitness, IntegerCorrelatedForbiddenRootWitness,
};
