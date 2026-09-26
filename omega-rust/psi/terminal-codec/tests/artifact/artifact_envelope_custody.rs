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
//! the retained manifest and the terminal verifier rejects. Every leg is
//! declared once in `artifact_envelope_custody_fields.rs` and driven by the
//! shared one-field substitution matrix.

use std::ops::Range;

#[path = "artifact_envelope_custody_fields.rs"]
mod artifact_envelope_custody_fields;

use super::{canonical_artifact, kernel_bundle, machine_id, obligation_id, semantic_module};
use artifact_envelope_custody_fields::{
    ArtifactEnvelopeFieldForTest, DebugBearingEnvelopeFieldForTest,
};
use optimization_core::{
    MutationOutcome, OneFieldSubstitutionMatrix, run_one_field_substitution_matrix,
};
use proof_admission::AdmissionProfile;
use terminal_codec::optimization::{PsiOptimization, PsiOptimizationSelections};
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

/// The family's combined independent checker result: canonical envelope
/// decoding first, then replay of the decoded sections against the retained
/// artifact manifest.
#[derive(Debug, PartialEq)]
enum EnvelopeCheck {
    Decode(CanonicalTerminalArtifactError),
    ManifestReplay(ArtifactManifestError),
}

/// The independent checker shared by both envelope families.
fn check_envelope(
    bytes: &[u8],
    retained: &CanonicalTerminalArtifact,
) -> Result<Vec<u8>, EnvelopeCheck> {
    let decoded = CanonicalTerminalArtifact::from_bytes(bytes).map_err(EnvelopeCheck::Decode)?;
    let module = decode_module(decoded.semantic_bytes())
        .expect("a decoded artifact's semantic section still decodes");
    let proof = decode_proof_section_for(&module, decoded.proof_bytes())
        .expect("a decoded artifact's proof section still decodes for its subject");
    validate_artifact_manifest(
        &module,
        &proof,
        decoded.optimization(),
        None,
        decoded.debug_bytes(),
        retained.manifest(),
    )
    .map_err(EnvelopeCheck::ManifestReplay)?;
    Ok(bytes.to_vec())
}

