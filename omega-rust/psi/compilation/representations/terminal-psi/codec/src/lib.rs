#![forbid(unsafe_code)]

//! Canonical Terminal-Psi encoding.
//!
//! [`sections::semantic_module`] owns canonical module encoding and identity;
//! other `sections/` owners retain independently identified wire products.
//! [`canonical_artifact`] binds those bytes into the handoff envelope.

pub(crate) use optimization::codec_bytes;

mod canonical_artifact;
mod codec_error;
mod cursor;
pub(crate) use optimization::digest_encoder;
pub use sections::semantic_module::provider_candidate_wire::{
    decode_provider_candidate_record, encode_provider_candidate_record,
};
mod sections;

pub use canonical_artifact::{
    CanonicalTerminalArtifact, CanonicalTerminalArtifactEnvelopeError,
    CanonicalTerminalArtifactError, TerminalArtifactView,
};
pub use codec_error::CodecError;
pub use sections::artifact_manifest::{
    ArtifactManifestError, SectionFingerprint, TerminalArtifactIdentity, TerminalArtifactManifest,
    build_artifact_manifest, validate_artifact_manifest,
};
pub use sections::debug_map::{
    DebugFileId, DebugMapError, DebugSite, DebugSourceDigest, DebugSourceFile, DebugSourceOrigin,
    DebugSourceSpan, DebugSubject, TerminalDebugMap, decode_debug_map, encode_debug_map,
    source_digest, validate_debug_map,
};
pub use sections::optimization_execution::{
    PsiOptimizationExecutionIdentity, PsiOptimizationExecutionRecord,
    PsiOptimizationExecutionRecordBuildError, PsiOptimizationExecutionRecordDecodeError,
    PsiOptimizationExecutionRecordError, append_terminal_identity,
    build_identity_optimization_execution_record, decode_psi_optimization_execution_record,
    encode_psi_optimization_execution_record,
};
pub use sections::semantic_module::canonical_order::{
    canonical_proposition_order_key, canonical_scalar_term_order_key,
};
pub use sections::semantic_module::mathematical_certificate_wire::{
    DecodedMathematicalCertificate, decode_mathematical_certificate,
    encode_mathematical_certificate,
};
pub use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity};

pub use sections::semantic_module::{
    decode_canonical_proposition, decode_crash_route_buckets, decode_module,
    encode_crash_route_buckets, encode_module, encode_structural_type_declaration,
    module_decode_count, semantic_fingerprint, terminal_psi_identity,
};

pub use sections::semantic_module::module_wire::quotient_wire::{
    decode_quotient_catalog, encode_quotient_catalog,
};
