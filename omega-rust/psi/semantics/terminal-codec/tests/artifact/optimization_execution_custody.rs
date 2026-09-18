//! One-field mutation coverage for the canonical pre-Terminal optimization
//! execution receipt embedded in every published Terminal artifact.
//!
//! The receipt carries five representable content fields — the selected pass
//! roster plus input and output semantic/proof identities — and derives its
//! receipt identity from them at construction. A substitution either fails to
//! form a canonical record on the wire, or decodes to a different record whose
//! honestly recomputed identity diverges; independent replay then rejects it
//! through the published artifact-manifest join, and the produced-side
//! `validate_output` binding rejects output substitutions outright.

use super::{kernel_bundle, semantic_module};
use optimization::{PsiOptimization, PsiOptimizationSelections};
use proof_admission::AdmissionProfile;
use terminal_codec::{
    ArtifactManifestError, ProofBundleFingerprint, PsiOptimizationExecutionRecord,
    PsiOptimizationExecutionRecordDecodeError, PsiOptimizationExecutionRecordError,
    build_artifact_manifest, decode_psi_optimization_execution_record,
    encode_psi_optimization_execution_record, proof_bundle_fingerprint, terminal_psi_identity,
    validate_artifact_manifest,
};
use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};
use terminal_verifier::verify_module;

/// Byte offsets of every wire field inside the canonical record encoding.
struct RecordSpans {
    magic: std::ops::Range<usize>,
    format_marker: std::ops::Range<usize>,
    selections_len: std::ops::Range<usize>,
    selections: std::ops::Range<usize>,
    input_vocabulary: std::ops::Range<usize>,
    input_fingerprint: std::ops::Range<usize>,
    input_proof: std::ops::Range<usize>,
    output_vocabulary: std::ops::Range<usize>,
    output_fingerprint: std::ops::Range<usize>,
    output_proof: std::ops::Range<usize>,
}

fn record_spans(encoded: &[u8]) -> RecordSpans {
    let selections_len = usize::try_from(u64::from_le_bytes(
        encoded[10..18].try_into().expect("fixed length field"),
    ))
    .expect("record selections length fits usize");
    let selections = 18..18 + selections_len;
    let input_vocabulary = selections.end..selections.end + 2;
    let input_fingerprint = input_vocabulary.end..input_vocabulary.end + 32;
    let input_proof = input_fingerprint.end..input_fingerprint.end + 32;
    let output_vocabulary = input_proof.end..input_proof.end + 2;
    let output_fingerprint = output_vocabulary.end..output_vocabulary.end + 32;
    let output_proof = output_fingerprint.end..output_fingerprint.end + 32;
    RecordSpans {
        magic: 0..8,
        format_marker: 8..10,
        selections_len: 10..18,
        selections,
        input_vocabulary,
        input_fingerprint,
        input_proof,
        output_vocabulary,
        output_fingerprint,
        output_proof,
    }
}

fn semantic(byte: u8) -> TerminalPsiIdentity {
    TerminalPsiIdentity {
        vocabulary_marker: VocabularyMarker::CURRENT,
        program_fingerprint: SemanticFingerprint::from_bytes([byte; 32]),
    }
}

fn proof(byte: u8) -> ProofBundleFingerprint {
    ProofBundleFingerprint::from_bytes([byte; 32])
}

fn selected(passes: &[PsiOptimization]) -> PsiOptimizationSelections {
    PsiOptimizationSelections::new(passes.iter().copied()).expect("unique pass roster")
}

/// Substitute the encoded selections section and repair its length prefix.
fn splice_selections(encoded: &[u8], spans: &RecordSpans, roster: &[u8]) -> Vec<u8> {
    let mut mutated = Vec::with_capacity(18 + roster.len() + 132);
    mutated.extend_from_slice(&encoded[..spans.selections_len.start]);
    mutated.extend_from_slice(
        &u64::try_from(roster.len())
            .expect("selections fit u64")
            .to_le_bytes(),
    );
    mutated.extend_from_slice(roster);
    mutated.extend_from_slice(&encoded[spans.selections.end..]);
    mutated
}

