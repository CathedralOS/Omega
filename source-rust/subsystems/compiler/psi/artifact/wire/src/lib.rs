#![forbid(unsafe_code)]

/*
We implement the canonical binary wire codec (`PSITERM\0`), cryptographic domain fingerprinting
(`SemanticFingerprint`, `TerminalPsiIdentity`), and receiving structural-foundation validation for
`TerminalModule` and its companion sections (`TerminalArtifactManifest`, `TerminalDebugMap`
and `OptimizationExecutionReceipt`). The module format and native program identities
use the same declaration encodings from `psi-declaration-encoding`.

This crate does not define or hand out the sealed Terminal Psi module. `terminal-codec` owns
`ClosedTerminalModule`, the canonical artifact envelope and entry receipts, and re-exports this
crate, so a form that names only these encoders cannot reach the sealed module.
*/

/*
Why enforce a single canonical byte encoding and run structural-foundation validation inside the
decoder rather than trusting deserialized tables? In Omega's separate-compilation
architecture, a `SemanticFingerprint` must identify the exact semantic
program across hosts, caches, and package boundaries. Re-encoding every decoded module and
rejecting any byte stream that deviates from canonical table order, violates structural-foundation
closure, or carries trailing bytes gives a single byte representation of portable
semantics. The digest commits to those bytes; it does not establish their truth
or authorize execution.
*/

/*
The lowered-form checker's accepted closure publishes canonical Psi artifacts
through `terminal-codec`, which encodes them with this crate. The interpreter, native lowering and inspection paths borrow
the same immutable closed module or decode external bytes at ingress. Receiving
host authorization of an exact producer and artifact remains separate from
decoding, structural validation and carried proof data.
*/

mod block_wire;
pub use psi_declaration_encoding::invocations::{
    decode_callable_invocations, encode_callable_invocations,
};
mod closed_types;
pub mod contract_forms;
pub use closed_types::ClosedTypeCatalog;
mod foundation_validation;
mod identity_formations;
mod wire_fields;
pub use identity_formations::{
    ExpectedIdentityFormation, ExpectedIdentityFormations, verify_identity_formations,
};
mod mathematical_proof_actual;
mod theorem_citations;
pub(crate) use mathematical_proof_actual::indexed_theorem_roots;
pub use theorem_citations::{
    TheoremCitationContext, theorem_citation_actual, theorem_citation_context,
    validate_theorem_evidence, validate_theorem_parameters, validate_theorem_supply_sites,
};
mod module_wire;
mod obligation_evidence;
mod sections;
mod semantic_module;
mod service_schema_wire;
#[cfg(test)]
mod spec_tag_tables;
mod structural_wire;
pub(crate) use psi_declaration_encoding::terms::table as term_table;
mod term_wire;
pub(crate) use semantic_module::*;

pub(crate) use ::wire::codec_bytes;

/// Encodes a module without validation so a test can build malformed wire
/// for a receiver to refuse.
#[cfg(feature = "test-support")]
pub mod test_support {
    pub use crate::module_wire::encode_raw as encode_unchecked_module;
}

/// Encoding and decoding steps used by `terminal_codec::ClosedTerminalModule`
/// and its artifact envelope. The codec owns the sealed module type.
#[doc(hidden)]
pub mod sealing {
    pub use crate::sections::artifact_manifest::manifest_for_semantic;
    pub use crate::semantic_module::{
        decode_closed_parts, encode_closed_execution_module, encode_invocation_prefix,
        encoded_fingerprint,
    };
}

pub use indexed_theorem_roots::verify_indexed_theorem_roots;
pub use mathematical_proof_actual::{
    MathematicalProofActualError, mathematical_carrier_key, theorem_implication_key,
    verify_mathematical_proof_actual, verify_module_mathematical_actuals,
};
pub(crate) use psi_declaration_encoding::error as codec_error;
pub(crate) mod cursor {

    pub(crate) use ::wire::cursor::Cursor;
}
pub(crate) use ::wire::digest_encoder;
pub use sections::semantic_module::provider_candidate_wire::{
    decode_provider_candidate_record, encode_provider_candidate_record,
};

pub use codec_error::CodecError;
pub use obligation_evidence::{
    EvidenceSealValidationError, EvidenceSealValidator, EvidenceSeals, ObligationEvidenceSeal,
    canonical_certificate_bytes, decode_operation_evidence, encode_operation_evidence,
};
pub use sections::artifact_manifest::{
    ArtifactManifestError, SectionFingerprint, TerminalArtifactIdentity, TerminalArtifactManifest,
    build_artifact_manifest, validate_artifact_manifest,
    validate_artifact_manifest_with_publication,
};
mod debug_map_records;
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
    encode_crash_route_buckets, encode_module, encode_structural_domain_declaration,
    encode_structural_type_declaration, module_decode_count, semantic_fingerprint,
    terminal_psi_identity,
};

pub use sections::semantic_module::module_wire::access_lifecycle_wire::{
    decode_access_lifecycle_catalog, encode_access_lifecycle_catalog,
};
pub use sections::semantic_module::module_wire::access_view_plan_wire::{
    decode_access_view_catalog, encode_access_view_catalog,
};
pub use sections::semantic_module::module_wire::access_view_wire::{
    decode_access_view_input_catalog, encode_access_view_input_catalog,
};
pub use sections::semantic_module::module_wire::callable_values_wire::{
    decode_callable_catalog, encode_callable_catalog,
};
pub use sections::semantic_module::module_wire::dynamic_dispatch_wire::{
    decode_dynamic_dispatch_catalog, encode_dynamic_dispatch_catalog,
};
pub use sections::semantic_module::module_wire::quotient_wire::{
    decode_quotient_catalog, encode_quotient_catalog,
};
