//! One-field mutation coverage for the canonical Terminal artifact transport
//! envelope (`PSIART`), the custody boundary that joins every canonical
//! section under one recomputed artifact identity.
//!
//! The envelope owns only its header — the artifact magic, the format marker,
//! one declared u64 length per retained section, the debug presence tag, and
//! the debug length prefix — plus the four section payloads whose own codecs
//! authenticate their contents. `from_bytes` re-derives the manifest from the
//! decoded sections and refuses any byte-level drift the inner decoders
//! tolerate, so a substitution either fails canonical decoding (at the
//! envelope or inside one section) or rides a consistent payload set whose
//! honestly recomputed artifact identity diverges and whose replay against
//! the retained manifest and the terminal verifier rejects.

use std::ops::Range;

use super::{canonical_artifact, kernel_bundle, machine_id, obligation_id, semantic_module};
use optimization::{PsiOptimization, PsiOptimizationSelections};
use proof_admission::AdmissionProfile;
use terminal_codec::{
    ArtifactManifestError, CanonicalTerminalArtifact, CanonicalTerminalArtifactEnvelopeError,
    CanonicalTerminalArtifactError, CodecError, DebugFileId, DebugMapError, DebugSite,
    DebugSourceFile, DebugSourceOrigin, DebugSourceSpan, DebugSubject, ProofCodecError,
    PsiOptimizationExecutionRecord, PsiOptimizationExecutionRecordDecodeError,
    PsiOptimizationExecutionRecordError, TerminalDebugMap,
    build_identity_optimization_execution_record, decode_module, decode_proof_section_for,
    encode_debug_map, encode_module, encode_proof_section,
    encode_psi_optimization_execution_record, proof_bundle_fingerprint, source_digest,
    terminal_psi_identity, validate_artifact_manifest, verify_terminal_artifact_proof,
};
use terminal_psi::TerminalModule;
use terminal_verifier::ProofBundle;

/// Byte offsets of every envelope-owned field: the fixed header, each
/// declared length, and the four section payload spans.
struct EnvelopeSpans {
    magic: Range<usize>,
    format_marker: Range<usize>,
    semantic_len: Range<usize>,
    proof_len: Range<usize>,
    optimization_len: Range<usize>,
    debug_tag: Range<usize>,
    debug_len: Option<Range<usize>>,
    semantic: Range<usize>,
    proof: Range<usize>,
    optimization: Range<usize>,
    debug: Option<Range<usize>>,
}

fn envelope_spans(bytes: &[u8]) -> EnvelopeSpans {
    let len_at = |range: Range<usize>| -> usize {
        usize::try_from(u64::from_le_bytes(
            bytes[range].try_into().expect("u64 length field"),
        ))
        .expect("a fixture section length fits usize")
    };
    let semantic_len = len_at(10..18);
    let proof_len = len_at(18..26);
    let optimization_len = len_at(26..34);
    let debug_len = match bytes[34] {
        0 => None,
        1 => Some(len_at(35..43)),
        tag => panic!("the fixture envelope carries a valid debug tag, not {tag}"),
    };
    let payload = if debug_len.is_some() { 43 } else { 35 };
    let semantic = payload..payload + semantic_len;
    let proof = semantic.end..semantic.end + proof_len;
    let optimization = proof.end..proof.end + optimization_len;
    let debug = debug_len.map(|len| optimization.end..optimization.end + len);
    assert_eq!(
        debug.as_ref().map_or(optimization.end, |span| span.end),
        bytes.len(),
        "the fixture envelope is exactly its declared sections"
    );
    EnvelopeSpans {
        magic: 0..8,
        format_marker: 8..10,
        semantic_len: 10..18,
        proof_len: 18..26,
        optimization_len: 26..34,
        debug_tag: 34..35,
        debug_len: debug_len.map(|_| 35..43),
        semantic,
        proof,
        optimization,
        debug,
    }
}

