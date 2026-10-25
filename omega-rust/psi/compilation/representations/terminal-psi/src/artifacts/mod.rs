//! Replaceable proof and presentation companions, separate from module semantics.
//!
//! These are data schemas, not admission results. Canonical codecs and checkers
//! consume them independently; merely constructing them grants no authority.

mod checked_program_entry;
mod debug_map;
mod machine_provenance;
pub use machine_provenance::TerminalMachineProvenance;
mod proof_kernel;
pub use checked_program_entry::CheckedProgramEntryTerminalReceipt;
pub use debug_map::{
    DebugFileId, DebugSite, DebugSourceDigest, DebugSourceFile, DebugSourceOrigin, DebugSourceSpan,
    DebugSubject, TerminalDebugMap,
};
pub use proof_kernel::{
    AdmissionEvidence, AdmissionKind, CertificateEnvelope, CorrelatedAffineBranchWitness,
    CorrelatedAffineStepWitness, EvidenceRoute, IntegerAffineWitness, IntegerCastChainWitness,
    IntegerCorrelatedForbiddenRootWitness, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker,
};
