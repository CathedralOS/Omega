#![forbid(unsafe_code)]

/*
We implement the canonical binary wire codec (`PSITERM\0`), cryptographic domain fingerprinting
(`SemanticFingerprint`, `TerminalPsiIdentity`), and receiving structural-foundation validation for
`TerminalModule` and its companion sections (`TerminalArtifactManifest`, `TerminalDebugMap`,
`OptimizationExecutionReceipt`, and `TerminalTraceV1`), together with the canonical encoders of
Terminal vocabulary (propositions, crash routes, access and quotient catalogs) that native forms
use for their own identities.

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
pub(crate) use mathematical_proof_actual::indexed_theorem_roots;
mod module_wire;
mod obligation_evidence;
mod sections;
mod semantic_module;
mod service_schema_wire;
mod structural_wire;
mod term_table;
mod term_wire;
pub(crate) use semantic_module::*;

pub(crate) use optimization::codec_bytes;

/// The encoding steps `terminal_codec::ClosedTerminalModule` and its artifact
/// envelope seal. Nothing else should call these: a module closed here is
/// not sealed until that type holds it.
/// Encodes a module without validating it, so a test can build deliberately
/// malformed wire for a receiver to refuse.
#[cfg(feature = "test-support")]
pub mod test_support {
    pub use crate::module_wire::encode_raw as encode_unchecked_module;
}

#[doc(hidden)]
pub mod sealing {
    pub use crate::sections::artifact_manifest::manifest_for_semantic;
    pub use crate::semantic_module::{
        decode_closed_parts, encode_closed_execution_module, encoded_fingerprint,
    };
}

pub(crate) mod codec_error {

    use semantic_vocabulary::PropositionError;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum CodecError {
        InvalidMagic,
        UnsupportedFormatMarker(u16),
        UnsupportedVocabularyMarker(u16),
        UnexpectedEnd,
        TrailingBytes(usize),
        InvalidBoolean(u8),
        InvalidTag(&'static str, u8),
        ZeroIdentity(&'static str),
        CollectionTooLong(&'static str),
        NonCanonicalOrder(&'static str),
        NonCanonicalEncoding,
        NestedConjunction,
        NestedDisjunction,
        /// A term-table row names its own or a later entry.
        TermTableForwardReference(&'static str),
        /// A term-table row names entries other than the unconsumed ones
        /// immediately before it, or a single-term table has other than one root.
        NonCanonicalTermTable(&'static str),
        StringTooLong(&'static str),
        InvalidUtf8(&'static str),
        MalformedStructuralFoundation(&'static str),
        MalformedWriteFrame(String),
        MalformedRanking(&'static str),
        MalformedScalarReturnFoundation {
            machine: String,
            domain: String,
            edge: semantic_vocabulary::EdgeId,
            source: semantic_vocabulary::ValueId,
            result: semantic_vocabulary::ValueId,
            reason: String,
        },
        MalformedOperationFoundation {
            machine: String,
            state: String,
            block: semantic_vocabulary::BlockId,
            operation: semantic_vocabulary::OperationId,
            reason: &'static str,
        },
        MalformedStructuralOperation {
            machine: String,
            operation: semantic_vocabulary::OperationId,
            reason: String,
        },
        AccessPlan(String),
        NativeLayout(String),
        AccessViewPlan(terminal_semantics::AccessViewPlanError),
        AccessRamCorrespondence(String),
        AccessViewInput(terminal_semantics::AccessViewInputError),
        MalformedMathematicalCertificate(&'static str),
        InvalidIdentityFormation(String),
        MalformedProposition(PropositionError),
        IntegerWidthClosure(semantic_vocabulary::IntegerWidthClosureError),
        ObligationLedgerMismatch,
    }

    impl std::fmt::Display for CodecError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "{self:?}")
        }
    }

    impl std::error::Error for CodecError {}

    pub(crate) fn malformed<T>(message: &'static str) -> Result<T, CodecError> {
        Err(CodecError::MalformedStructuralFoundation(message))
    }
}
pub use indexed_theorem_roots::verify_indexed_theorem_roots;
pub use mathematical_proof_actual::{
    MathematicalProofActualError, mathematical_carrier_key, verify_mathematical_proof_actual,
    verify_module_mathematical_actuals,
};
pub(crate) mod cursor {

    pub(crate) use optimization::cursor::Cursor;
}
pub(crate) use optimization::digest_encoder;
pub use sections::semantic_module::provider_candidate_wire::{
    decode_provider_candidate_record, encode_provider_candidate_record,
};

pub use codec_error::CodecError;
pub use obligation_evidence::{
    EvidenceSealValidationError, EvidenceSealValidator, EvidenceSeals, ObligationEvidenceSeal,
    canonical_certificate_bytes,
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