/// One debug map bound to the fixture module: a single file and a single
/// machine subject, the smallest section the envelope can carry.
fn debug_map(module: &TerminalModule) -> TerminalDebugMap {
    let file = DebugFileId::new(1).expect("nonzero file identity");
    TerminalDebugMap {
        semantic: terminal_psi_identity(module).expect("module identity"),
        files: vec![DebugSourceFile {
            id: file,
            origin: DebugSourceOrigin::User,
            byte_len: 15,
            digest: source_digest(b"machine main {}"),
            path: "main.omg".to_owned(),
        }],
        sites: vec![DebugSite {
            subject: DebugSubject::Machine(machine_id(1)),
            span: DebugSourceSpan {
                file,
                start: 0,
                end: 7,
            },
        }],
    }
}

/// Rebuild a canonical envelope around honestly encoded sections, repairing
/// every declared length the header carries. This is exactly the layout
/// `to_bytes` produces for the same sections.
fn envelope_for(
    semantic: &[u8],
    proof: &[u8],
    optimization: &[u8],
    debug: Option<&[u8]>,
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"PSIART\0\0");
    out.extend_from_slice(&2_u16.to_le_bytes());
    for len in [semantic.len(), proof.len(), optimization.len()] {
        out.extend_from_slice(
            &u64::try_from(len)
                .expect("a fixture section fits u64")
                .to_le_bytes(),
        );
    }
    match debug {
        None => out.push(0),
        Some(bytes) => {
            out.push(1);
            out.extend_from_slice(
                &u64::try_from(bytes.len())
                    .expect("the debug section fits u64")
                    .to_le_bytes(),
            );
        }
    }
    out.extend_from_slice(semantic);
    out.extend_from_slice(proof);
    out.extend_from_slice(optimization);
    if let Some(bytes) = debug {
        out.extend_from_slice(bytes);
    }
    out
}

fn substitute(envelope: &[u8], field: &Range<usize>, replacement: &[u8]) -> Vec<u8> {
    let mut mutated = envelope[..field.start].to_vec();
    mutated.extend_from_slice(replacement);
    mutated.extend_from_slice(&envelope[field.end..]);
    mutated
}

fn flip(envelope: &[u8], offset: usize) -> Vec<u8> {
    let mut mutated = envelope.to_vec();
    mutated[offset] ^= 0xFF;
    mutated
}

fn set_len(envelope: &[u8], field: &Range<usize>, declared: u64) -> Vec<u8> {
    substitute(envelope, field, &declared.to_le_bytes())
}

fn reject(name: &'static str, bytes: &[u8], expected: CanonicalTerminalArtifactError) {
    assert_eq!(
        CanonicalTerminalArtifact::from_bytes(bytes),
        Err(expected),
        "{name} must reject at canonical decoding"
    );
}

/// A representable substitution: the envelope still decodes to an internally
/// consistent artifact whose recomputed identity diverges, and the retained
/// manifest replay rejects it.
fn divergent(
    name: &'static str,
    mutated: &[u8],
    retained: &CanonicalTerminalArtifact,
) -> CanonicalTerminalArtifact {
    let decoded = CanonicalTerminalArtifact::from_bytes(mutated)
        .unwrap_or_else(|error| panic!("{name} must still decode: {error:?}"));
    assert_eq!(
        decoded.to_bytes(),
        mutated,
        "{name} must re-encode byte-exact"
    );
    decoded
        .validate()
        .unwrap_or_else(|error| panic!("{name} must stay internally consistent: {error:?}"));
    assert_ne!(
        decoded.manifest().identity(),
        retained.manifest().identity(),
        "{name} must diverge the honestly recomputed artifact identity"
    );
    let module = decode_module(decoded.semantic_bytes())
        .expect("a representable semantic section still decodes");
    let proof = decode_proof_section_for(&module, decoded.proof_bytes())
        .expect("a representable proof section still decodes for its subject");
    assert_eq!(
        validate_artifact_manifest(
            &module,
            &proof,
            decoded.optimization(),
            None,
            decoded.debug_bytes(),
            retained.manifest(),
        ),
        Err(ArtifactManifestError::ManifestMismatch),
        "{name} must reject at the retained-manifest replay"
    );
    decoded
}

