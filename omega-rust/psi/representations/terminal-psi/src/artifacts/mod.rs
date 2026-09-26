//! Replaceable proof and presentation companions, separate from module semantics.
//!
//! These are data schemas, not admission results. Canonical codecs and checkers
//! consume them independently; merely constructing them grants no authority.

mod checked_program_entry;
mod debug_map;
mod proof_bundle;
pub use checked_program_entry::{
    CheckedProgramEntryFusedServiceField, CheckedProgramEntryReceiverCleanup,
    CheckedProgramEntryReceiverEligibility, CheckedProgramEntryReceiverProjection,
    CheckedProgramEntryTerminalReceipt,
};
pub use debug_map::{
    DebugFileId, DebugSite, DebugSourceDigest, DebugSourceFile, DebugSourceOrigin, DebugSourceSpan,
    DebugSubject, TerminalDebugMap,
};
pub use proof_bundle::{
    AdmissionEvidence, AdmissionKind, CertificateEnvelope, ControlCycleEvidence,
    CorrelatedAffineBranchWitness, CorrelatedAffineStepWitness, CrashCertificate,
    CrashObligationEvidence, CrashObligationOwner, EvidenceProducerProvenance,
    EvidenceProducerRealization, EvidenceProducerRowSource, EvidenceRoute, IntegerAffineWitness,
    IntegerCastChainWitness, IntegerCorrelatedForbiddenRootWitness, ObligationEvidence,
    PrimitiveJudgment, ProofBundle, ProofNode, ProofRule, ProofSystemMarker,
    RecursiveComponentCertificate, RecursiveComponentEvidence, RecursiveEdgeCertificate,
};
