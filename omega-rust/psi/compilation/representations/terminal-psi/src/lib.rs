#![forbid(unsafe_code)]

//! Self-contained, target-neutral Terminal Psi.
//!
//! Begin at [`terminal_module`]: one current program with concept-owned
//! vocabulary. Optimizations precede publication; target realization follows it.
//! Canonical serialization and independent verification have separate owners.

mod artifacts;
pub(crate) use optimization::digest_encoder;
pub mod terminal_module;
pub use artifacts::{
    AdmissionEvidence, AdmissionKind, CertificateEnvelope, CheckedProgramEntryTerminalReceipt,
    CorrelatedAffineBranchWitness, CorrelatedAffineStepWitness, DebugFileId, DebugSite,
    DebugSourceDigest, DebugSourceFile, DebugSourceOrigin, DebugSourceSpan, DebugSubject,
    EvidenceRoute, IntegerAffineWitness, IntegerCastChainWitness,
    IntegerCorrelatedForbiddenRootWitness, PrimitiveJudgment, ProofNode, ProofRule,
    ProofSystemMarker, TerminalDebugMap, TerminalMachineProvenance,
};
pub use language_core::inline_assembly::{
    AsmControlRegister as ControlRegister, AsmFenceKind as MemoryFenceKind,
    AsmInterruptControlKind as InterruptControlKind, AsmRegisterCarrier as RegisterCarrier,
    AsmRegisterOperation as RegisterOperation, AsmSystemRegister as SystemRegister,
    AsmTargetApplicability as RegisterTargetApplicability,
};
pub use language_semantics::byte_predicates::{ByteSequencePredicate, ByteSequencePredicateSet};

mod callback_application;
pub use callback_application::TerminalCallbackApplication;

#[cfg(test)]
mod source_guards;

pub use language_core::BindingRelevance;
pub use semantic_vocabulary::{
    BoundaryCompletion, BoundaryRequirementKind, ExternalCompletionKind,
};
pub use terminal_module::*;

mod target_layouts;
pub use target_layouts::{
    LayoutApplicationId, StructuralRecordStorage, TargetLayoutApplication,
    TargetLayoutFieldStorage, TargetLayoutRealization, TargetLayoutSchemaArgument,
    TargetLayoutSchemaField, TargetLayoutSchemaFieldKind,
};

pub use terminal_module::quotients::{
    TerminalQuotientAdaptation, TerminalQuotientAdaptationLicense, TerminalQuotientCatalog,
    TerminalQuotientOperation, TerminalQuotientRepresentation,
};
mod direct_calls;
pub use direct_calls::{
    DirectCallParts, DirectCallRole, call_structural_arguments, direct_call_parts,
};