/// A representable substitution: the envelope still decodes to an internally
/// consistent artifact that re-encodes byte-exact and whose recomputed
/// identity diverges. The checker has already rejected it against the
/// retained manifest.
fn representable(
    field: impl std::fmt::Debug,
    mutated: &[u8],
    retained: &CanonicalTerminalArtifact,
) -> CanonicalTerminalArtifact {
    let decoded = CanonicalTerminalArtifact::from_bytes(mutated)
        .unwrap_or_else(|error| panic!("{field:?} must still decode: {error:?}"));
    assert_eq!(
        decoded.to_bytes(),
        mutated,
        "{field:?} must re-encode byte-exact"
    );
    decoded
        .validate()
        .unwrap_or_else(|error| panic!("{field:?} must stay internally consistent: {error:?}"));
    assert_ne!(
        decoded.manifest().identity(),
        retained.manifest().identity(),
        "{field:?} must diverge the honestly recomputed artifact identity"
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

/// The two retained envelopes and the honestly produced alternates their
/// substitution hooks draw from beyond each family's donor.
struct EnvelopeFixture {
    spans: EnvelopeSpans,
    debug_envelope: Vec<u8>,
    debug_spans: EnvelopeSpans,
    /// Sections resealed for the retained module under a proof bundle whose
    /// first evidence row names an obligation the module never raised.
    retargeted: (Vec<u8>, Vec<u8>, Vec<u8>),
    /// An identity-stage receipt claiming a selected pass roster over the
    /// retained module and bundle's unchanged products.
    selected_receipt: Vec<u8>,
}

impl EnvelopeFixture {
    fn section<'a>(bytes: &'a [u8], span: &Range<usize>) -> &'a [u8] {
        &bytes[span.clone()]
    }

    /// The debug-free family's honest-recomputation hook: rewrite exactly one
    /// envelope field, or replace payloads while every declared length
    /// honestly follows them. The donor is the consistent foreign artifact.
    fn substitute_envelope_for_test(
        &self,
        bytes: &mut Vec<u8>,
        field: ArtifactEnvelopeFieldForTest,
        donor: &[u8],
    ) {
        use ArtifactEnvelopeFieldForTest as Field;
        let spans = &self.spans;
        let donor_spans = envelope_spans(donor);
        let semantic = Self::section(bytes, &spans.semantic).to_vec();
        let proof = Self::section(bytes, &spans.proof).to_vec();
        let optimization = Self::section(bytes, &spans.optimization).to_vec();
        let length = |span: &Range<usize>| u64::try_from(span.len()).unwrap();
        *bytes = match field {
            Field::Magic => flip(bytes, spans.magic.start),
            Field::FormatMarker => substitute(bytes, &spans.format_marker, &3_u16.to_le_bytes()),
            Field::SemanticLengthCleared => set_len(bytes, &spans.semantic_len, 0),
            Field::SemanticLengthShortened => {
                set_len(bytes, &spans.semantic_len, length(&spans.semantic) - 1)
            }
            Field::SemanticLengthLengthened => {
                set_len(bytes, &spans.semantic_len, length(&spans.semantic) + 1)
            }
            Field::SemanticLengthOverLarge => set_len(bytes, &spans.semantic_len, u64::MAX),
            Field::ProofLengthCleared => set_len(bytes, &spans.proof_len, 0),
            Field::ProofLengthShortened => {
                set_len(bytes, &spans.proof_len, length(&spans.proof) - 1)
            }
            Field::ProofLengthLengthened => {
                set_len(bytes, &spans.proof_len, length(&spans.proof) + 1)
            }
            Field::ProofLengthOverLarge => set_len(bytes, &spans.proof_len, u64::MAX),
            Field::OptimizationLengthCleared => set_len(bytes, &spans.optimization_len, 0),
            Field::OptimizationLengthShortened => set_len(
                bytes,
                &spans.optimization_len,
                length(&spans.optimization) - 1,
            ),
            Field::OptimizationLengthLengthened => set_len(
                bytes,
                &spans.optimization_len,
                length(&spans.optimization) + 1,
            ),
            Field::OptimizationLengthOverLarge => set_len(bytes, &spans.optimization_len, u64::MAX),
            Field::DebugTagUnknown => substitute(bytes, &spans.debug_tag, &[2]),
            Field::DebugTagClaimedWithoutLength => substitute(bytes, &spans.debug_tag, &[1]),
            Field::SemanticMagic => flip(bytes, spans.semantic.start),
            Field::ProofMagic => flip(bytes, spans.proof.start),
            Field::OptimizationMagic => flip(bytes, spans.optimization.start),
            // The declared lengths honestly follow the payloads, so the
            // semantic slot now holds the sealed proof section.
            Field::ProofPayloadInSemanticSlot => {
                envelope_for(&proof, &semantic, &optimization, None)
            }
            // A dropped section keeps its declared length but surrenders its
            // bytes to the neighboring spans.
            Field::SemanticPayloadDropped => substitute(bytes, &spans.semantic, &[]),
            Field::ProofPayloadDropped => substitute(bytes, &spans.proof, &[]),
            Field::OptimizationPayloadDropped => bytes[..spans.optimization.start].to_vec(),
            Field::OptimizationPayloadDuplicated => {
                let mut mutated = bytes.clone();
                mutated.extend_from_slice(&optimization);
                mutated
            }
            Field::TrailingByte => {
                let mut mutated = bytes.clone();
                mutated.push(0);
                mutated
            }
            Field::SemanticPayloadForeign => envelope_for(
                Self::section(donor, &donor_spans.semantic),
                &proof,
                &optimization,
                None,
            ),
            Field::ProofPayloadForeign => envelope_for(
                &semantic,
                Self::section(donor, &donor_spans.proof),
                &optimization,
                None,
            ),
            Field::OptimizationPayloadForeign => envelope_for(
                &semantic,
                &proof,
                Self::section(donor, &donor_spans.optimization),
                None,
            ),
            Field::AllPayloadsForeign => envelope_for(
                Self::section(donor, &donor_spans.semantic),
                Self::section(donor, &donor_spans.proof),
                Self::section(donor, &donor_spans.optimization),
                None,
            ),
            Field::ProofRosterRetargeted => {
                let (semantic, proof, optimization) = &self.retargeted;
                envelope_for(semantic, proof, optimization, None)
            }
            Field::OptimizationRosterSelected => {
                envelope_for(&semantic, &proof, &self.selected_receipt, None)
            }
            Field::DebugSectionAdded => envelope_for(
                &semantic,
                &proof,
                &optimization,
                Some(Self::section(
                    &self.debug_envelope,
                    self.debug_spans.debug.as_ref().unwrap(),
                )),
            ),
        };
    }

    /// The debug-bearing family's honest-recomputation hook. The donor is the
    /// same artifact carrying a different debug map bound to the same module.
    fn substitute_debug_bearing_envelope_for_test(
        &self,
        bytes: &mut Vec<u8>,
        field: DebugBearingEnvelopeFieldForTest,
        donor: &[u8],
    ) {
        use DebugBearingEnvelopeFieldForTest as Field;
        let spans = &self.debug_spans;
        let debug = spans.debug.clone().unwrap();
        let debug_len = spans.debug_len.clone().unwrap();
        let debug_payload_len = u64::try_from(debug.len()).unwrap();
        let semantic = Self::section(bytes, &spans.semantic).to_vec();
        let proof = Self::section(bytes, &spans.proof).to_vec();
        let optimization = Self::section(bytes, &spans.optimization).to_vec();
        *bytes = match field {
            // A lengthened optimization declaration starves the debug read.
            Field::OptimizationLengthLengthened => set_len(
                bytes,
                &spans.optimization_len,
                u64::try_from(spans.optimization.len() + 1).unwrap(),
            ),
            Field::DebugTagDropped => substitute(bytes, &spans.debug_tag, &[0]),
            Field::DebugTagCorrupted => substitute(bytes, &spans.debug_tag, &[2]),
            Field::DebugLengthCleared => set_len(bytes, &debug_len, 0),
            Field::DebugLengthShortened => set_len(bytes, &debug_len, debug_payload_len - 1),
            Field::DebugLengthLengthened => set_len(bytes, &debug_len, debug_payload_len + 1),
            Field::DebugLengthOverLarge => set_len(bytes, &debug_len, u64::MAX),
            Field::DebugMagic => flip(bytes, debug.start),
            Field::DebugPayloadDropped => bytes[..debug.start].to_vec(),
            Field::DebugPayloadSubstituted => envelope_for(
                &semantic,
                &proof,
                &optimization,
                Some(Self::section(
                    donor,
                    envelope_spans(donor).debug.as_ref().unwrap(),
                )),
            ),
            Field::DebugSectionDropped => envelope_for(&semantic, &proof, &optimization, None),
        };
    }
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
    let semantic_len = spans.semantic.len();
    let proof_len = spans.proof.len();
    let optimization_len = spans.optimization.len();
    let debug_payload_len = debug_spans.debug.clone().unwrap().len();

    // A foreign module: the same machine shape under a different machine and
    // entry identity. Its complete consistent payload set is the debug-free
    // family's donor; its semantic section is canonical on its own.
    let mut foreign = semantic_module();
    foreign.machines[0].id = machine_id(7);
    foreign.entry = machine_id(7);
    let (foreign_semantic, foreign_proof, foreign_optimization) =
        consistent_sections(&foreign, &bundle);
    let donor = envelope_for(
        &foreign_semantic,
        &foreign_proof,
        &foreign_optimization,
        None,
    );

    // A different debug map bound to the same module: the debug-bearing
    // family's donor.
    let mut renamed = debug_map(&module);
    renamed.files[0].path = "renamed.omg".to_owned();
    let renamed_debug = encode_debug_map(&module, &renamed).expect("a renamed debug map encodes");
    let debug_donor = envelope_for(
        &debug_envelope[debug_spans.semantic.clone()],
        &debug_envelope[debug_spans.proof.clone()],
        &debug_envelope[debug_spans.optimization.clone()],
        Some(&renamed_debug),
    );

    // A proof-bundle substitution resealed for the retained module, and a
    // selected-pass roster on the identity-stage receipt under the same
    // produced identities.
    let mut retargeted_bundle = bundle.clone();
    retargeted_bundle.evidence[0].obligation = obligation_id(77);
    let produced_semantic = terminal_psi_identity(&module).expect("produced module identity");
    let produced_proof = proof_bundle_fingerprint(&bundle).expect("produced proof fingerprint");
    let selected = PsiOptimizationSelections::new([PsiOptimization::ControlFlowCleanup])
        .expect("a unique pass roster");
    let selected_record = PsiOptimizationExecutionRecord::new(
        selected,
        produced_semantic,
        produced_proof,
        produced_semantic,
        produced_proof,
    )
    .expect("a selected roster with unchanged products still forms a receipt");

    let fixture = EnvelopeFixture {
        spans,
        debug_envelope: debug_envelope.clone(),
        debug_spans,
        retargeted: consistent_sections(&module, &retargeted_bundle),
        selected_receipt: encode_psi_optimization_execution_record(&selected_record),
    };
    let spans = &fixture.spans;
    let debug_spans = &fixture.debug_spans;

    let foreign_identity = terminal_psi_identity(&foreign).expect("foreign module identity");
    let envelope_error = |error| {
        MutationOutcome::ExactError(EnvelopeCheck::Decode(
            CanonicalTerminalArtifactError::Envelope(error),
        ))
    };
    let decode = |error| MutationOutcome::ExactError(EnvelopeCheck::Decode(error));
    let replay_mismatch = || {
        MutationOutcome::ExactError(EnvelopeCheck::ManifestReplay(
            ArtifactManifestError::ManifestMismatch,
        ))
    };
    use CanonicalTerminalArtifactEnvelopeError as Envelope;

    let outcome = |field: ArtifactEnvelopeFieldForTest| -> MutationOutcome<EnvelopeCheck> {
        use ArtifactEnvelopeFieldForTest as Field;
        match field {
            // --- magic and format marker reject at decoding ---
            Field::Magic => envelope_error(Envelope::InvalidMagic),
            Field::FormatMarker => envelope_error(Envelope::UnsupportedFormatMarker(3)),
            // --- declared section lengths ---
            //
            // Every declared length is read before any payload is taken, so a
            // single-field length lie never reaches a section codec: a
            // shrunken declaration leaves the surrendered tail as trailing
            // bytes and a grown one starves a later section read.
            Field::SemanticLengthCleared => envelope_error(Envelope::TrailingBytes(semantic_len)),
            Field::ProofLengthCleared => envelope_error(Envelope::TrailingBytes(proof_len)),
            Field::OptimizationLengthCleared => {
                envelope_error(Envelope::TrailingBytes(optimization_len))
            }
            Field::SemanticLengthShortened
            | Field::ProofLengthShortened
            | Field::OptimizationLengthShortened
            | Field::TrailingByte => envelope_error(Envelope::TrailingBytes(1)),
            Field::SemanticLengthLengthened
            | Field::SemanticLengthOverLarge
            | Field::ProofLengthLengthened
            | Field::ProofLengthOverLarge
            | Field::OptimizationLengthLengthened
            | Field::OptimizationLengthOverLarge => envelope_error(Envelope::UnexpectedEnd),
            // --- debug presence tag ---
            Field::DebugTagUnknown => envelope_error(Envelope::InvalidDebugTag(2)),
            // Claiming presence without a declared debug length reads the
            // semantic magic as a u64 length and overruns the envelope.
            Field::DebugTagClaimedWithoutLength => envelope_error(Envelope::UnexpectedEnd),
            // --- payload bytes belong to their own section codecs ---
            Field::SemanticMagic => decode(CanonicalTerminalArtifactError::Semantic(
                CodecError::InvalidMagic,
            )),
            Field::ProofMagic => decode(CanonicalTerminalArtifactError::Proof(
                ProofCodecError::InvalidMagic,
            )),
            Field::OptimizationMagic => decode(CanonicalTerminalArtifactError::Optimization(
                PsiOptimizationExecutionRecordDecodeError::InvalidMagic,
            )),
            // A section payload moved out of its slot decodes under the wrong
            // codec.
            Field::ProofPayloadInSemanticSlot => decode(CanonicalTerminalArtifactError::Semantic(
                CodecError::InvalidMagic,
            )),
            // A dropped section surrenders its bytes to the neighboring
            // spans, so some later section read starves.
            Field::SemanticPayloadDropped
            | Field::ProofPayloadDropped
            | Field::OptimizationPayloadDropped => envelope_error(Envelope::UnexpectedEnd),
            // An extra section payload is trailing envelope content.
            Field::OptimizationPayloadDuplicated => {
                envelope_error(Envelope::TrailingBytes(optimization_len))
            }
            // --- cross-section joins reject non-canonical custody ---
            //
            // Substituting the semantic payload without resealing the joined
            // proof section rejects at the subject-binding decode: the
            // retained section is sealed to the original module identity. The
            // foreign-sealed proof section under the retained semantic payload
            // rejects at the same join in the other direction.
            Field::SemanticPayloadForeign => decode(CanonicalTerminalArtifactError::Proof(
                ProofCodecError::ProofSubjectMismatch {
                    claimed: produced_semantic,
                    reconstructed: foreign_identity,
                },
            )),
            Field::ProofPayloadForeign => decode(CanonicalTerminalArtifactError::Proof(
                ProofCodecError::ProofSubjectMismatch {
                    claimed: foreign_identity,
                    reconstructed: produced_semantic,
                },
            )),
            // The foreign identity-stage receipt under retained semantic and
            // proof payloads fails the produced-output binding inside
            // manifest building. The residual `NonCanonicalSections` byte
            // join stays defense in depth: every section decoder already
            // requires canonical re-encodings, so no single-field
            // substitution reaches it.
            Field::OptimizationPayloadForeign => decode(CanonicalTerminalArtifactError::Manifest(
                ArtifactManifestError::Optimization(
                    PsiOptimizationExecutionRecordError::OutputMismatch,
                ),
            )),
            // --- representable substitutions: honestly recomputed containing
            // identity, rejected by independent replay ---
            Field::AllPayloadsForeign
            | Field::ProofRosterRetargeted
            | Field::OptimizationRosterSelected
            | Field::DebugSectionAdded => replay_mismatch(),
        }
    };

    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "canonical terminal artifact envelope",
        fields: ArtifactEnvelopeFieldForTest::INVENTORY,
        honest: &|| envelope.clone(),
        donor: donor.clone(),
        custody: &|bytes: &Vec<u8>| bytes.clone(),
        substitute: &|bytes, field, donor| {
            fixture.substitute_envelope_for_test(bytes, field, donor);
        },
        check: &|bytes| check_envelope(bytes, &retained),
        outcome: &outcome,
        joined_replay: Some(&|bytes, field| {
            use ArtifactEnvelopeFieldForTest as Field;
            match field {
                // The complete consistent foreign payload set decodes to a
                // real artifact; the verifier replay rejects it as a
                // substitute for the retained artifact (the kernel bundle
                // still discharges the unchanged contract, so the semantic
                // verdict is unchanged even though custody diverged).
                Field::AllPayloadsForeign => {
                    let substituted = representable(field, bytes, &retained);
                    let verdict =
                        verify_terminal_artifact_proof(&substituted, &AdmissionProfile::default())
                            .expect(
                                "the foreign artifact still verifies under the unchanged contract",
                            );
                    assert_eq!(verdict.semantic_subject, foreign_identity);
                    assert_ne!(
                        verdict.semantic_subject,
                        retained.manifest().semantic(),
                        "the foreign artifact must not carry the retained semantic subject"
                    );
                }
                // The obligation retarget is representable and the verifier
                // replay refuses obligations the module never raised.
                Field::ProofRosterRetargeted => {
                    let substituted = representable(field, bytes, &retained);
                    assert!(
                        verify_terminal_artifact_proof(&substituted, &AdmissionProfile::default())
                            .is_err(),
                        "a retargeted proof roster must reject at the verifier replay"
                    );
                }
                Field::OptimizationRosterSelected => {
                    let substituted = representable(field, bytes, &retained);
                    assert_eq!(
                        substituted.optimization().selections(),
                        selected_record.selections(),
                        "the substitution decodes to the claimed receipt"
                    );
                }
                // Adding the honestly encoded section to the debug-free
                // envelope recomputes exactly the debug-bearing artifact.
                Field::DebugSectionAdded => {
                    let added = representable(field, bytes, &retained);
                    assert_eq!(
                        added.manifest().identity(),
                        retained_debug.manifest().identity(),
                        "adding the honest debug section recomputes exactly the debug-bearing artifact"
                    );
                }
                _ => {}
            }
        }),
    });

    let debug_outcome =
        |field: DebugBearingEnvelopeFieldForTest| -> MutationOutcome<EnvelopeCheck> {
            use DebugBearingEnvelopeFieldForTest as Field;
            match field {
                Field::OptimizationLengthLengthened
                | Field::DebugLengthLengthened
                | Field::DebugLengthOverLarge => envelope_error(Envelope::UnexpectedEnd),
                // Clearing presence strands the now unclaimed debug length
                // field and payload as trailing bytes.
                Field::DebugTagDropped => {
                    envelope_error(Envelope::TrailingBytes(8 + debug_payload_len))
                }
                Field::DebugTagCorrupted => envelope_error(Envelope::InvalidDebugTag(2)),
                Field::DebugLengthCleared => {
                    envelope_error(Envelope::TrailingBytes(debug_payload_len))
                }
                Field::DebugLengthShortened => envelope_error(Envelope::TrailingBytes(1)),
                Field::DebugMagic => decode(CanonicalTerminalArtifactError::Debug(
                    DebugMapError::InvalidMagic,
                )),
                Field::DebugPayloadDropped => envelope_error(Envelope::UnexpectedEnd),
                Field::DebugPayloadSubstituted | Field::DebugSectionDropped => replay_mismatch(),
            }
        };

    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "debug-bearing canonical terminal artifact envelope",
        fields: DebugBearingEnvelopeFieldForTest::INVENTORY,
        honest: &|| debug_envelope.clone(),
        donor: debug_donor,
        custody: &|bytes: &Vec<u8>| bytes.clone(),
        substitute: &|bytes, field, donor| {
            fixture.substitute_debug_bearing_envelope_for_test(bytes, field, donor);
        },
        check: &|bytes| check_envelope(bytes, &retained_debug),
        outcome: &debug_outcome,
        joined_replay: Some(&|bytes, field| {
            use DebugBearingEnvelopeFieldForTest as Field;
            match field {
                // A different debug map bound to the same module is
                // representable and diverges the containing identity.
                Field::DebugPayloadSubstituted => {
                    let substituted = representable(field, bytes, &retained_debug);
                    assert_eq!(
                        substituted.debug_bytes(),
                        Some(renamed_debug.as_slice()),
                        "the substitution decodes to the claimed debug section"
                    );
                }
                // Dropping the section recomputes an artifact identical to the
                // debug-free artifact, still foreign to the debug-bearing
                // retained manifest.
                Field::DebugSectionDropped => {
                    let dropped = representable(field, bytes, &retained_debug);
                    assert_eq!(
                        dropped.manifest().identity(),
                        retained.manifest().identity(),
                        "dropping the debug section recomputes exactly the debug-free artifact"
                    );
                }
                _ => {}
            }
        }),
    });

    // --- value sweeps beyond each lane's representative ---

    for offset in spans.magic.clone().skip(1) {
        reject(
            "an artifact-magic byte",
            &flip(&envelope, offset),
            CanonicalTerminalArtifactError::Envelope(Envelope::InvalidMagic),
        );
    }
    for marker in [0_u16, 1, u16::MAX] {
        reject(
            "the artifact format marker",
            &substitute(&envelope, &spans.format_marker, &marker.to_le_bytes()),
            CanonicalTerminalArtifactError::Envelope(Envelope::UnsupportedFormatMarker(marker)),
        );
    }
    reject(
        "an unknown debug presence tag",
        &substitute(&envelope, &spans.debug_tag, &[u8::MAX]),
        CanonicalTerminalArtifactError::Envelope(Envelope::InvalidDebugTag(u8::MAX)),
    );

    // Swapping the declared semantic and proof lengths changes two fields
    // but keeps the declared sum exact, so the shifted semantic span does
    // decode — into a truncated module the semantic codec rejects.
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

    // --- truncation rejects at the envelope ---

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
            CanonicalTerminalArtifactError::Envelope(Envelope::UnexpectedEnd),
        );
    }
    for cut in [
        debug_spans.debug.clone().unwrap().start + 1,
        debug_envelope.len() - 1,
    ] {
        reject(
            "a truncated debug-bearing envelope",
            &debug_envelope[..cut],
            CanonicalTerminalArtifactError::Envelope(Envelope::UnexpectedEnd),
        );
    }
    reject(
        "an empty envelope",
        &[],
        CanonicalTerminalArtifactError::Envelope(Envelope::UnexpectedEnd),
    );
}
