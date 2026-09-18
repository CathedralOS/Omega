#![forbid(unsafe_code)]

//! Canonical binary encoding and semantic identity for terminal Psi.
//!
//! Only the semantic module is encoded here. Proof bundles, installation
//! records, and debug/source maps have separate identities and can be replaced
//! without changing [`TerminalPsiIdentity`].
//!
//! Start at [`encode_module`] and [`decode_module`]. The semantic module's
//! section payloads, shape checks, and byte cursors live under
//! [`semantic_module`], and the error vocabulary in [`codec_error`]. The other
//! root files each own one separately identified section: proof bundle and
//! sidecar, artifact envelope and manifest, publication, trust graph,
//! obligation ledger, debug map, optimization execution, observation profile,
//! and the program-local root catalog.
//!
//! `canonical_artifact.rs` is the handoff artifact, `publication.rs` publishes
//! one fail-closed, `codec_error.rs` is the single error, and `sections/`
//! holds the canonical encoding of every section.

mod canonical_artifact;
mod codec_error;
pub use sections::semantic_module::provider_candidate_wire::{
    decode_provider_candidate_record, encode_provider_candidate_record,
};
mod publication;
mod sections;

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
pub use sections::program_local_root_catalog::{
    ProgramLocalRootProducerCatalogError, VerifiedProgramLocalRootProducerCatalog,
    VerifiedProgramLocalRootProducerSchema,
};
pub use sections::proof_bundle::{
    ProofBundleFingerprint, ProofCodecError, decode_proof_bundle, decode_proof_section,
    decode_proof_section_for, encode_proof_bundle, encode_proof_section, proof_bundle_fingerprint,
    render_verified_proof_synopsis,
};
pub use sections::proof_sidecar::{
    PSI_TERMINAL_VERIFIED_GUARANTEE, PccDependency, PccGuarantee, PccIncompleteness,
    PccProductKind, PccProofSidecar, PccReceiverPolicy, PccRejection, PccVerificationOutcome,
    PccVerifiedProduct, TerminalProofVerdict, admission_profile_identity, build_psi_proof_sidecar,
    pcc_artifact_commitment, psi_semantic_profile_identity, terminal_assumption_closure,
    verify_pcc_claim_fields, verify_psi_proof_sidecar, verify_terminal_artifact_proof,
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
pub use sections::trust_graph::{
    TerminalTrustGraphIdentity, TrustAcceptingPolicy, TrustDependencyDigest, TrustDependencyKind,
    TrustDependencyNode, TrustDependencyStatus, TrustGraphError, ValidatedTerminalTrustGraph,
    current_rust_operation_semantics_trust_identity, current_terminal_trust_graph,
    render_terminal_trust_graph, validate_terminal_trust_graph,
};
pub use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity};

use sections::semantic_module::canonical_order::{
    crash_routes_are_canonical, validate_canonical_order, validate_crash_route_predicates,
};
use sections::semantic_module::contract_wire::{decode_crash_routes, encode_crash_routes};
use sections::semantic_module::module_wire::{decode_module_body, encode_raw};

use sections::semantic_module::module_foundation_validation::validate_structural_foundation;
use sections::semantic_module::proposition_wire::decode_proposition;
use sections::semantic_module::wire::{Reader, Writer};
use sections::semantic_module::{FINGERPRINT_DOMAIN, FORMAT_MARKER, MAGIC};
use sha2::{Digest, Sha256};
use terminal_psi::TerminalModule;
use terminal_verifier::validate_module_representation;

pub fn encode_module(module: &TerminalModule) -> Result<Vec<u8>, CodecError> {
    sections::semantic_module::scalar_qualification_wire::validate(&module.scalar_qualifications)?;
    validate_canonical_order(module)?;
    validate_structural_foundation(module)?;
    validate_module_representation(module).map_err(CodecError::InvalidModule)?;
    encode_raw(module)
}

/// Encode one structural declaration using the exact module-section wire format.
///
/// This omits the module header and declaration count. Encoding alone does not
/// validate the referenced-type closure or establish any value invariant.
pub fn encode_structural_type_declaration(
    declaration: &terminal_psi::StructuralTypeDeclaration,
) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    sections::semantic_module::structural_type_wire::encode_structural_type(
        &mut writer,
        declaration,
    )?;
    Ok(writer.finish())
}

pub fn decode_module(bytes: &[u8]) -> Result<TerminalModule, CodecError> {
    let mut reader = Reader::new(bytes);
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(CodecError::InvalidMagic);
    }
    let format_marker = reader.u16()?;
    if format_marker != FORMAT_MARKER {
        return Err(CodecError::UnsupportedFormatMarker(format_marker));
    }
    let module = decode_module_body(&mut reader)?;
    if reader.remaining() != 0 {
        return Err(CodecError::TrailingBytes(reader.remaining()));
    }
    sections::semantic_module::scalar_qualification_wire::validate(&module.scalar_qualifications)?;
    validate_canonical_order(&module)?;
    validate_structural_foundation(&module)?;
    validate_module_representation(&module).map_err(CodecError::InvalidModule)?;
    let canonical = encode_raw(&module)?;
    if canonical != bytes {
        return Err(CodecError::NonCanonicalEncoding);
    }
    Ok(module)
}