#[test]
fn psi_optimization_execution_record_rejects_every_one_field_substitution() {
    let module = semantic_module();
    let bundle = kernel_bundle();
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("the fixture module verifies under its proof bundle");
    let produced_semantic = terminal_psi_identity(&module).expect("produced module identity");
    let produced_proof = proof_bundle_fingerprint(&bundle).expect("produced proof fingerprint");

    // The fixture receipt: a real selected roster, foreign input products, and
    // the honestly produced output identities so `validate_output` binds.
    let record = PsiOptimizationExecutionRecord::new(
        selected(&[
            PsiOptimization::ControlFlowCleanup,
            PsiOptimization::CopyPropagation,
        ]),
        semantic(0xA1),
        proof(0xB1),
        produced_semantic,
        produced_proof,
    )
    .expect("executed receipt forms");
    let encoded = encode_psi_optimization_execution_record(&record);
    let spans = record_spans(&encoded);
    assert_eq!(encoded.len(), spans.output_proof.end);
    assert_eq!(
        decode_psi_optimization_execution_record(&encoded),
        Ok(record.clone()),
        "the canonical record round-trips"
    );

    // The published artifact manifest binds the receipt identity: replaying a
    // retained manifest against a substituted receipt is the independent
    // replay join for input-side fields, while `validate_output` binds the
    // produced semantic and proof identities directly.
    let retained_manifest = build_artifact_manifest(
        &module,
        &bundle,
        &record,
        Some(b"installed-section"),
        Some(b"debug-section"),
    )
    .expect("retained artifact manifest");
    assert_eq!(retained_manifest.optimization(), record.identity());

    let divergent = |name: &'static str,
                     mutated: &[u8],
                     expect_output_mismatch: bool|
     -> PsiOptimizationExecutionRecord {
        let substituted = decode_psi_optimization_execution_record(mutated)
            .unwrap_or_else(|error| panic!("{name} must still decode: {error:?}"));
        assert_ne!(substituted, record, "{name} must change the receipt");
        assert_eq!(
            encode_psi_optimization_execution_record(&substituted),
            mutated,
            "{name} must re-encode canonically"
        );
        assert_ne!(
            substituted.identity(),
            record.identity(),
            "{name} must diverge the honestly recomputed receipt identity"
        );
        if expect_output_mismatch {
            assert_eq!(
                substituted.validate_output(produced_semantic, produced_proof),
                Err(PsiOptimizationExecutionRecordError::OutputMismatch),
                "{name} must reject at the produced-output binding"
            );
        } else {
            assert_eq!(
                substituted.validate_output(produced_semantic, produced_proof),
                Ok(()),
                "{name} keeps the produced-output binding"
            );
        }
        match validate_artifact_manifest(
            &module,
            &bundle,
            &substituted,
            Some(b"installed-section"),
            Some(b"debug-section"),
            retained_manifest,
        ) {
            Err(ArtifactManifestError::ManifestMismatch) => {}
            Err(ArtifactManifestError::Optimization(
                PsiOptimizationExecutionRecordError::OutputMismatch,
            )) if expect_output_mismatch => {}
            other => panic!("{name} must reject at the manifest replay: {other:?}"),
        }
        substituted
    };

    // --- selections roster: substitute, extend, and drop members ---

    for (name, roster) in [
        (
            "a substituted roster member",
            selected(&[
                PsiOptimization::GlobalValueNumbering,
                PsiOptimization::CopyPropagation,
            ]),
        ),
        (
            "an extended roster",
            selected(&[
                PsiOptimization::ControlFlowCleanup,
                PsiOptimization::CopyPropagation,
                PsiOptimization::DeadPureScalarElimination,
            ]),
        ),
        (
            "a dropped roster member",
            selected(&[PsiOptimization::ControlFlowCleanup]),
        ),
    ] {
        divergent(
            name,
            &splice_selections(&encoded, &spans, &roster.encode()),
            false,
        );
    }

    // The whole roster clears only for the identity stage: a record claiming
    // changed products with no selected pass is rejected at construction and
    // at canonical decoding.
    assert_eq!(
        PsiOptimizationExecutionRecord::new(
            PsiOptimizationSelections::default(),
            semantic(0xA1),
            proof(0xB1),
            produced_semantic,
            produced_proof,
        ),
        Err(PsiOptimizationExecutionRecordError::IdentityStageChangedProduct)
    );
    let cleared = splice_selections(
        &encoded,
        &spans,
        &PsiOptimizationSelections::default().encode(),
    );
    assert_eq!(
        decode_psi_optimization_execution_record(&cleared),
        Err(PsiOptimizationExecutionRecordDecodeError::InvalidRecord(
            PsiOptimizationExecutionRecordError::IdentityStageChangedProduct
        )),
        "an empty roster claiming changed outputs must reject at decoding"
    );

    // --- input-side identities: representable, replay-bound through the
    // manifest identity ---

    for (name, range) in [
        (
            "the input program fingerprint",
            spans.input_fingerprint.clone(),
        ),
        ("the input proof fingerprint", spans.input_proof.clone()),
    ] {
        let mut mutated = encoded.clone();
        mutated[range.start] ^= 0xFF;
        divergent(name, &mutated, false);
    }

    // A zero fingerprint stays representable (the receipt imposes no nonzero
    // rule); it still decodes to a different record and rejects at replay.
    let mut mutated = encoded.clone();
    mutated[spans.input_fingerprint.clone()].copy_from_slice(&[0; 32]);
    divergent("a zero input program fingerprint", &mutated, false);

    // --- output-side identities: representable, rejected by the produced
    // output binding before the manifest join is even consulted ---

    for (name, range) in [
        (
            "the output program fingerprint",
            spans.output_fingerprint.clone(),
        ),
        ("the output proof fingerprint", spans.output_proof.clone()),
    ] {
        let mut mutated = encoded.clone();
        mutated[range.start] ^= 0xFF;
        divergent(name, &mutated, true);
    }

    // --- vocabulary markers admit only the current vocabulary ---

    for (name, range) in [
        (
            "the input vocabulary marker",
            spans.input_vocabulary.clone(),
        ),
        (
            "the output vocabulary marker",
            spans.output_vocabulary.clone(),
        ),
    ] {
        let mut mutated = encoded.clone();
        mutated[range].copy_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(
            decode_psi_optimization_execution_record(&mutated),
            Err(PsiOptimizationExecutionRecordDecodeError::UnsupportedVocabularyMarker(u16::MAX)),
            "{name} substitution must reject at decoding"
        );
    }

    // --- framing axes reject at decoding ---

    let mut mutated = encoded.clone();
    mutated[spans.magic.start] ^= 0xFF;
    assert_eq!(
        decode_psi_optimization_execution_record(&mutated),
        Err(PsiOptimizationExecutionRecordDecodeError::InvalidMagic)
    );

    let mut mutated = encoded.clone();
    mutated[spans.format_marker.clone()].copy_from_slice(&u16::MAX.to_le_bytes());
    assert_eq!(
        decode_psi_optimization_execution_record(&mutated),
        Err(PsiOptimizationExecutionRecordDecodeError::UnsupportedFormatMarker(u16::MAX))
    );

    // A lying selections length either starves the inner codec or overruns the
    // record.
    let mut mutated = encoded.clone();
    mutated[spans.selections_len.clone()].copy_from_slice(&0_u64.to_le_bytes());
    assert_eq!(
        decode_psi_optimization_execution_record(&mutated),
        Err(PsiOptimizationExecutionRecordDecodeError::Selection(
            optimization::PsiOptimizationSelectionDecodeError::Truncated
        ))
    );
    let mut mutated = encoded.clone();
    mutated[spans.selections_len.clone()].copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        decode_psi_optimization_execution_record(&mutated),
        Err(PsiOptimizationExecutionRecordDecodeError::UnexpectedEnd)
    );

    // --- inner selections codec axes reject through the record decode ---

    let inner = |patch: &dyn Fn(&mut Vec<u8>)| -> Vec<u8> {
        let mut inner = encoded[spans.selections.clone()].to_vec();
        patch(&mut inner);
        splice_selections(&encoded, &spans, &inner)
    };

    for (name, roster, expected) in [
        (
            "the selections magic",
            inner(&|inner| inner[0] ^= 0xFF),
            optimization::PsiOptimizationSelectionDecodeError::WrongMagic,
        ),
        (
            "the selections version",
            inner(&|inner| inner[8..12].copy_from_slice(&u32::MAX.to_le_bytes())),
            optimization::PsiOptimizationSelectionDecodeError::UnsupportedVersion(u32::MAX),
        ),
        (
            "an unknown pass tag",
            inner(&|inner| inner[16] = 0xFE),
            optimization::PsiOptimizationSelectionDecodeError::UnknownTag(0xFE),
        ),
        (
            "a duplicated pass",
            inner(&|inner| inner[17] = inner[16]),
            optimization::PsiOptimizationSelectionDecodeError::Duplicate(
                PsiOptimization::ControlFlowCleanup,
            ),
        ),
        (
            "a noncanonical roster order",
            inner(&|inner| inner[16..].reverse()),
            optimization::PsiOptimizationSelectionDecodeError::NonCanonicalOrder,
        ),
    ] {
        assert_eq!(
            decode_psi_optimization_execution_record(&roster),
            Err(PsiOptimizationExecutionRecordDecodeError::Selection(
                expected
            )),
            "{name} must reject at decoding"
        );
    }

    // A count lying about its roster starves or strands the inner codec.
    let under = inner(&|inner| inner[12..16].copy_from_slice(&1_u32.to_le_bytes()));
    assert_eq!(
        decode_psi_optimization_execution_record(&under),
        Err(PsiOptimizationExecutionRecordDecodeError::Selection(
            optimization::PsiOptimizationSelectionDecodeError::TrailingBytes
        ))
    );
    let over = inner(&|inner| inner[12..16].copy_from_slice(&3_u32.to_le_bytes()));
    assert_eq!(
        decode_psi_optimization_execution_record(&over),
        Err(PsiOptimizationExecutionRecordDecodeError::Selection(
            optimization::PsiOptimizationSelectionDecodeError::Truncated
        ))
    );

    // --- truncation and trailing bytes reject at decoding ---

    for cut in [
        spans.magic.end - 1,
        spans.format_marker.end - 1,
        spans.selections_len.end - 1,
        spans.selections.end - 1,
        spans.input_vocabulary.end - 1,
        spans.input_fingerprint.end - 1,
        spans.input_proof.end - 1,
        spans.output_vocabulary.end - 1,
        spans.output_fingerprint.end - 1,
        spans.output_proof.end - 1,
    ] {
        assert!(
            decode_psi_optimization_execution_record(&encoded[..cut]).is_err(),
            "truncation at byte {cut} must reject"
        );
    }

    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        decode_psi_optimization_execution_record(&trailing),
        Err(PsiOptimizationExecutionRecordDecodeError::TrailingBytes(1))
    );

    // --- the identity-stage receipt rejects output substitutions at
    // construction and still binds at replay ---

    let identity_record =
        terminal_codec::build_identity_optimization_execution_record(&module, &bundle)
            .expect("identity-stage receipt");
    assert!(identity_record.selections().is_empty());
    assert_eq!(identity_record.input_semantic(), produced_semantic);
    assert_eq!(identity_record.output_proof(), produced_proof);

    let identity_encoded = encode_psi_optimization_execution_record(&identity_record);
    let identity_spans = record_spans(&identity_encoded);
    let identity_manifest = build_artifact_manifest(
        &module,
        &bundle,
        &identity_record,
        Some(b"installed-section"),
        Some(b"debug-section"),
    )
    .expect("identity-stage retained manifest");

    // On the identity stage, selecting a pass is the only independently
    // representable content substitution: the receipt still forms with an
    // honestly recomputed identity and rejects at the retained manifest.
    let identity_mutated = splice_selections(
        &identity_encoded,
        &identity_spans,
        &selected(&[PsiOptimization::ProofCheckElision]).encode(),
    );
    let substituted = decode_psi_optimization_execution_record(&identity_mutated)
        .expect("a selected roster on the identity stage still decodes");
    assert_ne!(substituted.identity(), identity_record.identity());
    assert_eq!(
        validate_artifact_manifest(
            &module,
            &bundle,
            &substituted,
            Some(b"installed-section"),
            Some(b"debug-section"),
            identity_manifest,
        ),
        Err(ArtifactManifestError::ManifestMismatch),
        "a roster substitution on the identity-stage receipt must reject at replay"
    );

    // Every product-field substitution on the identity stage is
    // non-canonical: the empty roster cannot claim changed products, so the
    // wire form rejects at construction inside decoding rather than forming a
    // divergent record.
    for range in [
        identity_spans.input_fingerprint.clone(),
        identity_spans.input_proof.clone(),
        identity_spans.output_fingerprint.clone(),
        identity_spans.output_proof.clone(),
    ] {
        let mut mutated = identity_encoded.clone();
        mutated[range.start] ^= 0xFF;
        assert_eq!(
            decode_psi_optimization_execution_record(&mutated),
            Err(PsiOptimizationExecutionRecordDecodeError::InvalidRecord(
                PsiOptimizationExecutionRecordError::IdentityStageChangedProduct
            )),
            "an identity-stage product substitution must reject at decoding"
        );
    }
}
