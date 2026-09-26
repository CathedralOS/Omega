//! One-field mutation coverage for the canonical Terminal debug-map section.
//!
//! The debug map is replaceable presentation evidence bound into the artifact
//! manifest through the `debug` section fingerprint. Its wire fields — the
//! sealed semantic identity, the counted source-file roster (file identity,
//! origin, byte length, digest, path), and the counted site roster (subject,
//! span file, span start, span end) — are each substituted independently,
//! every leg declared once in `debug_map_custody_fields.rs` and driven by the
//! shared one-field substitution matrix. A
//! substitution either fails canonical decoding (the decoder replays
//! `validate_debug_map` against the module and re-encodes the decoded map), or
//! decodes to a different map whose honestly recomputed manifest identity
//! diverges and whose replay against the retained manifest rejects.

use std::ops::Range;

#[path = "debug_map_custody_fields.rs"]
mod debug_map_custody_fields;

use super::{
    canonical_artifact, contract_id, edge_id, kernel_bundle, machine_id, obligation_id,
    operation_id, semantic_module, value_id,
};
use debug_map_custody_fields::DebugMapFieldForTest;
use optimization_core::{
    MutationOutcome, OneFieldSubstitutionMatrix, run_one_field_substitution_matrix,
};
use proof_admission::AdmissionProfile;
use semantic_vocabulary::ClaimId;
use terminal_codec::{
    ArtifactManifestError, CanonicalTerminalArtifact, CanonicalTerminalArtifactError, CodecError,
    DebugFileId, DebugMapError, DebugSite, DebugSourceFile, DebugSourceOrigin, DebugSourceSpan,
    DebugSubject, TerminalDebugMap, build_artifact_manifest, decode_debug_map, encode_debug_map,
    source_digest, terminal_psi_identity, validate_artifact_manifest, validate_debug_map,
};
use terminal_psi::{SemanticFingerprint, TerminalModule, TerminalPsiIdentity};
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

/// The first file row with every field held at its honest value except the
/// presentation path, whose u32 prefix is repaired to the new length.
fn first_file_row_with_path(path: &str) -> Vec<u8> {
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
}

/// An order-preserving fourth file row appended after the roster.
fn inserted_file_row(encoded: &[u8], spans: &MapSpans) -> Vec<u8> {
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
    inserted
}

fn overwrite(section: &mut [u8], range: Range<usize>, bytes: &[u8]) {
    section[range].copy_from_slice(bytes);
}

