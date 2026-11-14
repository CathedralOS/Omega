#![forbid(unsafe_code)]

/*
We implement the canonical binary wire codec (`PSITERM\0`), cryptographic domain fingerprinting
(`SemanticFingerprint`, `TerminalPsiIdentity`), and receiving structural-foundation validation for
`TerminalModule` and its companion artifact sections (`CanonicalTerminalArtifact`,
`TerminalArtifactManifest`, `TerminalDebugMap`, `OptimizationExecutionReceipt`, and
`TerminalTraceV1`).
*/

/*
Why enforce a single canonical byte encoding and run structural-foundation validation inside the
decoder rather than trusting deserialized tables? In Omega's separate-compilation and
proof-carrying architecture, a `SemanticFingerprint` must uniquely identify the exact semantic
program across hosts, caches, and package boundaries. Re-encoding every decoded module and
rejecting any byte stream that deviates from canonical table order, violates structural-foundation
closure, or carries trailing bytes guarantees that two artifacts have the same fingerprint if and
only if their canonical wire bytes are identical.
*/

/*
Consumed by `08_lowered-psi-to-terminal-psi` to publish canonical Terminal Psi artifacts and
compute module fingerprints, by `terminal-verification` and `00_terminal-psi-to-abstract-operations`
to admit foreign or cached artifacts across trust boundaries, and by `omega` build orchestration
and package review tools to inspect manifests, traces, and proof certificates.
*/

mod block_wire;
mod canonical_artifact;
pub mod entry_receipt;
mod wire_fields;
pub(crate) use canonical_artifact::execution_code;
mod foundation_validation;
mod identity_formations;
pub use identity_formations::{
    ExpectedIdentityFormation, ExpectedIdentityFormations, verify_identity_formations,
};
mod mathematical_proof_actual;
pub(crate) use mathematical_proof_actual::indexed_theorem_roots;
mod module_wire;
mod obligation_evidence;
mod sections;
mod semantic_module;
mod structural_wire;
mod term_table;
mod term_wire;
pub(crate) use semantic_module::*;

pub(crate) use optimization::codec_bytes;

pub use execution_code::{
    InvocationTerminalCode, RetainedTerminalCode, TerminalExecutionIdentity, TerminalExecutionView,
    TerminalProgramProduct, TerminalPublication,
};
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

pub use canonical_artifact::{
    CanonicalTerminalArtifact, CanonicalTerminalArtifactEnvelopeError,
    CanonicalTerminalArtifactError, TerminalArtifactView,
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
    encode_crash_route_buckets, encode_module, encode_structural_type_declaration,
    module_decode_count, semantic_fingerprint, terminal_psi_identity,
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
pub use sections::semantic_module::module_wire::quotient_wire::{
    decode_quotient_catalog, encode_quotient_catalog,
};
