//! One-field mutation coverage for the canonical Terminal debug-map section.
//!
//! The debug map is replaceable presentation evidence bound into the artifact
//! manifest through the `debug` section fingerprint. Its wire fields — the
//! sealed semantic identity, the counted source-file roster (file identity,
//! origin, byte length, digest, path), and the counted site roster (subject,
//! span file, span start, span end) — are each substituted independently. A
//! substitution either fails canonical decoding (the decoder replays
//! `validate_debug_map` against the module and re-encodes the decoded map), or
//! decodes to a different map whose honestly recomputed manifest identity
//! diverges and whose replay against the retained manifest rejects.

use std::ops::Range;

use super::{
    canonical_artifact, contract_id, edge_id, kernel_bundle, machine_id, obligation_id,
    operation_id, semantic_module, value_id,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::ClaimId;
use terminal_codec::{
    ArtifactManifestError, CanonicalTerminalArtifact, CanonicalTerminalArtifactError, CodecError,
    DebugFileId, DebugMapError, DebugSite, DebugSourceFile, DebugSourceOrigin, DebugSourceSpan,
    DebugSubject, TerminalDebugMap, build_artifact_manifest, decode_debug_map, encode_debug_map,
    source_digest, terminal_psi_identity, validate_artifact_manifest, validate_debug_map,
};
use terminal_psi::TerminalModule;
use terminal_verifier::verify_module;

/// Byte offsets of every wire field inside the canonical section encoding.
struct MapSpans {
    magic: Range<usize>,
    format_marker: Range<usize>,
    vocabulary: Range<usize>,
    fingerprint: Range<usize>,
    file_count: Range<usize>,
    files: Vec<FileSpan>,
    site_count: Range<usize>,
    sites: Vec<SiteSpan>,
    end: usize,
}

struct FileSpan {
    row: Range<usize>,
    id: Range<usize>,
    origin: Range<usize>,
    byte_len: Range<usize>,
    digest: Range<usize>,
    path_len: Range<usize>,
    path: Range<usize>,
}

struct SiteSpan {
    row: Range<usize>,
    subject_tag: Range<usize>,
    subject_id: Range<usize>,
    file: Range<usize>,
    start: Range<usize>,
    end: Range<usize>,
}

fn map_spans(encoded: &[u8]) -> MapSpans {
    let u32_at = |offset: usize| -> usize {
        u32::from_le_bytes(encoded[offset..offset + 4].try_into().expect("u32 field"))
            .try_into()
            .expect("count fits usize")
    };
    let file_count = u32_at(44);
    let mut cursor = 48;
    let mut files = Vec::with_capacity(file_count);
    for _ in 0..file_count {
        let row = cursor;
        let path_len = u32_at(cursor + 45);
        let path = cursor + 49..cursor + 49 + path_len;
        files.push(FileSpan {
            id: cursor..cursor + 4,
            origin: cursor + 4..cursor + 5,
            byte_len: cursor + 5..cursor + 13,
            digest: cursor + 13..cursor + 45,
            path_len: cursor + 45..cursor + 49,
            row: row..path.end,
            path: path.clone(),
        });
        cursor = path.end;
    }
    let site_count = cursor..cursor + 4;
    cursor += 4;
    let site_total = u32_at(site_count.start);
    let mut sites = Vec::with_capacity(site_total);
    for _ in 0..site_total {
        let row = cursor;
        // Every subject kind in the fixture carries exactly one u64 identity;
        // the two-identity Claim row is exercised through a splice below.
        let subject_id = cursor + 1..cursor + 9;
        let site_end = cursor + 29;
        sites.push(SiteSpan {
            subject_tag: cursor..cursor + 1,
            subject_id,
            file: cursor + 9..cursor + 13,
            start: cursor + 13..cursor + 21,
            end: cursor + 21..site_end,
            row: row..site_end,
        });
        cursor = site_end;
    }
    MapSpans {
        magic: 0..8,
        format_marker: 8..10,
        vocabulary: 10..12,
        fingerprint: 12..44,
        file_count: 44..48,
        files,
        site_count,
        sites,
        end: cursor,
    }
}

fn file_id(raw: u32) -> DebugFileId {
    DebugFileId::new(raw).expect("nonzero file identity")
}

/// Three source files (the third deliberately unreferenced) and six sites
/// spanning seven subject kinds over the shared fixture module.
fn debug_map(module: &TerminalModule) -> TerminalDebugMap {
    TerminalDebugMap {
        semantic: terminal_psi_identity(module).expect("module identity"),
        files: vec![
            DebugSourceFile {
                id: file_id(1),
                origin: DebugSourceOrigin::User,
                byte_len: 15,
                digest: source_digest(b"machine main {}"),
                path: "main.omg".to_owned(),
            },
            DebugSourceFile {
                id: file_id(2),
                origin: DebugSourceOrigin::Toolchain,
                byte_len: 64,
                digest: source_digest(b"// toolchain prelude"),
                path: "toolchain/prelude.omg".to_owned(),
            },
            DebugSourceFile {
                id: file_id(3),
                origin: DebugSourceOrigin::Toolchain,
                byte_len: 9,
                digest: source_digest(b"generated"),
                path: "generated.omg".to_owned(),
            },
        ],
        sites: vec![
            DebugSite {
                subject: DebugSubject::Machine(machine_id(1)),
                span: DebugSourceSpan {
                    file: file_id(1),
                    start: 0,
                    end: 7,
                },
            },
            DebugSite {
                subject: DebugSubject::Operation(operation_id(1)),
                span: DebugSourceSpan {
                    file: file_id(1),
                    start: 8,
                    end: 12,
                },
            },
            DebugSite {
                subject: DebugSubject::Edge(edge_id(1)),
                span: DebugSourceSpan {
                    file: file_id(2),
                    start: 0,
                    end: 5,
                },
            },
            DebugSite {
                subject: DebugSubject::Value(value_id(2)),
                span: DebugSourceSpan {
                    file: file_id(1),
                    start: 8,
                    end: 15,
                },
            },
            DebugSite {
                subject: DebugSubject::Contract(contract_id(1)),
                span: DebugSourceSpan {
                    file: file_id(1),
                    start: 0,
                    end: 15,
                },
            },
            DebugSite {
                subject: DebugSubject::Obligation(obligation_id(1)),
                span: DebugSourceSpan {
                    file: file_id(1),
                    start: 8,
                    end: 15,
                },
            },
        ],
    }
}

/// Splice a replacement file row into the roster, repairing the count prefix.
fn splice_file_row(encoded: &[u8], spans: &MapSpans, index: usize, row: &[u8]) -> Vec<u8> {
    let mut mutated = Vec::with_capacity(encoded.len() + row.len() - spans.files[index].row.len());
    mutated.extend_from_slice(&encoded[..spans.file_count.start]);
    mutated.extend_from_slice(
        &u32::try_from(spans.files.len() - 1 + usize::from(!row.is_empty()))
            .expect("file count fits u32")
            .to_le_bytes(),
    );
    mutated.extend_from_slice(&encoded[spans.file_count.end..spans.files[index].row.start]);
    mutated.extend_from_slice(row);
    mutated.extend_from_slice(&encoded[spans.files[index].row.end..]);
    mutated
}

/// Splice a replacement site row into the roster, repairing the count prefix.
fn splice_site_row(encoded: &[u8], spans: &MapSpans, index: usize, row: &[u8]) -> Vec<u8> {
    let mut mutated = Vec::with_capacity(encoded.len() + row.len() - spans.sites[index].row.len());
    mutated.extend_from_slice(&encoded[..spans.site_count.start]);
    mutated.extend_from_slice(
        &u32::try_from(spans.sites.len() - 1 + usize::from(!row.is_empty()))
            .expect("site count fits u32")
            .to_le_bytes(),
    );
    mutated.extend_from_slice(&encoded[spans.site_count.end..spans.sites[index].row.start]);
    mutated.extend_from_slice(row);
    mutated.extend_from_slice(&encoded[spans.sites[index].row.end..]);
    mutated
}

/// The byte range a fabricated Claim subject row occupies inside site 5's row:
/// the tag byte plus two u64 identities in place of the one-identity subject.
fn claim_subject_row(machine: u64, claim: u64) -> [u8; 17] {
    let mut row = [0_u8; 17];
    row[0] = 9;
    row[1..9].copy_from_slice(&machine.to_le_bytes());
    row[9..17].copy_from_slice(&claim.to_le_bytes());
    row
}

#[test]
fn terminal_debug_map_rejects_every_one_field_substitution() {
    let module = semantic_module();
    let bundle = kernel_bundle();
    verify_module(&module, &bundle, &AdmissionProfile::default())
        .expect("the fixture module verifies under its proof bundle");
    let map = debug_map(&module);
    let encoded = encode_debug_map(&module, &map).expect("canonical debug section");
    let spans = map_spans(&encoded);
    assert_eq!(encoded.len(), spans.end, "span map must cover the section");
    assert_eq!(
        decode_debug_map(&module, &encoded),
        Ok(map.clone()),
        "the canonical section round-trips"
    );

    // The retained published artifact binds the section fingerprint into its
    // manifest: replaying that manifest against a substituted section is the
    // independent replay join for every representable field.
    let artifact = canonical_artifact(&module, &bundle, Some(&map));
    let retained = artifact.manifest();
    assert_eq!(
        retained.debug().map(|fingerprint| *fingerprint.as_bytes()),
        build_artifact_manifest(
            &module,
            &bundle,
            artifact.optimization(),
            None,
            Some(&encoded),
        )
        .expect("honest manifest over the retained section")
        .debug()
        .map(|fingerprint| *fingerprint.as_bytes()),
    );

    // A substitution that still forms a canonical section decodes to a
    // different map, honestly recomputes a divergent artifact identity, and is
    // rejected by replay against the retained manifest.
    let representable = |name: &'static str, mutated: &[u8]| -> TerminalDebugMap {
        let substituted = decode_debug_map(&module, mutated)
            .unwrap_or_else(|error| panic!("{name} must still decode: {error:?}"));
        assert_ne!(substituted, map, "{name} must change the map");
        assert_eq!(
            encode_debug_map(&module, &substituted),
            Ok(mutated.to_vec()),
            "{name} must re-encode canonically"
        );
        assert_eq!(
            validate_debug_map(&module, &substituted),
            Ok(()),
            "{name} must still satisfy the module-bound map invariants"
        );
        let recomputed = build_artifact_manifest(
            &module,
            &bundle,
            artifact.optimization(),
            None,
            Some(mutated),
        )
        .expect("honest manifest for the substituted section");
        assert_ne!(
            recomputed.identity(),
            retained.identity(),
            "{name} must diverge the recomputed artifact identity"
        );
        assert_eq!(
            validate_artifact_manifest(
                &module,
                &bundle,
                artifact.optimization(),
                None,
                Some(mutated),
                retained,
            ),
            Err(ArtifactManifestError::ManifestMismatch),
            "{name} must reject at the retained-manifest replay"
        );
        substituted
    };
    let rejected = |name: &'static str, mutated: &[u8], expected: DebugMapError| {
        assert_eq!(
            decode_debug_map(&module, mutated),
            Err(expected),
            "{name} must reject at canonical decoding"
        );
    };
    let rejected_unspecified = |name: &'static str, mutated: &[u8]| {
        assert!(
            decode_debug_map(&module, mutated).is_err(),
            "{name} must reject at canonical decoding"
        );
    };

    // --- sealed semantic identity: the vocabulary marker and program
    // fingerprint bind the map to one exact module ---

    let mut mutated = encoded.clone();
    mutated[spans.vocabulary.clone()].copy_from_slice(&u16::MAX.to_le_bytes());
    rejected(
        "the vocabulary marker",
        &mutated,
        DebugMapError::UnsupportedVocabularyMarker(u16::MAX),
    );

    let mut mutated = encoded.clone();
    mutated[spans.fingerprint.start] ^= 0xFF;
    assert!(
        matches!(
            decode_debug_map(&module, &mutated),
            Err(DebugMapError::SemanticIdentityMismatch { .. })
        ),
        "a foreign program fingerprint must reject at decoding"
    );

    let mut foreign = module.clone();
    foreign.machines[0].id = machine_id(7);
    foreign.entry = machine_id(7);
    assert!(
        matches!(
            decode_debug_map(&foreign, &encoded),
            Err(DebugMapError::SemanticIdentityMismatch { .. })
        ),
        "the retained section must reject under a foreign module"
    );

    // --- framing axes reject at decoding ---

    let mut mutated = encoded.clone();
    mutated[spans.magic.start] ^= 0xFF;
    rejected("the magic", &mutated, DebugMapError::InvalidMagic);

    let mut mutated = encoded.clone();
    mutated[spans.format_marker.clone()].copy_from_slice(&u16::MAX.to_le_bytes());
    rejected(
        "the format marker",
        &mutated,
        DebugMapError::UnsupportedFormatMarker(u16::MAX),
    );

    let mut trailing = encoded.clone();
    trailing.push(0);
    rejected(
        "a trailing byte",
        &trailing,
        DebugMapError::TrailingBytes(1),
    );

    for cut in [
        spans.magic.end - 1,
        spans.format_marker.end - 1,
        spans.vocabulary.end - 1,
        spans.fingerprint.end - 1,
        spans.file_count.end - 1,
        spans.files[0].row.end - 1,
        spans.site_count.end - 1,
        spans.sites[0].row.end - 1,
        spans.end - 1,
    ] {
        rejected_unspecified("a truncated section", &encoded[..cut]);
    }

    // --- file roster: origin, byte length, digest, and path are
    // independently representable; the file identity is order- and join-bound
    // ---

    let mut mutated = encoded.clone();
    mutated[spans.files[0].origin.clone()].copy_from_slice(&[2]);
    representable("a file's source origin", &mutated);

    let mut mutated = encoded.clone();
    mutated[spans.files[0].byte_len.clone()].copy_from_slice(&16_u64.to_le_bytes());
    representable("a file's byte length", &mutated);

    let mut mutated = encoded.clone();
    mutated[spans.files[0].digest.start] ^= 0xFF;
    representable("a file's source digest", &mutated);

    // A different-length path repairs its u32 prefix and still decodes.
    let file_row = |path: &str| -> Vec<u8> {
        let mut row = Vec::new();
        row.extend_from_slice(&1_u32.to_le_bytes());
        row.push(1);
        row.extend_from_slice(&15_u64.to_le_bytes());
        row.extend_from_slice(source_digest(b"machine main {}").as_bytes());
        row.extend_from_slice(
            &u32::try_from(path.len())
                .expect("path fits u32")
                .to_le_bytes(),
        );
        row.extend_from_slice(path.as_bytes());
        row
    };
    representable(
        "a file's presentation path",
        &splice_file_row(&encoded, &spans, 0, &file_row("renamed/main.omg")),
    );

    // An unreferenced roster member drops out representably; an inserted,
    // order-preserving file row is representable too.
    representable(
        "a dropped unreferenced file row",
        &splice_file_row(&encoded, &spans, 2, &[]),
    );
    let mut extra_file = Vec::new();
    extra_file.extend_from_slice(&4_u32.to_le_bytes());
    extra_file.push(2);
    extra_file.extend_from_slice(&3_u64.to_le_bytes());
    extra_file.extend_from_slice(source_digest(b"tmp").as_bytes());
    extra_file.extend_from_slice(&9_u32.to_le_bytes());
    extra_file.extend_from_slice(b"extra.omg");
    let mut inserted = Vec::with_capacity(encoded.len() + extra_file.len());
    inserted.extend_from_slice(&encoded[..spans.file_count.start]);
    inserted.extend_from_slice(&4_u32.to_le_bytes());
    inserted.extend_from_slice(&encoded[spans.file_count.end..spans.files[2].row.end]);
    inserted.extend_from_slice(&extra_file);
    inserted.extend_from_slice(&encoded[spans.files[2].row.end..]);
    representable("an inserted file row", &inserted);

    // A referenced file cannot drop out: the site roster's span-file join
    // rejects at decoding.
    rejected(
        "a dropped referenced file row",
        &splice_file_row(&encoded, &spans, 1, &[]),
        DebugMapError::UnknownFile(file_id(2)),
    );

    // File identities admit no representable substitution: a zero identity
    // rejects outright, and any other value breaks the strictly increasing
    // roster or strands the spans that join it.
    let mut mutated = encoded.clone();
    mutated[spans.files[0].id.clone()].copy_from_slice(&0_u32.to_le_bytes());
    rejected(
        "a zero file identity",
        &mutated,
        DebugMapError::ZeroFileIdentity,
    );
    let mut mutated = encoded.clone();
    mutated[spans.files[1].id.clone()].copy_from_slice(&1_u32.to_le_bytes());
    rejected(
        "a duplicated file identity",
        &mutated,
        DebugMapError::NonCanonicalOrder("debug files by DebugFileId"),
    );
    let mut mutated = encoded.clone();
    mutated[spans.files[0].id.clone()].copy_from_slice(&5_u32.to_le_bytes());
    rejected(
        "a file identity breaking roster order",
        &mutated,
        DebugMapError::NonCanonicalOrder("debug files by DebugFileId"),
    );

    let mut mutated = encoded.clone();
    mutated[spans.files[0].origin.clone()].copy_from_slice(&[3]);
    rejected(
        "an unknown source-origin tag",
        &mutated,
        DebugMapError::InvalidTag("DebugSourceOrigin", 3),
    );

    // A shrinking byte length strands the spans that index the file.
    let mut mutated = encoded.clone();
    mutated[spans.files[0].byte_len.clone()].copy_from_slice(&10_u64.to_le_bytes());
    rejected(
        "a byte length stranding spans",
        &mutated,
        DebugMapError::InvalidSpan(DebugSourceSpan {
            file: file_id(1),
            start: 8,
            end: 12,
        }),
    );

    let mut mutated = encoded.clone();
    mutated[spans.files[0].path.clone()].copy_from_slice(&[0xFF; 8]);
    rejected(
        "a non-UTF-8 path",
        &mutated,
        DebugMapError::Codec(CodecError::InvalidUtf8("debug source path")),
    );

    let mut mutated = encoded.clone();
    mutated[spans.files[0].path_len.clone()].copy_from_slice(&((1_u32 << 20) + 1).to_le_bytes());
    rejected(
        "an over-long path",
        &mutated,
        DebugMapError::Codec(CodecError::StringTooLong("debug source path")),
    );

    // A count lying about its roster starves or strands the cursor.
    let mut mutated = encoded.clone();
    mutated[spans.file_count.clone()].copy_from_slice(&0_u32.to_le_bytes());
    rejected_unspecified("a cleared file count", &mutated);
    let mut mutated = encoded.clone();
    mutated[spans.file_count.clone()].copy_from_slice(&7_u32.to_le_bytes());
    rejected_unspecified("an over-counted file roster", &mutated);

    // --- site roster: subject, span file, and span bounds are independently
    // representable where they keep the roster order and the file join ---

    // A subject-kind substitution that preserves order and names an existing
    // subject: Machine(1) -> Block(1) sorts before Operation(1).
    let mut mutated = encoded.clone();
    mutated[spans.sites[0].subject_tag.clone()].copy_from_slice(&[2]);
    representable("a site's subject kind", &mutated);

    // An identity substitution inside one kind: Value(2) -> Value(1) keeps
    // Edge(1) < Value(1) < Contract(1).
    let mut mutated = encoded.clone();
    mutated[spans.sites[3].subject_id.clone()].copy_from_slice(&1_u64.to_le_bytes());
    representable("a site's subject identity", &mutated);

    let mut mutated = encoded.clone();
    mutated[spans.sites[0].start.clone()].copy_from_slice(&4_u64.to_le_bytes());
    representable("a site's span start", &mutated);

    let mut mutated = encoded.clone();
    mutated[spans.sites[0].end.clone()].copy_from_slice(&10_u64.to_le_bytes());
    representable("a site's span end", &mutated);

    // Rebinding the span to another rostered file keeps every join.
    let mut mutated = encoded.clone();
    mutated[spans.sites[0].file.clone()].copy_from_slice(&2_u32.to_le_bytes());
    representable("a site's span file", &mutated);

    representable(
        "a dropped site row",
        &splice_site_row(&encoded, &spans, 1, &[]),
    );

    // Subject substitutions that break the roster order or name no module
    // subject reject at decoding.
    let mut mutated = encoded.clone();
    mutated[spans.sites[0].subject_tag.clone()].copy_from_slice(&[6]);
    rejected(
        "a subject kind breaking roster order",
        &mutated,
        DebugMapError::NonCanonicalOrder("debug sites by subject"),
    );

    let mut mutated = encoded.clone();
    mutated[spans.sites[1].subject_tag.clone()].copy_from_slice(&[1]);
    rejected(
        "a duplicated site subject",
        &mutated,
        DebugMapError::NonCanonicalOrder("debug sites by subject"),
    );

    let mut mutated = encoded.clone();
    mutated[spans.sites[0].subject_tag.clone()].copy_from_slice(&[0]);
    rejected(
        "an unknown subject tag",
        &mutated,
        DebugMapError::InvalidTag("DebugSubject", 0),
    );

    let mut mutated = encoded.clone();
    mutated[spans.sites[0].subject_id.clone()].copy_from_slice(&99_u64.to_le_bytes());
    rejected(
        "a subject naming no machine",
        &mutated,
        DebugMapError::UnknownSubject(DebugSubject::Machine(machine_id(99))),
    );

    // The module carries no structural place, so a Place subject cannot join.
    let mut mutated = encoded.clone();
    mutated[spans.sites[5].subject_tag.clone()].copy_from_slice(&[8]);
    rejected(
        "a subject naming no place",
        &mutated,
        DebugMapError::UnknownSubject(DebugSubject::Place(
            semantic_vocabulary::PlaceId::new(1).expect("nonzero place identity"),
        )),
    );

    // The two-identity Claim subject splices into the trailing site row and
    // still rejects: machine 1 carries no claim.
    let mut claim_site = encoded[spans.sites[5].row.clone()].to_vec();
    claim_site.splice(0..9, claim_subject_row(1, 1));
    rejected(
        "a subject naming no claim",
        &splice_site_row(&encoded, &spans, 5, &claim_site),
        DebugMapError::UnknownSubject(DebugSubject::Claim {
            machine: machine_id(1),
            claim: ClaimId::new(1).expect("nonzero claim identity"),
        }),
    );

    // Span coordinates reject the moment they lose the file join or invert.
    let mut mutated = encoded.clone();
    mutated[spans.sites[0].file.clone()].copy_from_slice(&0_u32.to_le_bytes());
    rejected(
        "a zero span file",
        &mutated,
        DebugMapError::ZeroFileIdentity,
    );
    let mut mutated = encoded.clone();
    mutated[spans.sites[0].file.clone()].copy_from_slice(&7_u32.to_le_bytes());
    rejected(
        "an unrostered span file",
        &mutated,
        DebugMapError::UnknownFile(file_id(7)),
    );
    let mut mutated = encoded.clone();
    mutated[spans.sites[0].start.clone()].copy_from_slice(&9_u64.to_le_bytes());
    rejected(
        "a span start overtaking its end",
        &mutated,
        DebugMapError::InvalidSpan(DebugSourceSpan {
            file: file_id(1),
            start: 9,
            end: 7,
        }),
    );
    let mut mutated = encoded.clone();
    mutated[spans.sites[0].end.clone()].copy_from_slice(&16_u64.to_le_bytes());
    rejected(
        "a span end escaping its file",
        &mutated,
        DebugMapError::InvalidSpan(DebugSourceSpan {
            file: file_id(1),
            start: 0,
            end: 16,
        }),
    );

    let mut mutated = encoded.clone();
    mutated[spans.site_count.clone()].copy_from_slice(&0_u32.to_le_bytes());
    rejected_unspecified("a cleared site count", &mutated);
    let mut mutated = encoded.clone();
    mutated[spans.site_count.clone()].copy_from_slice(&9_u32.to_le_bytes());
    rejected_unspecified("an over-counted site roster", &mutated);

    // --- the producing side binds the same rules at encoding ---

    let mut reordered = map.clone();
    reordered.sites.swap(0, 1);
    assert_eq!(
        encode_debug_map(&module, &reordered),
        Err(DebugMapError::NonCanonicalOrder("debug sites by subject")),
        "a reordered roster must reject at encoding"
    );
    let mut foreign_map = map.clone();
    foreign_map.semantic = terminal_psi_identity(&foreign).expect("foreign identity");
    assert!(matches!(
        encode_debug_map(&module, &foreign_map),
        Err(DebugMapError::SemanticIdentityMismatch { .. })
    ));
    let mut invalid_span = map.clone();
    invalid_span.sites[0].span.end = 16;
    assert_eq!(
        encode_debug_map(&module, &invalid_span),
        Err(DebugMapError::InvalidSpan(invalid_span.sites[0].span)),
        "an escaping span must reject at encoding"
    );

    // --- envelope replay: the same substitution rides the transport envelope
    // and diverges the decoded artifact identity ---

    let envelope = artifact.to_bytes();
    let semantic_len =
        usize::try_from(u64::from_le_bytes(envelope[10..18].try_into().unwrap())).unwrap();
    let proof_len =
        usize::try_from(u64::from_le_bytes(envelope[18..26].try_into().unwrap())).unwrap();
    let optimization_len =
        usize::try_from(u64::from_le_bytes(envelope[26..34].try_into().unwrap())).unwrap();
    assert_eq!(
        envelope[34], 1,
        "the fixture artifact carries a debug section"
    );
    let debug_len =
        usize::try_from(u64::from_le_bytes(envelope[35..43].try_into().unwrap())).unwrap();
    assert_eq!(debug_len, encoded.len());
    let debug_offset = 43 + semantic_len + proof_len + optimization_len;

    let mut swapped_envelope = envelope.clone();
    let mut swapped_section = encoded.clone();
    swapped_section[spans.files[0].origin.clone()].copy_from_slice(&[2]);
    swapped_envelope[debug_offset..debug_offset + debug_len].copy_from_slice(&swapped_section);
    let decoded = CanonicalTerminalArtifact::from_bytes(&swapped_envelope)
        .expect("a representable substitution still forms an artifact");
    assert_ne!(
        decoded.manifest().identity(),
        retained.identity(),
        "the substituted envelope must recompute a divergent artifact identity"
    );
    assert_eq!(
        validate_artifact_manifest(
            &module,
            &bundle,
            decoded.optimization(),
            None,
            decoded.debug_bytes(),
            retained,
        ),
        Err(ArtifactManifestError::ManifestMismatch),
        "the substituted envelope must reject at the retained-manifest replay"
    );

    let mut corrupt_envelope = envelope;
    corrupt_envelope[debug_offset + spans.files[0].origin.start] = 3;
    assert!(matches!(
        CanonicalTerminalArtifact::from_bytes(&corrupt_envelope),
        Err(CanonicalTerminalArtifactError::Debug(
            DebugMapError::InvalidTag("DebugSourceOrigin", 3)
        ))
    ));
}