/// The debug-map family's honest-recomputation hook: rewrite exactly one wire
/// field of the canonical section, repairing only the count or length prefix
/// the substituted row owns. The donor section supplies an authentic foreign
/// source digest.
fn substitute_debug_map_for_test(
    section: &mut Vec<u8>,
    field: DebugMapFieldForTest,
    donor: &[u8],
    spans: &MapSpans,
) {
    use DebugMapFieldForTest as Field;
    let file = &spans.files[0];
    let site = &spans.sites[0];
    match field {
        Field::VocabularyMarker => {
            overwrite(section, spans.vocabulary.clone(), &u16::MAX.to_le_bytes())
        }
        Field::ProgramFingerprint => section[spans.fingerprint.start] ^= 0xFF,
        Field::Magic => section[spans.magic.start] ^= 0xFF,
        Field::FormatMarker => overwrite(
            section,
            spans.format_marker.clone(),
            &u16::MAX.to_le_bytes(),
        ),
        Field::TrailingByte => section.push(0),
        Field::FileOrigin => overwrite(section, file.origin.clone(), &[2]),
        Field::FileByteLength => overwrite(section, file.byte_len.clone(), &16_u64.to_le_bytes()),
        Field::FileDigest => overwrite(section, file.digest.clone(), &donor[file.digest.clone()]),
        Field::FilePath => {
            *section = splice_file_row(
                section,
                spans,
                0,
                &first_file_row_with_path("renamed/main.omg"),
            );
        }
        Field::UnreferencedFileDropped => *section = splice_file_row(section, spans, 2, &[]),
        Field::FileInserted => *section = inserted_file_row(section, spans),
        Field::ReferencedFileDropped => *section = splice_file_row(section, spans, 1, &[]),
        Field::FileIdentityZero => overwrite(section, file.id.clone(), &0_u32.to_le_bytes()),
        Field::FileIdentityDuplicated => {
            overwrite(section, spans.files[1].id.clone(), &1_u32.to_le_bytes());
        }
        Field::FileIdentityOutOfOrder => overwrite(section, file.id.clone(), &5_u32.to_le_bytes()),
        Field::FileOriginUnknownTag => overwrite(section, file.origin.clone(), &[3]),
        Field::FileByteLengthStrandingSpans => {
            overwrite(section, file.byte_len.clone(), &10_u64.to_le_bytes());
        }
        Field::FilePathNonUtf8 => overwrite(section, file.path.clone(), &[0xFF; 8]),
        Field::FilePathOverLong => {
            overwrite(
                section,
                file.path_len.clone(),
                &((1_u32 << 20) + 1).to_le_bytes(),
            );
        }
        Field::FileCountCleared => {
            overwrite(section, spans.file_count.clone(), &0_u32.to_le_bytes())
        }
        Field::FileCountOverCounted => {
            overwrite(section, spans.file_count.clone(), &7_u32.to_le_bytes())
        }
        // Machine(1) -> Block(1) keeps the roster order and names an existing
        // subject.
        Field::SiteSubjectKind => overwrite(section, site.subject_tag.clone(), &[2]),
        // Value(2) -> Value(1) keeps Edge(1) < Value(1) < Contract(1).
        Field::SiteSubjectIdentity => {
            overwrite(
                section,
                spans.sites[3].subject_id.clone(),
                &1_u64.to_le_bytes(),
            );
        }
        Field::SiteSpanStart => overwrite(section, site.start.clone(), &4_u64.to_le_bytes()),
        Field::SiteSpanEnd => overwrite(section, site.end.clone(), &10_u64.to_le_bytes()),
        // Rebinding the span to another rostered file keeps every join.
        Field::SiteSpanFile => overwrite(section, site.file.clone(), &2_u32.to_le_bytes()),
        Field::SiteDropped => *section = splice_site_row(section, spans, 1, &[]),
        Field::SiteSubjectKindOutOfOrder => overwrite(section, site.subject_tag.clone(), &[6]),
        Field::SiteSubjectDuplicated => {
            overwrite(section, spans.sites[1].subject_tag.clone(), &[1])
        }
        Field::SiteSubjectUnknownTag => overwrite(section, site.subject_tag.clone(), &[0]),
        Field::SiteSubjectNamesNoMachine => {
            overwrite(section, site.subject_id.clone(), &99_u64.to_le_bytes());
        }
        // The module carries no structural place, so a Place subject cannot
        // join.
        Field::SiteSubjectNamesNoPlace => {
            overwrite(section, spans.sites[5].subject_tag.clone(), &[8])
        }
        // The two-identity Claim subject splices into the trailing site row;
        // machine 1 carries no claim.
        Field::SiteSubjectNamesNoClaim => {
            let mut claim_site = section[spans.sites[5].row.clone()].to_vec();
            claim_site.splice(0..9, claim_subject_row(1, 1));
            *section = splice_site_row(section, spans, 5, &claim_site);
        }
        Field::SiteSpanFileZero => overwrite(section, site.file.clone(), &0_u32.to_le_bytes()),
        Field::SiteSpanFileUnrostered => {
            overwrite(section, site.file.clone(), &7_u32.to_le_bytes())
        }
        Field::SiteSpanStartOvertakingEnd => {
            overwrite(section, site.start.clone(), &9_u64.to_le_bytes())
        }
        Field::SiteSpanEndEscapingFile => {
            overwrite(section, site.end.clone(), &16_u64.to_le_bytes())
        }
        Field::SiteCountCleared => {
            overwrite(section, spans.site_count.clone(), &0_u32.to_le_bytes())
        }
        Field::SiteCountOverCounted => {
            overwrite(section, spans.site_count.clone(), &9_u32.to_le_bytes())
        }
    }
}

/// The family's combined independent checker result: canonical decoding
/// first, then replay of the substituted section against the retained
/// artifact manifest.
#[derive(Debug, PartialEq)]
enum DebugMapCheck {
    Decode(DebugMapError),
    ManifestReplay(ArtifactManifestError),
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

