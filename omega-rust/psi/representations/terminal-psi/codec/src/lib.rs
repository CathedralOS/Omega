#![forbid(unsafe_code)]

//! Canonical Terminal-Psi encoding, artifact publication, and receiver admission.
//!
//! [`sections::semantic_module`] owns canonical module encoding and identity;
//! other `sections/` owners retain independently identified wire products.
//! [`canonical_artifact`] binds those bytes into the handoff envelope, and
//! [`publication`] stages and publishes canonical semantic output.
//!
//! [`artifact_admission`] owns receiver policy and independent proof replay.
//! [`trust_graph`] reconstructs the implementation's disclosed trust closure;
//! [`verified_program`] projects producer schemas from verifier-owned evidence.
//! Canonical decoding and physical publication do not establish proof admission.

mod artifact_admission;
mod canonical_artifact;
mod codec_error;
pub use sections::semantic_module::provider_candidate_wire::{
    decode_provider_candidate_record, encode_provider_candidate_record,
};
mod publication;
mod sections;
mod trust_graph;
mod verified_program;

pub use artifact_admission::{
    PccIncompleteness, PccReceiverPolicy, PccRejection, PccVerificationOutcome, PccVerifiedProduct,
    TerminalProofVerdict, verify_pcc_claim_fields, verify_psi_proof_sidecar,
    verify_terminal_artifact_proof,
};
pub use canonical_artifact::{
    CanonicalTerminalArtifact, CanonicalTerminalArtifactEnvelopeError,
    CanonicalTerminalArtifactError,
};
pub use codec_error::CodecError;
pub use publication::{
    PublishedTerminalSemanticArtifact, TerminalSemanticArtifactPublication,
    TerminalSemanticPublicationError,
};
pub use sections::artifact_manifest::{
    ArtifactManifestError, SectionFingerprint, TerminalArtifactIdentity, TerminalArtifactManifest,
    build_artifact_manifest, validate_artifact_manifest,
};
pub use sections::debug_map::{
    DebugFileId, DebugMapError, DebugSite, DebugSourceDigest, DebugSourceFile, DebugSourceOrigin,
    DebugSourceSpan, DebugSubject, TerminalDebugMap, decode_debug_map, encode_debug_map,
    source_digest, validate_debug_map,
};
pub use sections::obligation_ledger::{
    TerminalObligationLedger, TerminalObligationLedgerFingerprint,
    build_terminal_obligation_ledger, decode_terminal_obligation_ledger,
    encode_terminal_obligation_ledger, terminal_obligation_ledger_fingerprint,
    validate_terminal_obligation_ledger,
};
pub use sections::optimization_execution::{
    PsiOptimizationExecutionIdentity, PsiOptimizationExecutionRecord,
    PsiOptimizationExecutionRecordBuildError, PsiOptimizationExecutionRecordDecodeError,
    PsiOptimizationExecutionRecordError, build_identity_optimization_execution_record,
    decode_psi_optimization_execution_record, encode_psi_optimization_execution_record,
};
pub use sections::proof_bundle::{
    ProofBundleFingerprint, ProofCodecError, decode_proof_bundle, decode_proof_section,
    decode_proof_section_for, encode_proof_bundle, encode_proof_section, proof_bundle_fingerprint,
    render_verified_proof_synopsis,
};
pub use sections::proof_sidecar::psi_claim::{
    PSI_TERMINAL_VERIFIED_GUARANTEE, admission_profile_identity, build_psi_proof_sidecar,
    psi_semantic_profile_identity, terminal_assumption_closure,
};
pub use sections::proof_sidecar::{
    PccDependency, PccGuarantee, PccProductKind, PccProofSidecar, pcc_artifact_commitment,
};
pub use sections::semantic_module::canonical_order::{
    canonical_proposition_order_key, canonical_scalar_term_order_key,
};
pub use sections::semantic_module::mathematical_certificate_wire::{
    DecodedMathematicalCertificate, decode_mathematical_certificate,
    encode_mathematical_certificate,
};
pub use sections::terminal_trace_v1_profile::{
    TerminalTraceV1ProfileAcceptanceError, TerminalTraceV1ProfileBuildError,
    TerminalTraceV1ProfileCodecError, accept_terminal_trace_v1_profile,
    decode_terminal_trace_v1_profile, encode_terminal_trace_v1_profile,
    reconstruct_canonical_terminal_trace_v1_profile,
};
pub use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity};
pub use trust_graph::{
    TerminalTrustGraphIdentity, TrustAcceptingPolicy, TrustDependencyDigest, TrustDependencyKind,
    TrustDependencyNode, TrustDependencyStatus, TrustGraphError, ValidatedTerminalTrustGraph,
    current_rust_operation_semantics_trust_identity, current_terminal_trust_graph,
    render_terminal_trust_graph, validate_terminal_trust_graph,
};
pub use verified_program::{
    ProgramLocalRootProducerCatalogError, VerifiedProgramLocalRootProducerCatalog,
    VerifiedProgramLocalRootProducerSchema,
};

pub use sections::semantic_module::{
    decode_canonical_proposition, decode_crash_route_buckets, decode_module,
    encode_crash_route_buckets, encode_module, encode_structural_type_declaration,
    semantic_fingerprint, terminal_psi_identity,
};

#[cfg(test)]
mod tests;
