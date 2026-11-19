/*
Replaceable proof, debug-map, machine-provenance, and program-entry companion records carried
alongside `TerminalModule` in canonical Terminal Psi artifacts (`TerminalDebugMap`,
`TerminalMachineProvenance`, `CheckedProgramEntryTerminalReceipt`, `CertificateEnvelope`, and
`ProofNode` trees). These records are pure data schemas; canonical codecs and independent verifiers
validate them at artifact boundaries without trusting producer construction.
*/

pub(crate) mod checked_program_entry;
pub(crate) mod debug_map;
pub(crate) mod machine_provenance;
pub use machine_provenance::TerminalMachineProvenance;
pub(crate) mod proof_kernel;
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