    // An authentic foreign section over the same module: only the first
    // file's source digest differs.
    let mut donor_map = map.clone();
    donor_map.files[0].digest = source_digest(b"machine donor {}");
    let donor = encode_debug_map(&module, &donor_map).expect("canonical donor section");

    let check = |section: &Vec<u8>| -> Result<Vec<u8>, DebugMapCheck> {
        decode_debug_map(&module, section).map_err(DebugMapCheck::Decode)?;
        validate_artifact_manifest(
            &module,
            &bundle,
            artifact.optimization(),
            None,
            Some(section),
            retained,
        )
        .map_err(DebugMapCheck::ManifestReplay)?;
        Ok(section.clone())
    };

    let expected_identity = terminal_psi_identity(&module).expect("module identity");
    let mut foreign_fingerprint = *expected_identity.program_fingerprint.as_bytes();
    foreign_fingerprint[0] ^= 0xFF;
    let outcome = |field: DebugMapFieldForTest| -> MutationOutcome<DebugMapCheck> {
        use DebugMapFieldForTest as Field;
        let decode = |error| MutationOutcome::ExactError(DebugMapCheck::Decode(error));
        match field {
            // --- file roster: origin, byte length, digest, and path are
            // independently representable, as are dropping an unreferenced
            // file and inserting an order-preserving one; site roster:
            // subject, span file, and span bounds are representable where
            // they keep the roster order and the file join ---
            Field::FileOrigin
            | Field::FileByteLength
            | Field::FileDigest
            | Field::FilePath
            | Field::UnreferencedFileDropped
            | Field::FileInserted
            | Field::SiteSubjectKind
            | Field::SiteSubjectIdentity
            | Field::SiteSpanStart
            | Field::SiteSpanEnd
            | Field::SiteSpanFile
            | Field::SiteDropped => MutationOutcome::ExactError(DebugMapCheck::ManifestReplay(
                ArtifactManifestError::ManifestMismatch,
            )),
            // --- sealed semantic identity: the vocabulary marker and program
            // fingerprint bind the map to one exact module ---
            Field::VocabularyMarker => decode(DebugMapError::UnsupportedVocabularyMarker(u16::MAX)),
            Field::ProgramFingerprint => decode(DebugMapError::SemanticIdentityMismatch {
                expected: expected_identity,
                actual: TerminalPsiIdentity {
                    vocabulary_marker: expected_identity.vocabulary_marker,
                    program_fingerprint: SemanticFingerprint::from_bytes(foreign_fingerprint),
                },
            }),
            // --- framing axes reject at decoding ---
            Field::Magic => decode(DebugMapError::InvalidMagic),
            Field::FormatMarker => decode(DebugMapError::UnsupportedFormatMarker(u16::MAX)),
            Field::TrailingByte => decode(DebugMapError::TrailingBytes(1)),
            // A referenced file cannot drop out: the site roster's span-file
            // join rejects at decoding.
            Field::ReferencedFileDropped => decode(DebugMapError::UnknownFile(file_id(2))),
            // File identities admit no representable substitution: a zero
            // identity rejects outright, and any other value breaks the
            // strictly increasing roster or strands the spans that join it.
            Field::FileIdentityZero => decode(DebugMapError::ZeroFileIdentity),
            Field::FileIdentityDuplicated | Field::FileIdentityOutOfOrder => decode(
                DebugMapError::NonCanonicalOrder("debug files by DebugFileId"),
            ),
            Field::FileOriginUnknownTag => {
                decode(DebugMapError::InvalidTag("DebugSourceOrigin", 3))
            }
            // A shrinking byte length strands the spans that index the file.
            Field::FileByteLengthStrandingSpans => {
                decode(DebugMapError::InvalidSpan(DebugSourceSpan {
                    file: file_id(1),
                    start: 8,
                    end: 12,
                }))
            }
            Field::FilePathNonUtf8 => decode(DebugMapError::Codec(CodecError::InvalidUtf8(
                "debug source path",
            ))),
            Field::FilePathOverLong => decode(DebugMapError::Codec(CodecError::StringTooLong(
                "debug source path",
            ))),
            // A count lying about its roster starves or strands the cursor:
            // a cleared count leaves its rows as trailing bytes, and an
            // over-count runs the cursor off the section's end.
            Field::FileCountCleared => decode(DebugMapError::TrailingBytes(334)),
            Field::FileCountOverCounted => decode(DebugMapError::Codec(CodecError::UnexpectedEnd)),
            // Subject substitutions that break the roster order or name no
            // module subject reject at decoding.
            Field::SiteSubjectKindOutOfOrder | Field::SiteSubjectDuplicated => {
                decode(DebugMapError::NonCanonicalOrder("debug sites by subject"))
            }
            Field::SiteSubjectUnknownTag => decode(DebugMapError::InvalidTag("DebugSubject", 0)),
            Field::SiteSubjectNamesNoMachine => decode(DebugMapError::UnknownSubject(
                DebugSubject::Machine(machine_id(99)),
            )),
            Field::SiteSubjectNamesNoPlace => {
                decode(DebugMapError::UnknownSubject(DebugSubject::Place(
                    semantic_vocabulary::PlaceId::new(1).expect("nonzero place identity"),
                )))
            }
            Field::SiteSubjectNamesNoClaim => {
                decode(DebugMapError::UnknownSubject(DebugSubject::Claim {
                    machine: machine_id(1),
                    claim: ClaimId::new(1).expect("nonzero claim identity"),
                }))
            }
            // Span coordinates reject the moment they lose the file join or
            // invert.
            Field::SiteSpanFileZero => decode(DebugMapError::ZeroFileIdentity),
            Field::SiteSpanFileUnrostered => decode(DebugMapError::UnknownFile(file_id(7))),
            Field::SiteSpanStartOvertakingEnd => {
                decode(DebugMapError::InvalidSpan(DebugSourceSpan {
                    file: file_id(1),
                    start: 9,
                    end: 7,
                }))
            }
            Field::SiteSpanEndEscapingFile => decode(DebugMapError::InvalidSpan(DebugSourceSpan {
                file: file_id(1),
                start: 0,
                end: 16,
            })),
            Field::SiteCountCleared => decode(DebugMapError::TrailingBytes(174)),
            Field::SiteCountOverCounted => decode(DebugMapError::Codec(CodecError::UnexpectedEnd)),
        }
    };