/// Encode one canonical ordered crash-continuation roster without a Terminal
/// module envelope. This is the shared semantic leaf used by later
/// identity-bearing optimizer artifacts that must retain exact call routes.
pub fn encode_crash_route_buckets(
    routes: &[terminal_psi::CrashRouteBucket],
) -> Result<Vec<u8>, CodecError> {
    if !crash_routes_are_canonical(routes) {
        return Err(CodecError::NonCanonicalOrder(
            "crash routes by cause and guard",
        ));
    }
    validate_crash_route_predicates(routes)?;
    let mut writer = Writer::default();
    encode_crash_routes(&mut writer, routes)?;
    Ok(writer.finish())
}

/// Decode one complete canonical crash-continuation roster.
pub fn decode_crash_route_buckets(
    bytes: &[u8],
) -> Result<Vec<terminal_psi::CrashRouteBucket>, CodecError> {
    let mut reader = Reader::new(bytes);
    let routes = decode_crash_routes(&mut reader)?;
    if reader.remaining() != 0 {
        return Err(CodecError::TrailingBytes(reader.remaining()));
    }
    if encode_crash_route_buckets(&routes)? != bytes {
        return Err(CodecError::NonCanonicalEncoding);
    }
    Ok(routes)
}

/// Decode a complete canonical proposition ordering key.
pub fn decode_canonical_proposition(
    bytes: &[u8],
) -> Result<semantic_vocabulary::Proposition, CodecError> {
    let mut reader = Reader::new(bytes);
    let proposition = decode_proposition(&mut reader, 0)?;
    if reader.remaining() != 0 {
        return Err(CodecError::TrailingBytes(reader.remaining()));
    }
    if canonical_proposition_order_key(&proposition)? != bytes {
        return Err(CodecError::NonCanonicalEncoding);
    }
    Ok(proposition)
}

pub fn semantic_fingerprint(module: &TerminalModule) -> Result<SemanticFingerprint, CodecError> {
    let bytes = encode_module(module)?;
    Ok(fingerprint_bytes(&bytes))
}

pub fn terminal_psi_identity(module: &TerminalModule) -> Result<TerminalPsiIdentity, CodecError> {
    Ok(TerminalPsiIdentity {
        vocabulary_marker: module.vocabulary_marker,
        program_fingerprint: semantic_fingerprint(module)?,
    })
}

fn fingerprint_bytes(bytes: &[u8]) -> SemanticFingerprint {
    let mut digest = Sha256::new();
    digest.update(FINGERPRINT_DOMAIN);
    let byte_len =
        u64::try_from(bytes.len()).expect("terminal-Psi bytes fit the u64 digest domain");
    digest.update(byte_len.to_le_bytes());
    digest.update(bytes);
    SemanticFingerprint::from_bytes(digest.finalize().into())
}

#[cfg(test)]
mod resource_tests {
    use super::{CodecError, Reader};
    use crate::sections::semantic_module::wire::decode_counted;

    #[test]
    fn counted_decoder_rejects_impossible_capacity_before_allocation() {
        let bytes = u32::MAX.to_le_bytes();
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            decode_counted::<u8>(&mut reader, |reader| reader.u8()),
            Err(CodecError::UnexpectedEnd)
        );
    }
}

#[cfg(test)]
mod crash_route_roster_tests {
    use terminal_psi::{CrashCause, CrashRouteBucket, CrashRouteGuard};

    use super::{CodecError, decode_crash_route_buckets, encode_crash_route_buckets};

    fn route(cause: CrashCause) -> CrashRouteBucket {
        CrashRouteBucket {
            cause,
            alternatives: vec![CrashRouteGuard::Truth],
        }
    }

    #[test]
    fn standalone_crash_route_roster_round_trips_and_rejects_corruption() {
        let routes = vec![route(CrashCause::Trap), route(CrashCause::Abort)];
        let encoded = encode_crash_route_buckets(&routes).unwrap();
        assert_eq!(decode_crash_route_buckets(&encoded).unwrap(), routes);

        let truncated = &encoded[..encoded.len() - 1];
        assert!(decode_crash_route_buckets(truncated).is_err());
    }

    #[test]
    fn standalone_crash_route_roster_rejects_noncanonical_order() {
        let routes = vec![route(CrashCause::Abort), route(CrashCause::Trap)];
        assert_eq!(
            encode_crash_route_buckets(&routes),
            Err(CodecError::NonCanonicalOrder(
                "crash routes by cause and guard"
            ))
        );
    }
}