/// A complete consistent payload set for one semantic module and bundle: the
/// sealed proof section and identity-stage optimization receipt recomputed
/// for that exact pair, so the envelope decodes to a real (foreign) artifact.
fn consistent_sections(
    module: &TerminalModule,
    bundle: &ProofBundle,
) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let semantic = encode_module(module).expect("a canonical module encodes");
    let proof = encode_proof_section(module, bundle).expect("a sealed proof section encodes");
    let optimization = encode_psi_optimization_execution_record(
        &build_identity_optimization_execution_record(module, bundle)
            .expect("an identity-stage optimization receipt"),
    );
    (semantic, proof, optimization)
}

#[test]
fn canonical_terminal_artifact_envelope_rejects_every_one_field_substitution() {
    let module = semantic_module();
    let bundle = kernel_bundle();

    // The two retained artifacts: one without and one carrying the optional
    // debug section, so both presence states of the tagged header field are
    // exercised.
    let retained = canonical_artifact(&module, &bundle, None);
    let map = debug_map(&module);
    let retained_debug = canonical_artifact(&module, &bundle, Some(&map));
    for artifact in [&retained, &retained_debug] {
        artifact.validate().expect("the retained artifact replays");
        verify_terminal_artifact_proof(artifact, &AdmissionProfile::default())
            .expect("the retained artifact passes verifier replay");
    }
    let envelope = retained.to_bytes();
    let debug_envelope = retained_debug.to_bytes();
    let spans = envelope_spans(&envelope);
    let debug_spans = envelope_spans(&debug_envelope);
    assert_eq!(envelope[spans.debug_tag.clone()], [0]);
    assert_eq!(debug_envelope[debug_spans.debug_tag.clone()], [1]);
    assert_eq!(
        debug_spans
            .debug_len
            .clone()
            .map(|span| span.end - span.start),
        Some(8)
    );

    // --- magic and format marker reject at decoding ---

    for offset in spans.magic.clone() {
        reject(
            "an artifact-magic byte",
            &flip(&envelope, offset),
            CanonicalTerminalArtifactError::Envelope(
                CanonicalTerminalArtifactEnvelopeError::InvalidMagic,
            ),
        );
    }
    for marker in [0_u16, 1, 3, u16::MAX] {
        reject(
            "the artifact format marker",
            &substitute(&envelope, &spans.format_marker, &marker.to_le_bytes()),
            CanonicalTerminalArtifactError::Envelope(
                CanonicalTerminalArtifactEnvelopeError::UnsupportedFormatMarker(marker),
            ),
        );
    }

    // --- declared section lengths ---
    //
    // Every declared length is read before any payload is taken, so a
    // single-field length lie never reaches a section codec: a shrunken
    // declaration leaves the surrendered tail as trailing bytes and a grown
    // one starves a later section read.

    let semantic_len = spans.semantic.end - spans.semantic.start;
    let proof_len = spans.proof.end - spans.proof.start;
    let optimization_len = spans.optimization.end - spans.optimization.start;
    reject(
        "a cleared semantic length",
        &set_len(&envelope, &spans.semantic_len, 0),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::TrailingBytes(semantic_len),
        ),
    );
    reject(
        "a shortened semantic length",
        &set_len(
            &envelope,
            &spans.semantic_len,
            u64::try_from(semantic_len - 1).unwrap(),
        ),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::TrailingBytes(1),
        ),
    );
    reject(
        "a lengthened semantic length",
        &set_len(
            &envelope,
            &spans.semantic_len,
            u64::try_from(semantic_len + 1).unwrap(),
        ),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd,
        ),
    );
    reject(
        "an over-large semantic length",
        &set_len(&envelope, &spans.semantic_len, u64::MAX),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd,
        ),
    );
    // Swapping the declared semantic and proof lengths keeps the declared sum
    // exact, so the shifted semantic span does decode — into a truncated
    // module the semantic codec rejects.
    let swapped = substitute(
        &set_len(
            &envelope,
            &spans.semantic_len,
            u64::try_from(proof_len).unwrap(),
        ),
        &spans.proof_len,
        &u64::try_from(semantic_len).unwrap().to_le_bytes(),
    );
    assert!(
        matches!(
            CanonicalTerminalArtifact::from_bytes(&swapped),
            Err(CanonicalTerminalArtifactError::Semantic(_))
        ),
        "swapped declared lengths must starve or spill the module decode"
    );

    // --- declared proof length ---

    reject(
        "a cleared proof length",
        &set_len(&envelope, &spans.proof_len, 0),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::TrailingBytes(proof_len),
        ),
    );
    reject(
        "a shortened proof length",
        &set_len(
            &envelope,
            &spans.proof_len,
            u64::try_from(proof_len - 1).unwrap(),
        ),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::TrailingBytes(1),
        ),
    );
    reject(
        "a lengthened proof length",
        &set_len(
            &envelope,
            &spans.proof_len,
            u64::try_from(proof_len + 1).unwrap(),
        ),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd,
        ),
    );
    reject(
        "an over-large proof length",
        &set_len(&envelope, &spans.proof_len, u64::MAX),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd,
        ),
    );

    // --- declared optimization length ---

    reject(
        "a cleared optimization length",
        &set_len(&envelope, &spans.optimization_len, 0),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::TrailingBytes(optimization_len),
        ),
    );
    reject(
        "a shortened optimization length",
        &set_len(
            &envelope,
            &spans.optimization_len,
            u64::try_from(optimization_len - 1).unwrap(),
        ),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::TrailingBytes(1),
        ),
    );
    reject(
        "a lengthened optimization length",
        &set_len(
            &envelope,
            &spans.optimization_len,
            u64::try_from(optimization_len + 1).unwrap(),
        ),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd,
        ),
    );
    reject(
        "an over-large optimization length",
        &set_len(&envelope, &spans.optimization_len, u64::MAX),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd,
        ),
    );
    // On the debug-bearing envelope a lengthened optimization declaration
    // starves the debug read the same way.
    reject(
        "a lengthened optimization length over a debug section",
        &set_len(
            &debug_envelope,
            &debug_spans.optimization_len,
            u64::try_from(optimization_len + 1).unwrap(),
        ),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd,
        ),
    );

    // --- debug presence tag ---

    for tag in [2_u8, u8::MAX] {
        reject(
            "an unknown debug presence tag",
            &substitute(&envelope, &spans.debug_tag, &[tag]),
            CanonicalTerminalArtifactError::Envelope(
                CanonicalTerminalArtifactEnvelopeError::InvalidDebugTag(tag),
            ),
        );
    }
    // Claiming presence without a declared debug length reads the semantic
    // magic as a u64 length and overruns the envelope.
    reject(
        "a presence tag with no debug length field",
        &substitute(&envelope, &spans.debug_tag, &[1]),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd,
        ),
    );
    // Clearing presence on the debug-bearing envelope strands the now
    // unclaimed debug length field and payload as trailing bytes.
    let debug_payload_len = debug_spans.debug.clone().unwrap().len();
    reject(
        "a dropped debug presence tag",
        &substitute(&debug_envelope, &debug_spans.debug_tag, &[0]),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::TrailingBytes(8 + debug_payload_len),
        ),
    );
    reject(
        "a corrupted debug presence tag",
        &substitute(&debug_envelope, &debug_spans.debug_tag, &[2]),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::InvalidDebugTag(2),
        ),
    );

    // --- declared debug length on the debug-bearing envelope ---

    reject(
        "a cleared debug length",
        &set_len(&debug_envelope, &debug_spans.debug_len.clone().unwrap(), 0),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::TrailingBytes(debug_payload_len),
        ),
    );
    reject(
        "a shortened debug length",
        &set_len(
            &debug_envelope,
            &debug_spans.debug_len.clone().unwrap(),
            u64::try_from(debug_spans.debug.clone().unwrap().len() - 1).unwrap(),
        ),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::TrailingBytes(1),
        ),
    );
    reject(
        "a lengthened debug length",
        &set_len(
            &debug_envelope,
            &debug_spans.debug_len.clone().unwrap(),
            u64::try_from(debug_spans.debug.clone().unwrap().len() + 1).unwrap(),
        ),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd,
        ),
    );
    reject(
        "an over-large debug length",
        &set_len(
            &debug_envelope,
            &debug_spans.debug_len.clone().unwrap(),
            u64::MAX,
        ),
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd,
        ),
    );

    // --- payload bytes belong to their own section codecs ---

    reject(
        "a corrupted semantic magic",
        &flip(&envelope, spans.semantic.start),
        CanonicalTerminalArtifactError::Semantic(CodecError::InvalidMagic),
    );
    reject(
        "a corrupted proof magic",
        &flip(&envelope, spans.proof.start),
        CanonicalTerminalArtifactError::Proof(ProofCodecError::InvalidMagic),
    );
    reject(
        "a corrupted optimization magic",
        &flip(&envelope, spans.optimization.start),
        CanonicalTerminalArtifactError::Optimization(
            PsiOptimizationExecutionRecordDecodeError::InvalidMagic,
        ),
    );
    reject(
        "a corrupted debug magic",
        &flip(&debug_envelope, debug_spans.debug.clone().unwrap().start),
        CanonicalTerminalArtifactError::Debug(DebugMapError::InvalidMagic),
    );

    // A section payload moved out of its slot decodes under the wrong codec:
    // the declared lengths honestly follow the payloads, so the semantic slot
    // now holds the sealed proof section.
    reject(
        "the proof payload occupying the semantic slot",
        &envelope_for(
            &envelope[spans.proof.clone()],
            &envelope[spans.semantic.clone()],
            &envelope[spans.optimization.clone()],
            None,
        ),
        CanonicalTerminalArtifactError::Semantic(CodecError::InvalidMagic),
    );
    // A dropped section keeps its declared length but surrenders its bytes to
    // the neighboring spans, so some later section read starves.
    for (name, dropped) in [
        ("a dropped semantic payload", {
            let mut mutated = envelope[..spans.semantic.start].to_vec();
            mutated.extend_from_slice(&envelope[spans.semantic.end..]);
            mutated
        }),
        ("a dropped proof payload", {
            let mut mutated = envelope[..spans.proof.start].to_vec();
            mutated.extend_from_slice(&envelope[spans.proof.end..]);
            mutated
        }),
        (
            "a dropped optimization payload",
            envelope[..spans.optimization.start].to_vec(),
        ),
        (
            "a dropped debug payload",
            debug_envelope[..debug_spans.debug.clone().unwrap().start].to_vec(),
        ),
    ] {
        reject(
            name,
            &dropped,
            CanonicalTerminalArtifactError::Envelope(
                CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd,
            ),
        );
    }
    // An extra section payload is trailing envelope content.
    reject(
        "a duplicated optimization payload",
        &{
            let mut mutated = envelope.clone();
            mutated.extend_from_slice(&envelope[spans.optimization.clone()]);
            mutated
        },
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::TrailingBytes(optimization_len),
        ),
    );

    // --- trailing bytes and truncation reject at the envelope ---

    reject(
        "a trailing byte",
        &{
            let mut mutated = envelope.clone();
            mutated.push(0);
            mutated
        },
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::TrailingBytes(1),
        ),
    );
    for cut in [
        spans.magic.end - 1,
        spans.format_marker.end - 1,
        spans.semantic_len.end - 1,
        spans.proof_len.end - 1,
        spans.optimization_len.end - 1,
        spans.debug_tag.start,
        spans.semantic.start + 1,
        spans.semantic.end - 1,
        spans.proof.start + 1,
        spans.optimization.start + 1,
        envelope.len() - 1,
    ] {
        reject(
            "a truncated envelope",
            &envelope[..cut],
            CanonicalTerminalArtifactError::Envelope(
                CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd,
            ),
        );
    }
    for cut in [
        debug_spans.debug.clone().unwrap().start + 1,
        debug_envelope.len() - 1,
    ] {
        reject(
            "a truncated debug-bearing envelope",
            &debug_envelope[..cut],
            CanonicalTerminalArtifactError::Envelope(
                CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd,
            ),
        );
    }
    reject(
        "an empty envelope",
        &[],
        CanonicalTerminalArtifactError::Envelope(
            CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd,
        ),
    );

    // --- cross-section joins reject non-canonical custody ---

    // A foreign module: the same machine shape under a different machine and
    // entry identity. Its semantic section is canonical on its own.
    let mut foreign = semantic_module();
    foreign.machines[0].id = machine_id(7);
    foreign.entry = machine_id(7);
    let (foreign_semantic, foreign_proof, foreign_optimization) =
        consistent_sections(&foreign, &bundle);

    // Substituting the semantic payload without resealing the joined proof
    // section rejects at the subject-binding decode: the retained section is
    // sealed to the original module identity.
    assert!(
        matches!(
            CanonicalTerminalArtifact::from_bytes(&envelope_for(
                &foreign_semantic,
                &envelope[spans.proof.clone()],
                &envelope[spans.optimization.clone()],
                None,
            )),
            Err(CanonicalTerminalArtifactError::Proof(
                ProofCodecError::ProofSubjectMismatch { .. }
            ))
        ),
        "a foreign semantic payload must reject at the proof subject join"
    );
    // The foreign-sealed proof section under the retained semantic payload
    // rejects at the same join in the other direction.
    assert!(
        matches!(
            CanonicalTerminalArtifact::from_bytes(&envelope_for(
                &envelope[spans.semantic.clone()],
                &foreign_proof,
                &envelope[spans.optimization.clone()],
                None,
            )),
            Err(CanonicalTerminalArtifactError::Proof(
                ProofCodecError::ProofSubjectMismatch { .. }
            ))
        ),
        "a foreign-sealed proof payload must reject at the subject join"
    );
    // The residual `NonCanonicalSections` byte join stays defense in depth:
    // every section decoder already requires canonical re-encodings, so no
    // single-field substitution reaches it.
    // The foreign identity-stage receipt under retained semantic and proof
    // payloads fails the produced-output binding inside manifest building.
    assert!(
        matches!(
            CanonicalTerminalArtifact::from_bytes(&envelope_for(
                &envelope[spans.semantic.clone()],
                &envelope[spans.proof.clone()],
                &foreign_optimization,
                None,
            )),
            Err(CanonicalTerminalArtifactError::Manifest(
                ArtifactManifestError::Optimization(
                    PsiOptimizationExecutionRecordError::OutputMismatch
                )
            ))
        ),
        "a foreign optimization receipt must reject at the output binding"
    );

    // --- representable substitutions: honestly recomputed containing
    // identity, rejected by independent replay ---

    // The complete consistent foreign payload set decodes to a real artifact
    // whose recomputed identity diverges; the retained manifest and the
    // verifier replay both reject it as a substitute for the retained
    // artifact (the kernel bundle still discharges the unchanged contract,
    // so the semantic verdict is unchanged even though custody diverged).
    let substituted = divergent(
        "a consistently foreign artifact",
        &envelope_for(
            &foreign_semantic,
            &foreign_proof,
            &foreign_optimization,
            None,
        ),
        &retained,
    );
    let verdict = verify_terminal_artifact_proof(&substituted, &AdmissionProfile::default())
        .expect("the foreign artifact still verifies under the unchanged contract");
    assert_eq!(
        verdict.semantic_subject,
        terminal_psi_identity(&foreign).expect("foreign module identity"),
    );
    assert_ne!(
        verdict.semantic_subject,
        retained.manifest().semantic(),
        "the foreign artifact must not carry the retained semantic subject"
    );

    // A proof-bundle substitution resealed for the retained module: the
    // obligation retarget is representable, the recomputed identity diverges,
    // and the verifier replay refuses obligations the module never raised.
    let mut retargeted = bundle.clone();
    retargeted.evidence[0].obligation = obligation_id(77);
    let (proof_semantic, proof_substitution, proof_optimization) =
        consistent_sections(&module, &retargeted);
    let substituted = divergent(
        "a retargeted proof roster",
        &envelope_for(
            &proof_semantic,
            &proof_substitution,
            &proof_optimization,
            None,
        ),
        &retained,
    );
    assert!(
        verify_terminal_artifact_proof(&substituted, &AdmissionProfile::default()).is_err(),
        "a retargeted proof roster must reject at the verifier replay"
    );

    // A selected-pass roster on the optimization receipt is representable
    // under the same produced identities; the recomputed artifact identity
    // diverges and the retained manifest replay rejects it.
    let produced_semantic = terminal_psi_identity(&module).expect("produced module identity");
    let produced_proof = proof_bundle_fingerprint(&bundle).expect("produced proof fingerprint");
    let selected = PsiOptimizationSelections::new([PsiOptimization::ControlFlowCleanup])
        .expect("a unique pass roster");
    let record = PsiOptimizationExecutionRecord::new(
        selected,
        produced_semantic,
        produced_proof,
        produced_semantic,
        produced_proof,
    )
    .expect("a selected roster with unchanged products still forms a receipt");
    let substituted = divergent(
        "a selected roster on the identity-stage receipt",
        &envelope_for(
            &envelope[spans.semantic.clone()],
            &envelope[spans.proof.clone()],
            &encode_psi_optimization_execution_record(&record),
            None,
        ),
        &retained,
    );
    assert_eq!(
        substituted.optimization().selections(),
        record.selections(),
        "the substitution decodes to the claimed receipt"
    );

    // A different debug map bound to the same module is representable and
    // diverges the containing identity.
    let mut renamed = debug_map(&module);
    renamed.files[0].path = "renamed.omg".to_owned();
    let renamed_debug = encode_debug_map(&module, &renamed).expect("a renamed debug map encodes");
    let substituted = divergent(
        "a substituted debug payload",
        &envelope_for(
            &debug_envelope[debug_spans.semantic.clone()],
            &debug_envelope[debug_spans.proof.clone()],
            &debug_envelope[debug_spans.optimization.clone()],
            Some(&renamed_debug),
        ),
        &retained_debug,
    );
    assert_eq!(
        substituted.debug_bytes(),
        Some(renamed_debug.as_slice()),
        "the substitution decodes to the claimed debug section"
    );

    // The debug presence axis itself is representable in both directions:
    // dropping the section recomputes an artifact identical to the debug-free
    // artifact (still foreign to the debug-bearing retained manifest), and
    // adding the honestly encoded section to the debug-free envelope diverges
    // the same way.
    let dropped = divergent(
        "a dropped debug section",
        &envelope_for(
            &debug_envelope[debug_spans.semantic.clone()],
            &debug_envelope[debug_spans.proof.clone()],
            &debug_envelope[debug_spans.optimization.clone()],
            None,
        ),
        &retained_debug,
    );
    assert_eq!(
        dropped.manifest().identity(),
        retained.manifest().identity(),
        "dropping the debug section recomputes exactly the debug-free artifact"
    );
    let added = divergent(
        "an added debug section",
        &envelope_for(
            &envelope[spans.semantic.clone()],
            &envelope[spans.proof.clone()],
            &envelope[spans.optimization.clone()],
            Some(&debug_envelope[debug_spans.debug.clone().unwrap()]),
        ),
        &retained,
    );
    assert_eq!(
        added.manifest().identity(),
        retained_debug.manifest().identity(),
        "adding the honest debug section recomputes exactly the debug-bearing artifact"
    );
}