    // A substitution that still forms a canonical section decodes to a
    // different map that re-encodes canonically, satisfies the module-bound
    // invariants, and honestly recomputes a divergent artifact identity; the
    // checker has already rejected it against the retained manifest.
    let representable = |section: &Vec<u8>, field: DebugMapFieldForTest| {
        if !matches!(
            outcome(field),
            MutationOutcome::ExactError(DebugMapCheck::ManifestReplay(_))
        ) {
            return;
        }
        let substituted = decode_debug_map(&module, section)
            .unwrap_or_else(|error| panic!("{field:?} must still decode: {error:?}"));
        assert_ne!(substituted, map, "{field:?} must change the map");
        assert_eq!(
            encode_debug_map(&module, &substituted).as_ref(),
            Ok(section),
            "{field:?} must re-encode canonically"
        );
        assert_eq!(
            validate_debug_map(&module, &substituted),
            Ok(()),
            "{field:?} must still satisfy the module-bound map invariants"
        );
        let recomputed = build_artifact_manifest(
            &module,
            &bundle,
            artifact.optimization(),
            None,
            Some(section),
        )
        .expect("honest manifest for the substituted section");
        assert_ne!(
            recomputed.identity(),
            retained.identity(),
            "{field:?} must diverge the recomputed artifact identity"
        );
    };

    run_one_field_substitution_matrix(&OneFieldSubstitutionMatrix {
        family: "terminal debug map",
        fields: DebugMapFieldForTest::INVENTORY,
        honest: &|| encoded.clone(),
        donor,
        custody: &|section: &Vec<u8>| section.clone(),
        substitute: &|section, field, donor| {
            substitute_debug_map_for_test(section, field, donor, &spans);
        },
        check: &check,
        outcome: &outcome,
        joined_replay: Some(&representable),
    });

    // The retained section rejects under a foreign module: the sealed
    // semantic identity binds it to one exact program.
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

    // Truncation is not a one-field substitution: every cut through a field
    // rejects at decoding.
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
        assert!(
            decode_debug_map(&module, &encoded[..cut]).is_err(),
            "a truncated section must reject at canonical decoding"
        );
    }

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
