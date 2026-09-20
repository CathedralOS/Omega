//! One-field mutation coverage for the placed-image evidence section a native
//! `.proof` sidecar carries.
//!
//! The section's representable fields are the declared target tuple, the
//! declared executable-text, initialized-data and import-data file offsets,
//! and every field of the three sealed inventories: for
//! [`image::PlacedExecutableRegionInventory`] the text address, byte count,
//! digest and report fingerprint, the inventory seal digest and fingerprint,
//! each region row's origin, section offset, address, byte count, byte
//! digest, byte report fingerprint, symbol, and optional footprint with its
//! register and machine-state sets, and each gap row's five fields; for each
//! [`image::PlacedDataRegionInventory`] the same scalar and row shape minus
//! footprints. An import-data row that is not an `ImportBindingSlot`, or a
//! populated import-data inventory on a target that emits no separate
//! import-data extent, is unrepresentable.
//!
//! Each representable field substitutes independently — at the record level,
//! or at the wire level for the opaque scalar and digest fields — and the
//! mutated section still encodes in the single canonical order, decodes to
//! the substitution, and rejects under a named subject at independent
//! receiver-side replay: the semantic-profile join for a substituted
//! declared target, the placed-inventory replay against the committed bytes
//! for everything else. Substitutions the representation closes —
//! unrecognized envelope tags and versions, targets no realization path
//! produces, unknown row or register tags, a thunk row on a target that
//! realizes none or whose claimed extent or footprint is not the closed
//! thunk form's, a footprint register of the wrong architecture, unordered
//! or duplicated roster rows, footprint state bits that drop a
//! register-implied class, overstated counts, and truncated or extended
//! wire extents — reject at decoding, before any custody decision.

use std::ops::Range;

use calling_conventions::{
    MachineRegister, MachineState, MachineStateSet, RegisterSet, StateFootprintEvidence,
};
use image::{
    FinalDataRegion, FinalDataRegionOrigin, FinalExecutableRegion, FinalExecutableRegionOrigin,
    FinalImage, FinalImageLayout, FinalImageMemory, PlacedDataGap, PlacedDataRegion,
    PlacedExecutableGap, PlacedExecutableRegion,
};
use proof_admission::AdmissionProfile;
use terminal_codec::{
    PccGuarantee, PccIncompleteness, PccProductKind, PccProofSidecar, PccReceiverPolicy,
    PccVerificationOutcome, pcc_artifact_commitment,
};

use super::{
    MAX_FOOTPRINT_REGISTERS, MAX_INVENTORY_ROWS, NativeEvidenceError, NativePlacedImageEvidence,
};
use crate::pcc::{native_semantic_profile_identity, verify_native_proof_sidecar};

const TEXT_FILE_OFFSET: u64 = 96;
const TEXT_LEN: usize = 20;
const DATA_FILE_OFFSET: u64 = TEXT_FILE_OFFSET + TEXT_LEN as u64;
const DATA_LEN: usize = 12;

/// One three-region, two-gap inventory over non-uniform text, built through
/// the production placement path so every retained row carries honest
/// digests: `entry` at `[0..4)` with a footprint, `host_call` at `[8..12)`
/// without one, `tail` at `[16..20)` with a footprint whose machine state is
/// entirely register-implied, and unclassified gaps at `[4..8)` and
/// `[12..16)`.
fn placed_inventory(text: &[u8]) -> image::PlacedExecutableRegionInventory {
    let mut image = FinalImage::with_capacity(
        target::NativeTarget::linux_x64(),
        FinalImageMemory {
            text: text.to_vec(),
            ..FinalImageMemory::default()
        },
        Default::default(),
        0,
        0,
        0,
    );
    image.executable_regions.extend([
        FinalExecutableRegion {
            origin: FinalExecutableRegionOrigin::CompilerFunction,
            section_offset: 0,
            byte_count: 4,
            symbol: "entry".into(),
            footprint: Some(StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Rax, MachineRegister::X86Rdi]),
                MachineStateSet::new([MachineState::Flags]),
            )),
        },
        FinalExecutableRegion {
            origin: FinalExecutableRegionOrigin::CompilerFunction,
            section_offset: 8,
            byte_count: 4,
            symbol: "host_call".into(),
            footprint: None,
        },
        FinalExecutableRegion {
            origin: FinalExecutableRegionOrigin::CompilerFunction,
            section_offset: 16,
            byte_count: 4,
            symbol: "tail".into(),
            footprint: Some(StateFootprintEvidence::new(
                RegisterSet::new([MachineRegister::X86Xmm(0)]),
                MachineStateSet::empty(),
            )),
        },
    ]);
    image::place_executable_regions(
        &image,
        FinalImageLayout {
            text_address: 0x4010_0000,
            ..FinalImageLayout::default()
        },
    )
    .expect("the fixture regions place")
}

/// One two-row inventory over non-uniform initialized data, built through the
/// production placement path: `constants` at `[0..8)` plus one unclassified
/// gap at `[8..12)`.
fn placed_data_inventory(data: &[u8]) -> image::PlacedDataRegionInventory {
    let mut image = FinalImage::with_capacity(
        target::NativeTarget::linux_x64(),
        FinalImageMemory {
            data: data.to_vec(),
            ..FinalImageMemory::default()
        },
        Default::default(),
        0,
        0,
        0,
    );
    image.data_regions.push(FinalDataRegion {
        origin: FinalDataRegionOrigin::CompilerData,
        section_offset: 0,
        byte_count: 8,
        symbol: "constants".into(),
    });
    image::place_data_regions(
        &image,
        FinalImageLayout {
            data_address: 0x4020_0000,
            ..FinalImageLayout::default()
        },
    )
    .expect("the fixture data regions place")
}

/// One honest pair: the section, and the published container holding the
/// declared text extent at [`TEXT_FILE_OFFSET`] and the declared data extent
/// at [`DATA_FILE_OFFSET`]. The bytes are non-uniform so every declared span
/// commits to distinct content.
fn honest_pair() -> (NativePlacedImageEvidence, Vec<u8>) {
    let text: [u8; TEXT_LEN] = std::array::from_fn(|index| (index * 7 + 3) as u8);
    let data: [u8; DATA_LEN] = std::array::from_fn(|index| (index * 11 + 5) as u8);
    let mut executable = vec![0xffu8; TEXT_FILE_OFFSET as usize];
    executable.extend_from_slice(&text);
    executable.extend_from_slice(&data);
    executable.extend_from_slice(&[0x00u8; 32]);
    let evidence = NativePlacedImageEvidence::from_parts(
        target::NativeTarget::linux_x64(),
        TEXT_FILE_OFFSET,
        placed_inventory(&text),
        DATA_FILE_OFFSET,
        placed_data_inventory(&data),
        0,
        image::PlacedDataRegionInventory::empty(),
    );
    (evidence, executable)
}

/// A canonical sidecar carrying `evidence_bytes` under the honest declared
/// profile; the artifact commitment is honestly computed over `executable`.
fn native_sidecar(executable: &[u8], evidence_bytes: Vec<u8>) -> PccProofSidecar {
    native_sidecar_with_profile(
        executable,
        native_semantic_profile_identity(target::NativeTarget::linux_x64()),
        evidence_bytes,
    )
}

fn native_sidecar_with_profile(
    executable: &[u8],
    semantic_profile: String,
    evidence_bytes: Vec<u8>,
) -> PccProofSidecar {
    PccProofSidecar::new(
        PccProductKind::Native,
        pcc_artifact_commitment(executable),
        semantic_profile,
        "checker-profile".to_owned(),
        vec![PccGuarantee {
            identity: "guarantee".to_owned(),
            premises: Vec::new(),
        }],
        evidence_bytes,
        Vec::new(),
        Vec::new(),
    )
    .expect("a canonical sidecar")
}

fn rejecting_subject(outcome: PccVerificationOutcome) -> String {
    match outcome {
        PccVerificationOutcome::Reject(rejection) => rejection.subject,
        other => panic!("expected a named rejection, got {other:?}"),
    }
}

/// A read cursor that records the byte span of every field it walks.
struct Cursor<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> Cursor<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, len: usize) -> Range<usize> {
        let span = self.offset..self.offset + len;
        self.offset += len;
        span
    }

    fn u64(&mut self) -> (Range<usize>, u64) {
        let span = self.take(8);
        let value = u64::from_le_bytes(self.bytes[span.clone()].try_into().expect("u64 field"));
        (span, value)
    }
}

/// The wire span of one footprint body following a `1` presence tag.
struct FootprintSpan {
    register_count: Range<usize>,
    register_codes: Range<usize>,
    machine_state: Range<usize>,
}

/// The wire span of one executable region row.
struct RegionSpan {
    whole: Range<usize>,
    origin: Range<usize>,
    section_offset: Range<usize>,
    address: Range<usize>,
    byte_count: Range<usize>,
    byte_digest: Range<usize>,
    byte_report_fingerprint: Range<usize>,
    symbol_len: Range<usize>,
    symbol: Range<usize>,
    footprint_presence: Range<usize>,
    footprint: Option<FootprintSpan>,
}

/// The wire span of one executable gap row.
struct GapSpan {
    whole: Range<usize>,
    section_offset: Range<usize>,
    address: Range<usize>,
    byte_count: Range<usize>,
    byte_digest: Range<usize>,
    byte_report_fingerprint: Range<usize>,
}

/// The wire span of one data region row — a `RegionSpan` minus the
/// footprint tail.
struct DataRegionSpan {
    whole: Range<usize>,
    origin: Range<usize>,
    section_offset: Range<usize>,
    address: Range<usize>,
    byte_count: Range<usize>,
    byte_digest: Range<usize>,
    byte_report_fingerprint: Range<usize>,
    symbol_len: Range<usize>,
    symbol: Range<usize>,
}

/// Byte offsets of every wire field inside one canonical section encoding.
struct SectionSpans {
    magic: Range<usize>,
    version: Range<usize>,
    architecture: Range<usize>,
    object_format: Range<usize>,
    pointer_size: Range<usize>,
    pointer_alignment: Range<usize>,
    text_file_offset: Range<usize>,
    text_address: Range<usize>,
    text_byte_count: Range<usize>,
    text_digest: Range<usize>,
    text_report_fingerprint: Range<usize>,
    inventory_digest: Range<usize>,
    inventory_report_fingerprint: Range<usize>,
    region_count: Range<usize>,
    regions: Vec<RegionSpan>,
    gap_count: Range<usize>,
    gaps: Vec<GapSpan>,
    data_file_offset: Range<usize>,
    data_address: Range<usize>,
    data_byte_count: Range<usize>,
    data_digest: Range<usize>,
    data_report_fingerprint: Range<usize>,
    data_inventory_digest: Range<usize>,
    data_inventory_report_fingerprint: Range<usize>,
    data_region_count: Range<usize>,
    data_regions: Vec<DataRegionSpan>,
    data_gap_count: Range<usize>,
    data_gaps: Vec<GapSpan>,
    import_data_file_offset: Range<usize>,
    import_data_address: Range<usize>,
    import_data_byte_count: Range<usize>,
    import_data_digest: Range<usize>,
    import_data_report_fingerprint: Range<usize>,
    import_data_inventory_digest: Range<usize>,
    import_data_inventory_report_fingerprint: Range<usize>,
    import_data_region_count: Range<usize>,
    import_data_regions: Vec<DataRegionSpan>,
    import_data_gap_count: Range<usize>,
    import_data_gaps: Vec<GapSpan>,
    end: usize,
}

fn section_spans(encoded: &[u8]) -> SectionSpans {
    let mut cursor = Cursor::new(encoded);
    let magic = cursor.take(8);
    let version = cursor.take(2);
    let architecture = cursor.take(1);
    let object_format = cursor.take(1);
    let pointer_size = cursor.take(8);
    let pointer_alignment = cursor.take(8);
    let text_file_offset = cursor.take(8);
    let text_address = cursor.take(8);
    let text_byte_count = cursor.take(8);
    let text_digest = cursor.take(32);
    let text_report_fingerprint = cursor.take(8);
    let inventory_digest = cursor.take(32);
    let inventory_report_fingerprint = cursor.take(8);
    let (region_count, region_len) = cursor.u64();
    let mut regions = Vec::with_capacity(usize::try_from(region_len).expect("region count"));
    for _ in 0..region_len {
        let start = cursor.offset;
        let origin = cursor.take(1);
        let section_offset = cursor.take(8);
        let address = cursor.take(8);
        let byte_count = cursor.take(8);
        let byte_digest = cursor.take(32);
        let byte_report_fingerprint = cursor.take(8);
        let (symbol_len, symbol_bytes) = cursor.u64();
        let symbol = cursor.take(usize::try_from(symbol_bytes).expect("symbol length"));
        let footprint_presence = cursor.take(1);
        let footprint = if encoded[footprint_presence.start] == 1 {
            let (register_count, register_len) = cursor.u64();
            let register_codes =
                cursor.take(usize::try_from(register_len).expect("register count") * 2);
            let machine_state = cursor.take(2);
            Some(FootprintSpan {
                register_count,
                register_codes,
                machine_state,
            })
        } else {
            None
        };
        regions.push(RegionSpan {
            whole: start..cursor.offset,
            origin,
            section_offset,
            address,
            byte_count,
            byte_digest,
            byte_report_fingerprint,
            symbol_len,
            symbol,
            footprint_presence,
            footprint,
        });
    }
    let (gap_count, gap_len) = cursor.u64();
    let mut gaps = Vec::with_capacity(usize::try_from(gap_len).expect("gap count"));
    for _ in 0..gap_len {
        let start = cursor.offset;
        let section_offset = cursor.take(8);
        let address = cursor.take(8);
        let byte_count = cursor.take(8);
        let byte_digest = cursor.take(32);
        let byte_report_fingerprint = cursor.take(8);
        gaps.push(GapSpan {
            whole: start..cursor.offset,
            section_offset,
            address,
            byte_count,
            byte_digest,
            byte_report_fingerprint,
        });
    }
    let data_file_offset = cursor.take(8);
    let data_address = cursor.take(8);
    let data_byte_count = cursor.take(8);
    let data_digest = cursor.take(32);
    let data_report_fingerprint = cursor.take(8);
    let data_inventory_digest = cursor.take(32);
    let data_inventory_report_fingerprint = cursor.take(8);
    let (data_region_count, data_region_len) = cursor.u64();
    let mut data_regions =
        Vec::with_capacity(usize::try_from(data_region_len).expect("data region count"));
    for _ in 0..data_region_len {
        let start = cursor.offset;
        let origin = cursor.take(1);
        let section_offset = cursor.take(8);
        let address = cursor.take(8);
        let byte_count = cursor.take(8);
        let byte_digest = cursor.take(32);
        let byte_report_fingerprint = cursor.take(8);
        let (symbol_len, symbol_bytes) = cursor.u64();
        let symbol = cursor.take(usize::try_from(symbol_bytes).expect("symbol length"));
        data_regions.push(DataRegionSpan {
            whole: start..cursor.offset,
            origin,
            section_offset,
            address,
            byte_count,
            byte_digest,
            byte_report_fingerprint,
            symbol_len,
            symbol,
        });
    }
    let (data_gap_count, data_gap_len) = cursor.u64();
    let mut data_gaps = Vec::with_capacity(usize::try_from(data_gap_len).expect("data gap count"));
    for _ in 0..data_gap_len {
        let start = cursor.offset;
        let section_offset = cursor.take(8);
        let address = cursor.take(8);
        let byte_count = cursor.take(8);
        let byte_digest = cursor.take(32);
        let byte_report_fingerprint = cursor.take(8);
        data_gaps.push(GapSpan {
            whole: start..cursor.offset,
            section_offset,
            address,
            byte_count,
            byte_digest,
            byte_report_fingerprint,
        });
    }
    let import_data_file_offset = cursor.take(8);
    let import_data_address = cursor.take(8);
    let import_data_byte_count = cursor.take(8);
    let import_data_digest = cursor.take(32);
    let import_data_report_fingerprint = cursor.take(8);
    let import_data_inventory_digest = cursor.take(32);
    let import_data_inventory_report_fingerprint = cursor.take(8);
    let (import_data_region_count, import_data_region_len) = cursor.u64();
    let mut import_data_regions = Vec::with_capacity(
        usize::try_from(import_data_region_len).expect("import-data region count"),
    );
    for _ in 0..import_data_region_len {
        let start = cursor.offset;
        let origin = cursor.take(1);
        let section_offset = cursor.take(8);
        let address = cursor.take(8);
        let byte_count = cursor.take(8);
        let byte_digest = cursor.take(32);
        let byte_report_fingerprint = cursor.take(8);
        let (symbol_len, symbol_bytes) = cursor.u64();
        let symbol = cursor.take(usize::try_from(symbol_bytes).expect("symbol length"));
        import_data_regions.push(DataRegionSpan {
            whole: start..cursor.offset,
            origin,
            section_offset,
            address,
            byte_count,
            byte_digest,
            byte_report_fingerprint,
            symbol_len,
            symbol,
        });
    }
    let (import_data_gap_count, import_data_gap_len) = cursor.u64();
    let mut import_data_gaps =
        Vec::with_capacity(usize::try_from(import_data_gap_len).expect("import-data gap count"));
    for _ in 0..import_data_gap_len {
        let start = cursor.offset;
        let section_offset = cursor.take(8);
        let address = cursor.take(8);
        let byte_count = cursor.take(8);
        let byte_digest = cursor.take(32);
        let byte_report_fingerprint = cursor.take(8);
        import_data_gaps.push(GapSpan {
            whole: start..cursor.offset,
            section_offset,
            address,
            byte_count,
            byte_digest,
            byte_report_fingerprint,
        });
    }
    SectionSpans {
        magic,
        version,
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
        text_file_offset,
        text_address,
        text_byte_count,
        text_digest,
        text_report_fingerprint,
        inventory_digest,
        inventory_report_fingerprint,
        region_count,
        regions,
        gap_count,
        gaps,
        data_file_offset,
        data_address,
        data_byte_count,
        data_digest,
        data_report_fingerprint,
        data_inventory_digest,
        data_inventory_report_fingerprint,
        data_region_count,
        data_regions,
        data_gap_count,
        data_gaps,
        import_data_file_offset,
        import_data_address,
        import_data_byte_count,
        import_data_digest,
        import_data_report_fingerprint,
        import_data_inventory_digest,
        import_data_inventory_report_fingerprint,
        import_data_region_count,
        import_data_regions,
        import_data_gap_count,
        import_data_gaps,
        end: cursor.offset,
    }
}

/// Splice `replacement` over `span` of the canonical wire encoding.
fn splice(encoded: &[u8], span: &Range<usize>, replacement: &[u8]) -> Vec<u8> {
    let mut mutated =
        Vec::with_capacity(encoded.len() - (span.end - span.start) + replacement.len());
    mutated.extend_from_slice(&encoded[..span.start]);
    mutated.extend_from_slice(replacement);
    mutated.extend_from_slice(&encoded[span.end..]);
    mutated
}

/// Swap two adjacent wire spans, producing a non-canonical roster order.
fn swap(encoded: &[u8], first: &Range<usize>, second: &Range<usize>) -> Vec<u8> {
    assert_eq!(
        first.end, second.start,
        "swapped wire fields must be adjacent"
    );
    let mut mutated = Vec::with_capacity(encoded.len());
    mutated.extend_from_slice(&encoded[..first.start]);
    mutated.extend_from_slice(&encoded[second.clone()]);
    mutated.extend_from_slice(&encoded[first.clone()]);
    mutated.extend_from_slice(&encoded[second.end..]);
    mutated
}

#[test]
fn native_placed_image_evidence_rejects_every_one_field_substitution() {
    let (honest, executable) = honest_pair();
    let encoded = honest.to_bytes();
    let spans = section_spans(&encoded);
    assert_eq!(
        spans.end,
        encoded.len(),
        "the span walk covers the whole wire"
    );
    assert_eq!(spans.regions.len(), 3);
    assert_eq!(spans.gaps.len(), 2);
    assert_eq!(spans.data_regions.len(), 1);
    assert_eq!(spans.data_gaps.len(), 1);
    assert!(spans.import_data_regions.is_empty());
    assert!(spans.import_data_gaps.is_empty());
    assert_eq!(
        NativePlacedImageEvidence::from_bytes(&encoded),
        Ok(honest.clone()),
        "the honest section round-trips canonically"
    );

    // The receiver pins its policy once, from the honest offer — a mutated
    // offer never gets to choose what the receiver accepts. The honest pair
    // replays its coverage and thunk legs and still reports the behavioral
    // remainder `Incomplete`: custody is not certification.
    let honest_sidecar = native_sidecar(&executable, encoded.clone());
    let policy = PccReceiverPolicy::for_offered_claim(&honest_sidecar, AdmissionProfile::default());
    assert_eq!(
        verify_native_proof_sidecar(&executable, &honest_sidecar.to_bytes(), &policy),
        PccVerificationOutcome::Incomplete(PccIncompleteness::UnsupportedEvidence {
            product: PccProductKind::Native,
        })
    );

    // A record-level substitution still forms a canonical section — it
    // decodes to the mutated claim — and independent replay rejects it under
    // the named subject while the envelope keeps the honest claim.
    let rejects_at_replay =
        |name: &'static str, mutated: &NativePlacedImageEvidence, subject: &'static str| {
            let mutated_bytes = mutated.to_bytes();
            assert_ne!(mutated_bytes, encoded, "{name} must change the wire");
            assert_eq!(
                NativePlacedImageEvidence::from_bytes(&mutated_bytes),
                Ok(mutated.clone()),
                "{name} must remain a canonical section"
            );
            let sidecar = native_sidecar(&executable, mutated_bytes);
            assert_eq!(
                rejecting_subject(verify_native_proof_sidecar(
                    &executable,
                    &sidecar.to_bytes(),
                    &policy
                )),
                subject,
                "{name}"
            );
        };

    // A wire-level substitution inside a scalar or digest field decodes to a
    // different canonical section — the mutation is representable — and
    // rejects at the same replay join.
    let rejects_wire_at_replay =
        |name: &'static str, mutated_wire: Vec<u8>, subject: &'static str| {
            let decoded = NativePlacedImageEvidence::from_bytes(&mutated_wire)
                .unwrap_or_else(|error| panic!("{name} must decode canonically: {error:?}"));
            assert_ne!(decoded, honest, "{name} must decode to a different section");
            assert_eq!(
                decoded.to_bytes(),
                mutated_wire,
                "{name} must re-encode byte-identically"
            );
            let sidecar = native_sidecar(&executable, mutated_wire);
            assert_eq!(
                rejecting_subject(verify_native_proof_sidecar(
                    &executable,
                    &sidecar.to_bytes(),
                    &policy
                )),
                subject,
                "{name}"
            );
        };

    // A substitution the representation closes rejects at decoding, before
    // any custody decision.
    let malformed = |name: &'static str, mutated_wire: Vec<u8>| {
        assert!(
            matches!(
                NativePlacedImageEvidence::from_bytes(&mutated_wire),
                Err(NativeEvidenceError::Malformed(_))
            ),
            "{name} must reject as malformed evidence"
        );
    };

    // --- the declared target tuple ---

    // Object format keeps a declared partner under the fixed architecture,
    // so the substitution stays canonical and rejects at the semantic-profile
    // join: the evidence's declared target must realize the offered profile.
    // The same leg under an architecture substitution needs footprint-free
    // rows — the register vocabulary is closed per architecture, so rows
    // that carry one are malformed under a foreign arch rather than
    // canonical (asserted below with the decode-time legs).
    let mut mutated = honest.clone();
    mutated.target.architecture = target::Architecture::Aarch64;
    mutated.inventory.regions[0].footprint = None;
    mutated.inventory.regions[2].footprint = None;
    rejects_at_replay(
        "a substituted target architecture",
        &mutated,
        "semantic profile",
    );

    let mut mutated = honest.clone();
    mutated.target.architecture = target::Architecture::Aarch64;
    malformed(
        "a foreign architecture over x86 footprint registers",
        mutated.to_bytes(),
    );

    let mut mutated = honest.clone();
    mutated.target.object_format = target::ObjectFormat::Coff;
    rejects_at_replay(
        "a substituted target object format",
        &mutated,
        "semantic profile",
    );

    // Recomputing the offered profile over the substituted target does not
    // launder the substitution either: the pinned policy accepts only the
    // profile the receiver was fixed with.
    let consistent_lie = native_sidecar_with_profile(
        &executable,
        native_semantic_profile_identity(mutated.target),
        mutated.to_bytes(),
    );
    assert_eq!(
        rejecting_subject(verify_native_proof_sidecar(
            &executable,
            &consistent_lie.to_bytes(),
            &policy
        )),
        "semantic profile",
        "an honestly recomputed foreign profile must still reject"
    );

    // --- the declared text extent ---

    let mut mutated = honest.clone();
    mutated.text_file_offset -= 1;
    rejects_at_replay(
        "a shifted declared text extent",
        &mutated,
        "native executable inventory",
    );

    let mut mutated = honest.clone();
    mutated.text_file_offset = executable.len() as u64;
    rejects_at_replay(
        "a declared extent beyond the container",
        &mutated,
        "native executable inventory",
    );

    let mut mutated = honest.clone();
    mutated.text_file_offset = u64::MAX;
    rejects_at_replay(
        "an overflowing declared extent",
        &mutated,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.text_file_offset.start] ^= 0xff;
    rejects_wire_at_replay(
        "a wire-level text offset substitution",
        wire,
        "native executable inventory",
    );

    // --- the executable inventory scalars ---

    let mut wire = encoded.clone();
    wire[spans.text_address.start] ^= 0xff;
    rejects_wire_at_replay(
        "a substituted inventory text address",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.text_byte_count.start] ^= 1;
    rejects_wire_at_replay(
        "a substituted inventory text byte count",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.text_digest.start] ^= 0xff;
    rejects_wire_at_replay(
        "a substituted inventory text digest",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.text_report_fingerprint.start] ^= 1;
    rejects_wire_at_replay(
        "a substituted inventory text fingerprint",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.inventory_digest.start] ^= 0xff;
    rejects_wire_at_replay(
        "a substituted inventory seal digest",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.inventory_report_fingerprint.start] ^= 1;
    rejects_wire_at_replay(
        "a substituted inventory seal fingerprint",
        wire,
        "native executable inventory",
    );

    // --- each executable region-row field ---

    // Offset 4 keeps the rows offset-ordered, so the substitution stays
    // canonical; the replay then finds the row's stored address and digest
    // disagree with the bytes it now claims.
    rejects_wire_at_replay(
        "a substituted region section offset",
        splice(
            &encoded,
            &spans.regions[0].section_offset,
            &4u64.to_le_bytes(),
        ),
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.regions[0].address.start] ^= 0xff;
    rejects_wire_at_replay(
        "a substituted region address",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.regions[0].byte_count.start] ^= 1;
    rejects_wire_at_replay(
        "a substituted region byte count",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.regions[0].byte_digest.start] ^= 0xff;
    rejects_wire_at_replay(
        "a substituted region byte digest",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.regions[0].byte_report_fingerprint.start] ^= 1;
    rejects_wire_at_replay(
        "a substituted region byte fingerprint",
        wire,
        "native executable inventory",
    );

    // A same-length symbol substitution stays UTF-8 and canonical; the
    // inventory seal over the symbol no longer matches.
    let mut wire = encoded.clone();
    wire[spans.regions[0].symbol.start] = b'f';
    rejects_wire_at_replay(
        "a substituted region symbol",
        wire,
        "native executable inventory",
    );

    // --- the footprint sub-structure ---

    let mut mutated = honest.clone();
    mutated.inventory.regions[0].footprint = None;
    rejects_at_replay(
        "a dropped region footprint",
        &mutated,
        "native executable inventory",
    );

    let mut mutated = honest.clone();
    mutated.inventory.regions[1].footprint = Some(StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86Rcx]),
        MachineStateSet::new([MachineState::SegmentState]),
    ));
    rejects_at_replay(
        "an added region footprint",
        &mutated,
        "native executable inventory",
    );

    let mut mutated = honest.clone();
    mutated.inventory.regions[0].footprint = Some(StateFootprintEvidence::new(
        RegisterSet::new([
            MachineRegister::X86Rax,
            MachineRegister::X86Rdi,
            MachineRegister::X86Rbx,
        ]),
        MachineStateSet::new([MachineState::Flags]),
    ));
    rejects_at_replay(
        "a substituted footprint register set",
        &mutated,
        "native executable inventory",
    );

    let mut mutated = honest.clone();
    mutated.inventory.regions[0].footprint = Some(StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86Rax, MachineRegister::X86Rdi]),
        MachineStateSet::new([MachineState::Flags, MachineState::DebugState]),
    ));
    rejects_at_replay(
        "a substituted footprint machine-state set",
        &mutated,
        "native executable inventory",
    );

    // Dropping only the non-implied state class stays canonical — decode
    // re-derives the register-implied union — and still rejects: the sealed
    // footprint no longer matches the retained rows. (A wire form that drops
    // the implied class itself is non-canonical; that leg is below.)
    let mut mutated = honest.clone();
    mutated.inventory.regions[0].footprint = Some(StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::X86Rax, MachineRegister::X86Rdi]),
        MachineStateSet::empty(),
    ));
    rejects_at_replay(
        "a footprint reduced to register-implied machine state",
        &mutated,
        "native executable inventory",
    );

    let mut mutated = honest.clone();
    mutated.inventory.regions[0].footprint = Some(StateFootprintEvidence::new(
        RegisterSet::new([]),
        MachineStateSet::empty(),
    ));
    rejects_at_replay(
        "an emptied region footprint",
        &mutated,
        "native executable inventory",
    );

    // --- the executable region and gap rosters ---

    let mut mutated = honest.clone();
    mutated.inventory.regions.remove(0);
    rejects_at_replay(
        "a dropped region row",
        &mutated,
        "native executable inventory",
    );

    // An inserted row inside an unclassified gap keeps the offsets ordered,
    // so it still encodes canonically; the replay rejects because the byte
    // digest and gap partition it claims are not the ones the bytes produce.
    let mut mutated = honest.clone();
    mutated.inventory.regions.insert(
        1,
        PlacedExecutableRegion {
            origin: FinalExecutableRegionOrigin::CompilerFunction,
            section_offset: 4,
            address: mutated.inventory.text_address + 4,
            byte_count: 4,
            byte_digest: mutated.inventory.regions[1].byte_digest,
            byte_report_fingerprint: mutated.inventory.regions[1].byte_report_fingerprint,
            symbol: "forged".to_owned(),
            footprint: None,
        },
    );
    rejects_at_replay(
        "an inserted region row",
        &mutated,
        "native executable inventory",
    );

    let mut mutated = honest.clone();
    mutated.inventory.unclassified_gaps.remove(0);
    rejects_at_replay("a dropped gap row", &mutated, "native executable inventory");

    // An inserted gap inside an existing gap keeps the gap offsets ordered
    // and canonical; the recomputed partition still disagrees.
    let mut mutated = honest.clone();
    mutated.inventory.unclassified_gaps.insert(
        1,
        PlacedExecutableGap {
            section_offset: 6,
            address: mutated.inventory.text_address + 6,
            byte_count: 2,
            byte_digest: mutated.inventory.unclassified_gaps[0].byte_digest,
            byte_report_fingerprint: mutated.inventory.unclassified_gaps[0].byte_report_fingerprint,
        },
    );
    rejects_at_replay(
        "an inserted gap row",
        &mutated,
        "native executable inventory",
    );

    // --- each executable gap-row field ---

    rejects_wire_at_replay(
        "a substituted gap section offset",
        splice(&encoded, &spans.gaps[0].section_offset, &5u64.to_le_bytes()),
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.gaps[0].address.start] ^= 0xff;
    rejects_wire_at_replay(
        "a substituted gap address",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.gaps[0].byte_count.start] ^= 1;
    rejects_wire_at_replay(
        "a substituted gap byte count",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.gaps[0].byte_digest.start] ^= 0xff;
    rejects_wire_at_replay(
        "a substituted gap byte digest",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.gaps[0].byte_report_fingerprint.start] ^= 1;
    rejects_wire_at_replay(
        "a substituted gap byte fingerprint",
        wire,
        "native executable inventory",
    );

    // --- the declared data extent ---

    let mut mutated = honest.clone();
    mutated.data_file_offset -= 1;
    rejects_at_replay(
        "a shifted declared data extent",
        &mutated,
        "native executable inventory",
    );

    let mut mutated = honest.clone();
    mutated.data_file_offset = executable.len() as u64;
    rejects_at_replay(
        "a declared data extent beyond the container",
        &mutated,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.data_file_offset.start] ^= 0xff;
    rejects_wire_at_replay(
        "a wire-level data offset substitution",
        wire,
        "native executable inventory",
    );

    // --- the data inventory scalars ---

    let mut wire = encoded.clone();
    wire[spans.data_address.start] ^= 0xff;
    rejects_wire_at_replay(
        "a substituted inventory data address",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.data_byte_count.start] ^= 1;
    rejects_wire_at_replay(
        "a substituted inventory data byte count",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.data_digest.start] ^= 0xff;
    rejects_wire_at_replay(
        "a substituted inventory data digest",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.data_report_fingerprint.start] ^= 1;
    rejects_wire_at_replay(
        "a substituted inventory data fingerprint",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.data_inventory_digest.start] ^= 0xff;
    rejects_wire_at_replay(
        "a substituted data inventory seal digest",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.data_inventory_report_fingerprint.start] ^= 1;
    rejects_wire_at_replay(
        "a substituted data inventory seal fingerprint",
        wire,
        "native executable inventory",
    );

    // --- each data region-row field ---

    // A data origin substitution between two declared origins stays
    // canonical; the seal over the row's origin no longer matches.
    let mut wire = encoded.clone();
    wire[spans.data_regions[0].origin.start] = 3;
    rejects_wire_at_replay(
        "a substituted data region origin",
        wire,
        "native executable inventory",
    );

    rejects_wire_at_replay(
        "a substituted data region section offset",
        splice(
            &encoded,
            &spans.data_regions[0].section_offset,
            &4u64.to_le_bytes(),
        ),
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.data_regions[0].address.start] ^= 0xff;
    rejects_wire_at_replay(
        "a substituted data region address",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.data_regions[0].byte_count.start] ^= 1;
    rejects_wire_at_replay(
        "a substituted data region byte count",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.data_regions[0].byte_digest.start] ^= 0xff;
    rejects_wire_at_replay(
        "a substituted data region byte digest",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.data_regions[0].byte_report_fingerprint.start] ^= 1;
    rejects_wire_at_replay(
        "a substituted data region byte fingerprint",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.data_regions[0].symbol.start] = b'f';
    rejects_wire_at_replay(
        "a substituted data region symbol",
        wire,
        "native executable inventory",
    );

    // --- the data region and gap rosters ---

    let mut mutated = honest.clone();
    mutated.data_inventory.regions.remove(0);
    rejects_at_replay(
        "a dropped data region row",
        &mutated,
        "native executable inventory",
    );

    // An inserted row inside the unclassified gap keeps the offsets ordered,
    // so it still encodes canonically; the replay rejects because the byte
    // digest and gap partition it claims are not the ones the bytes produce.
    let mut mutated = honest.clone();
    mutated.data_inventory.regions.insert(
        1,
        PlacedDataRegion {
            origin: FinalDataRegionOrigin::AlignmentPadding,
            section_offset: 8,
            address: mutated.data_inventory.data_address + 8,
            byte_count: 4,
            byte_digest: mutated.data_inventory.regions[0].byte_digest,
            byte_report_fingerprint: mutated.data_inventory.regions[0].byte_report_fingerprint,
            symbol: String::new(),
        },
    );
    rejects_at_replay(
        "an inserted data region row",
        &mutated,
        "native executable inventory",
    );

    let mut mutated = honest.clone();
    mutated.data_inventory.unclassified_gaps.remove(0);
    rejects_at_replay(
        "a dropped data gap row",
        &mutated,
        "native executable inventory",
    );

    // An inserted gap inside the existing gap keeps the gap offsets ordered
    // and canonical; the recomputed partition still disagrees.
    let mut mutated = honest.clone();
    mutated.data_inventory.unclassified_gaps.insert(
        1,
        PlacedDataGap {
            section_offset: 9,
            address: mutated.data_inventory.data_address + 9,
            byte_count: 1,
            byte_digest: mutated.data_inventory.unclassified_gaps[0].byte_digest,
            byte_report_fingerprint: mutated.data_inventory.unclassified_gaps[0]
                .byte_report_fingerprint,
        },
    );
    rejects_at_replay(
        "an inserted data gap row",
        &mutated,
        "native executable inventory",
    );

    // --- each data gap-row field ---

    rejects_wire_at_replay(
        "a substituted data gap section offset",
        splice(
            &encoded,
            &spans.data_gaps[0].section_offset,
            &9u64.to_le_bytes(),
        ),
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.data_gaps[0].address.start] ^= 0xff;
    rejects_wire_at_replay(
        "a substituted data gap address",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.data_gaps[0].byte_count.start] ^= 1;
    rejects_wire_at_replay(
        "a substituted data gap byte count",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.data_gaps[0].byte_digest.start] ^= 0xff;
    rejects_wire_at_replay(
        "a substituted data gap byte digest",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.data_gaps[0].byte_report_fingerprint.start] ^= 1;
    rejects_wire_at_replay(
        "a substituted data gap byte fingerprint",
        wire,
        "native executable inventory",
    );

    // --- the import-data extent ---

    // The import-data inventory is the canonical empty one here, so its
    // representable substitutions split two ways: a field that makes the
    // inventory non-empty-shaped (its base address, byte count, or row
    // rosters) or that points its extent somewhere decodes as an
    // unrepresentable claim — a non-empty `.rdata` custody inventory exists
    // only under x86-64 Coff, and an empty one carries no extent — while a
    // digest or fingerprint substitution stays canonical and fails the
    // replayed seal over the empty extent instead.
    malformed(
        "a shifted declared import-data extent",
        splice(
            &encoded,
            &spans.import_data_file_offset,
            &4u64.to_le_bytes(),
        ),
    );

    let mut wire = encoded.clone();
    wire[spans.import_data_address.start] ^= 0xff;
    malformed("a substituted import-data base address", wire);

    let mut wire = encoded.clone();
    wire[spans.import_data_byte_count.start] ^= 1;
    malformed("a substituted import-data byte count", wire);

    let mut wire = encoded.clone();
    wire[spans.import_data_digest.start] ^= 0xff;
    rejects_wire_at_replay(
        "a substituted import-data digest",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.import_data_report_fingerprint.start] ^= 1;
    rejects_wire_at_replay(
        "a substituted import-data fingerprint",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.import_data_inventory_digest.start] ^= 0xff;
    rejects_wire_at_replay(
        "a substituted import-data inventory seal digest",
        wire,
        "native executable inventory",
    );

    let mut wire = encoded.clone();
    wire[spans.import_data_inventory_report_fingerprint.start] ^= 1;
    rejects_wire_at_replay(
        "a substituted import-data inventory seal fingerprint",
        wire,
        "native executable inventory",
    );

    malformed(
        "a padded import-data region roster",
        splice(
            &encoded,
            &spans.import_data_region_count,
            &2u64.to_le_bytes(),
        ),
    );
    malformed(
        "a padded import-data gap roster",
        splice(&encoded, &spans.import_data_gap_count, &2u64.to_le_bytes()),
    );

    // --- the containing commitment honestly recomputed ---

    // Recomputing the artifact commitment over substituted container bytes
    // does not launder the substitution: the section still replays against
    // the bytes actually published and rejects the mutated span.
    let mut foreign_executable = executable.clone();
    foreign_executable[TEXT_FILE_OFFSET as usize + 4] ^= 0xff;
    let foreign_sidecar = native_sidecar(&foreign_executable, encoded.clone());
    let foreign_policy =
        PccReceiverPolicy::for_offered_claim(&foreign_sidecar, AdmissionProfile::default());
    assert_eq!(
        rejecting_subject(verify_native_proof_sidecar(
            &foreign_executable,
            &foreign_sidecar.to_bytes(),
            &foreign_policy
        )),
        "native executable inventory",
        "a recomputed commitment over mutated bytes must still reject"
    );

    // The same laundering attempt against the data extent rejects identically.
    let mut foreign_executable = executable.clone();
    foreign_executable[DATA_FILE_OFFSET as usize + 4] ^= 0xff;
    let foreign_sidecar = native_sidecar(&foreign_executable, encoded.clone());
    let foreign_policy =
        PccReceiverPolicy::for_offered_claim(&foreign_sidecar, AdmissionProfile::default());
    assert_eq!(
        rejecting_subject(verify_native_proof_sidecar(
            &foreign_executable,
            &foreign_sidecar.to_bytes(),
            &foreign_policy
        )),
        "native executable inventory",
        "a recomputed commitment over mutated data bytes must still reject"
    );

    // Coverage is scoped to the declared extents: a substitution outside both
    // fails no row and reaches the honest behavioral-remainder verdict. The
    // artifact commitment, not the inventories, binds those bytes.
    let mut outside = executable.clone();
    outside[0] ^= 0xff;
    let outside_sidecar = native_sidecar(&outside, encoded.clone());
    let outside_policy =
        PccReceiverPolicy::for_offered_claim(&outside_sidecar, AdmissionProfile::default());
    assert_eq!(
        verify_native_proof_sidecar(&outside, &outside_sidecar.to_bytes(), &outside_policy),
        PccVerificationOutcome::Incomplete(PccIncompleteness::UnsupportedEvidence {
            product: PccProductKind::Native,
        }),
        "bytes outside the declared extents are bound by the commitment, not the inventories"
    );

    // --- the non-canonical legs: closed tags, closed targets, ordered and
    // deduplicated vocabularies, bounded counts, and the exact wire extent
    // all reject at decoding before any custody decision ---

    // The envelope's own identity: an unrecognized magic or version is
    // unsupported, never malformed.
    let mut wire = encoded.clone();
    wire[spans.magic.start] ^= 0xff;
    assert_eq!(
        NativePlacedImageEvidence::from_bytes(&wire),
        Err(NativeEvidenceError::Unsupported),
        "an unrecognized magic is unsupported"
    );
    let mut wire = encoded.clone();
    wire[spans.version.start] = 9;
    assert_eq!(
        NativePlacedImageEvidence::from_bytes(&wire),
        Err(NativeEvidenceError::Unsupported),
        "an unrecognized version is unsupported"
    );

    // Closed vocabulary tags.
    let mut wire = encoded.clone();
    wire[spans.architecture.start] = 9;
    malformed("an unknown architecture tag", wire);
    let mut wire = encoded.clone();
    wire[spans.object_format.start] = 9;
    malformed("an unknown object format tag", wire);
    let mut wire = encoded.clone();
    wire[spans.regions[0].origin.start] = 9;
    malformed("an unknown region origin tag", wire);
    let mut wire = encoded.clone();
    wire[spans.data_regions[0].origin.start] = 9;
    malformed("an unknown data region origin tag", wire);
    let mut wire = encoded.clone();
    wire[spans.regions[0].footprint_presence.start] = 2;
    malformed("an unknown footprint presence tag", wire);

    // The declared target set is closed: undeclared pointer axes and
    // undeclared (architecture, format) pairs are malformed, not merely
    // unusual claims.
    malformed(
        "an undeclared pointer size",
        splice(&encoded, &spans.pointer_size, &4u64.to_le_bytes()),
    );
    malformed(
        "an undeclared pointer alignment",
        splice(&encoded, &spans.pointer_alignment, &4u64.to_le_bytes()),
    );
    let mut wire = encoded.clone();
    wire[spans.object_format.start] = 2;
    malformed("an undeclared x86_64-macho target pair", wire);
    let mut wire = encoded.clone();
    wire[spans.architecture.start] = 1;
    wire[spans.object_format.start] = 3;
    malformed("an undeclared aarch64-coff target pair", wire);

    // The footprint register vocabulary is closed, canonically ordered, and
    // declared per target: a register the declared architecture does not own
    // is malformed, not a foreign claim.
    let footprint = spans.regions[0]
        .footprint
        .as_ref()
        .expect("the fixture carries a footprint");
    let mut wire = encoded.clone();
    wire[footprint.register_codes.start..footprint.register_codes.start + 2]
        .copy_from_slice(&0x0400u16.to_le_bytes());
    malformed("an unknown footprint register code", wire);
    let mut wire = encoded.clone();
    wire[footprint.register_codes.start + 2..footprint.register_codes.start + 4]
        .copy_from_slice(&0x0200u16.to_le_bytes());
    malformed("a footprint register of another architecture", wire);
    malformed(
        "an unbounded footprint register count",
        splice(
            &encoded,
            &footprint.register_count,
            &(MAX_FOOTPRINT_REGISTERS + 1).to_le_bytes(),
        ),
    );
    // A duplicated register dedups on decode, so the re-encode is shorter and
    // the offered wire form was never canonical.
    let mut wire = encoded.clone();
    wire[footprint.register_codes.clone()]
        .copy_from_slice(&[0u16.to_le_bytes(), 0u16.to_le_bytes()].concat());
    malformed("a duplicated footprint register", wire);
    // Reversed register order re-sorts on decode; the re-encode differs.
    let mut wire = encoded.clone();
    wire[footprint.register_codes.clone()]
        .copy_from_slice(&[7u16.to_le_bytes(), 0u16.to_le_bytes()].concat());
    malformed("unordered footprint registers", wire);
    // Machine-state bits outside the closed vocabulary reject.
    let mut wire = encoded.clone();
    wire[footprint.machine_state.clone()].copy_from_slice(&0x0200u16.to_le_bytes());
    malformed("machine-state bits outside the vocabulary", wire);
    // A wire state set missing the class its registers imply re-derives the
    // union on decode, so the offered wire form was never canonical.
    let mut wire = encoded.clone();
    wire[footprint.machine_state.clone()].copy_from_slice(&0x0004u16.to_le_bytes());
    malformed("a machine-state set missing an implied class", wire);

    // An import-thunk claim on a target that realizes none is malformed, and
    // a thunk row that misses the closed form's extent or footprint is
    // malformed on the realizing target — the thunk legs are exercised
    // directly below.
    let mut forged = honest.clone();
    forged.inventory.regions[1].origin = FinalExecutableRegionOrigin::ImportThunk;
    malformed("an import thunk row ELF cannot realize", forged.to_bytes());

    // Bounded roster counts.
    malformed(
        "an unbounded region count",
        splice(
            &encoded,
            &spans.region_count,
            &(MAX_INVENTORY_ROWS + 1).to_le_bytes(),
        ),
    );
    malformed(
        "an unbounded gap count",
        splice(
            &encoded,
            &spans.gap_count,
            &(MAX_INVENTORY_ROWS + 1).to_le_bytes(),
        ),
    );
    malformed(
        "an unbounded data region count",
        splice(
            &encoded,
            &spans.data_region_count,
            &(MAX_INVENTORY_ROWS + 1).to_le_bytes(),
        ),
    );
    malformed(
        "an unbounded data gap count",
        splice(
            &encoded,
            &spans.data_gap_count,
            &(MAX_INVENTORY_ROWS + 1).to_le_bytes(),
        ),
    );
    // A count that lies about the rows that follow desynchronizes the frame.
    malformed(
        "a shortened region count",
        splice(&encoded, &spans.region_count, &2u64.to_le_bytes()),
    );
    malformed(
        "an extended region count",
        splice(&encoded, &spans.region_count, &4u64.to_le_bytes()),
    );
    malformed(
        "a shortened gap count",
        splice(&encoded, &spans.gap_count, &1u64.to_le_bytes()),
    );
    malformed(
        "an extended gap count",
        splice(&encoded, &spans.gap_count, &3u64.to_le_bytes()),
    );
    malformed(
        "a shortened data region count",
        splice(&encoded, &spans.data_region_count, &0u64.to_le_bytes()),
    );
    malformed(
        "an extended data region count",
        splice(&encoded, &spans.data_region_count, &2u64.to_le_bytes()),
    );
    malformed(
        "a shortened data gap count",
        splice(&encoded, &spans.data_gap_count, &0u64.to_le_bytes()),
    );
    malformed(
        "an extended data gap count",
        splice(&encoded, &spans.data_gap_count, &2u64.to_le_bytes()),
    );

    // Row order and duplication are closed by the representation.
    malformed(
        "reordered region rows",
        swap(&encoded, &spans.regions[0].whole, &spans.regions[1].whole),
    );
    let mut wire = splice(&encoded, &spans.region_count, &4u64.to_le_bytes());
    wire.splice(
        spans.regions[1].whole.end..spans.regions[1].whole.end,
        encoded[spans.regions[1].whole.clone()].iter().copied(),
    );
    malformed("a duplicated region row", wire);
    malformed(
        "reordered gap rows",
        swap(&encoded, &spans.gaps[0].whole, &spans.gaps[1].whole),
    );
    let mut wire = splice(&encoded, &spans.gap_count, &3u64.to_le_bytes());
    wire.splice(
        spans.gaps[1].whole.end..spans.gaps[1].whole.end,
        encoded[spans.gaps[1].whole.clone()].iter().copied(),
    );
    malformed("a duplicated gap row", wire);
    let mut wire = splice(&encoded, &spans.data_region_count, &2u64.to_le_bytes());
    wire.splice(
        spans.data_regions[0].whole.end..spans.data_regions[0].whole.end,
        encoded[spans.data_regions[0].whole.clone()].iter().copied(),
    );
    malformed("a duplicated data region row", wire);

    // Region symbols are bounded UTF-8.
    let mut wire = encoded.clone();
    wire[spans.regions[0].symbol.start] = 0xff;
    malformed("a non-UTF-8 region symbol", wire);
    malformed(
        "an overstated symbol length",
        splice(
            &encoded,
            &spans.regions[0].symbol_len,
            &u64::MAX.to_le_bytes(),
        ),
    );
    let mut wire = encoded.clone();
    wire[spans.data_regions[0].symbol.start] = 0xff;
    malformed("a non-UTF-8 data region symbol", wire);
    malformed(
        "an overstated data symbol length",
        splice(
            &encoded,
            &spans.data_regions[0].symbol_len,
            &u64::MAX.to_le_bytes(),
        ),
    );

    // The wire extent is exact: a cut at any field boundary and any trailing
    // byte both reject.
    for cut in [
        spans.pointer_size.end - 1,
        spans.text_file_offset.end - 1,
        spans.text_address.end - 1,
        spans.text_byte_count.end - 1,
        spans.text_digest.end - 1,
        spans.text_report_fingerprint.end - 1,
        spans.inventory_digest.end - 1,
        spans.inventory_report_fingerprint.end - 1,
        spans.region_count.end - 1,
        spans.regions[0].address.end - 1,
        spans.regions[0].symbol.start,
        spans.regions[0]
            .footprint
            .as_ref()
            .expect("the fixture carries a footprint")
            .machine_state
            .end
            - 1,
        spans.regions[1].whole.end - 1,
        spans.gap_count.end - 1,
        spans.gaps[0].address.end - 1,
        spans.data_file_offset.end - 1,
        spans.data_address.end - 1,
        spans.data_byte_count.end - 1,
        spans.data_digest.end - 1,
        spans.data_report_fingerprint.end - 1,
        spans.data_inventory_digest.end - 1,
        spans.data_inventory_report_fingerprint.end - 1,
        spans.data_region_count.end - 1,
        spans.data_regions[0].symbol.start,
        spans.data_gap_count.end - 1,
        spans.data_gaps[0].address.end - 1,
        encoded.len() - 1,
    ] {
        assert!(
            NativePlacedImageEvidence::from_bytes(&encoded[..cut]).is_err(),
            "truncation at byte {cut} must reject"
        );
    }
    let mut wire = encoded.clone();
    wire.push(0);
    malformed("a trailing byte", wire);
}

/// The absolute address the Coff fixture's thunk displacement decodes to —
/// the thunk sits at address 12 (the default text base is zero), so its six
/// bytes end at 18 and `COFF_SLOT_ADDRESS - 18` is the displacement. The slot
/// stays within a signed `disp32` of the thunk, as a real `.rdata` base does.
const COFF_SLOT_ADDRESS: u64 = 0x2200;

/// A (x86_64, Coff) fixture carrying one real import thunk and the
/// `.rdata` custody it binds: twelve compiler bytes and the emitted
/// `jmp [rip+disp32]` over them, with the closed-form footprint that
/// realization attaches, plus the import-data extent holding the thunk's
/// `ImportBindingSlot` at the absolute address the displacement decodes to —
/// writer-owned data, so the placed inventory is built over the extent
/// itself rather than over `image.memory.data` (which stays empty here).
fn windows_thunk_pair() -> (NativePlacedImageEvidence, Vec<u8>) {
    // One `ImportBindingSlot` row plus the unclassified gap that stands in
    // for the descriptors, lookup tables and name bytes a real `.rdata`
    // table carries around the slots.
    let import_data: [u8; 16] = std::array::from_fn(|index| (index * 13 + 7) as u8);
    windows_thunk_pair_over(
        &import_data,
        vec![FinalDataRegion {
            origin: FinalDataRegionOrigin::ImportBindingSlot,
            section_offset: 8,
            byte_count: 8,
            symbol: "host_call".into(),
        }],
        COFF_SLOT_ADDRESS - 8,
    )
}

/// Build the Coff fixture over an arbitrary import-data extent. The thunk
/// still decodes to [`COFF_SLOT_ADDRESS`], so the caller chooses the placed
/// slot rows and base address that make the thunk↔slot pairing honest or
/// not; every seal stays honest either way.
fn windows_thunk_pair_over(
    import_data: &[u8],
    import_slots: Vec<FinalDataRegion>,
    import_data_address: u64,
) -> (NativePlacedImageEvidence, Vec<u8>) {
    windows_thunk_pair_in(
        vec![0xffu8; TEXT_FILE_OFFSET as usize],
        import_data,
        import_slots,
        import_data_address,
    )
}

/// The Coff thunk fixture inside the caller's container `header`: the
/// declared text extent lands right after the header bytes and the
/// import-data extent follows it, so a container carrying a loadable map
/// decides whether the declared extents find coverage.
fn windows_thunk_pair_in(
    mut executable: Vec<u8>,
    import_data: &[u8],
    import_slots: Vec<FinalDataRegion>,
    import_data_address: u64,
) -> (NativePlacedImageEvidence, Vec<u8>) {
    let text_file_offset = executable.len() as u64;
    let target = target::NativeTarget::windows_x64();
    let (extent, footprint) = super::import_thunk_form(target).expect("Coff realizes a thunk");
    let mut text = vec![0xabu8; 12];
    let displacement = (COFF_SLOT_ADDRESS - 18) as i32;
    text.extend_from_slice(&[0xff, 0x25]);
    text.extend_from_slice(&displacement.to_le_bytes());
    assert_eq!(text.len() - 12, extent);
    let mut image = FinalImage::with_capacity(
        target,
        FinalImageMemory {
            text: text.clone(),
            ..FinalImageMemory::default()
        },
        Default::default(),
        0,
        0,
        0,
    );
    image.executable_regions.extend([
        FinalExecutableRegion {
            origin: FinalExecutableRegionOrigin::CompilerFunction,
            section_offset: 0,
            byte_count: 12,
            symbol: "entry".into(),
            footprint: None,
        },
        FinalExecutableRegion {
            origin: FinalExecutableRegionOrigin::ImportThunk,
            section_offset: 12,
            byte_count: extent,
            symbol: "host_call".into(),
            footprint: Some(footprint),
        },
    ]);
    let layout = FinalImageLayout::default();
    let executable_inventory =
        image::place_executable_regions(&image, layout).expect("the thunk fixture places");
    let data_inventory =
        image::place_data_regions(&image, layout).expect("the empty data inventory places");
    let import_data_inventory = if import_data.is_empty() {
        image::PlacedDataRegionInventory::empty()
    } else {
        image::place_data_extent(
            import_data,
            import_slots,
            import_data_address,
            "import data",
        )
        .expect("the slot fixture places")
    };
    let import_data_file_offset = if import_data.is_empty() {
        0
    } else {
        text_file_offset + text.len() as u64
    };
    executable.extend_from_slice(&text);
    executable.extend_from_slice(import_data);
    executable.extend_from_slice(&[0x00u8; 32]);
    let evidence = NativePlacedImageEvidence::from_parts(
        target,
        text_file_offset,
        executable_inventory,
        0,
        data_inventory,
        import_data_file_offset,
        import_data_inventory,
    );
    (evidence, executable)
}

/// An (aarch64, MachO) fixture with one real import thunk and its binding
/// slot: `ADRP X16,+2 pages; LDR X16,[X16,#0]; BR X16` at text offset 12
/// binds the slot at `data_address`, which the placed data inventory commits.
fn macos_thunk_pair() -> (NativePlacedImageEvidence, Vec<u8>) {
    let target = target::NativeTarget::macos_arm64();
    let (extent, footprint) = super::import_thunk_form(target).expect("MachO realizes a thunk");
    let layout = FinalImageLayout {
        text_address: 0x1_0000_0000,
        data_address: 0x1_0000_2000,
        ..FinalImageLayout::default()
    };
    let mut text = vec![0xabu8; 12];
    text.extend_from_slice(&[
        0x10, 0x00, 0x00, 0xd0, // ADRP X16, +0x2000
        0x10, 0x02, 0x40, 0xf9, // LDR X16, [X16, #0]
        0x00, 0x02, 0x1f, 0xd6, // BR X16
    ]);
    assert_eq!(text.len() - 12, extent);
    let data = [0u8; 8];
    let mut image = FinalImage::with_capacity(
        target,
        FinalImageMemory {
            text: text.clone(),
            data: data.to_vec(),
            ..FinalImageMemory::default()
        },
        Default::default(),
        0,
        0,
        0,
    );
    image.executable_regions.extend([
        FinalExecutableRegion {
            origin: FinalExecutableRegionOrigin::CompilerFunction,
            section_offset: 0,
            byte_count: 12,
            symbol: "entry".into(),
            footprint: None,
        },
        FinalExecutableRegion {
            origin: FinalExecutableRegionOrigin::ImportThunk,
            section_offset: 12,
            byte_count: extent,
            symbol: "imported".into(),
            footprint: Some(footprint),
        },
    ]);
    image.data_regions.push(FinalDataRegion {
        origin: FinalDataRegionOrigin::ImportBindingSlot,
        section_offset: 0,
        byte_count: 8,
        symbol: "imported".into(),
    });
    let executable_inventory =
        image::place_executable_regions(&image, layout).expect("the thunk fixture places");
    let data_inventory =
        image::place_data_regions(&image, layout).expect("the slot fixture places");
    let mut executable = vec![0xffu8; TEXT_FILE_OFFSET as usize];
    executable.extend_from_slice(&text);
    executable.extend_from_slice(&data);
    executable.extend_from_slice(&[0x00u8; 32]);
    let evidence = NativePlacedImageEvidence::from_parts(
        target,
        TEXT_FILE_OFFSET,
        executable_inventory,
        TEXT_FILE_OFFSET + text.len() as u64,
        data_inventory,
        0,
        image::PlacedDataRegionInventory::empty(),
    );
    (evidence, executable)
}

fn offered_policy(sidecar: &PccProofSidecar) -> PccReceiverPolicy {
    PccReceiverPolicy::for_offered_claim(sidecar, AdmissionProfile::default())
}

/// The thunk leg: a claimed `ImportThunk` is bound to the declared target's
/// closed thunk sequence — its claimed extent and footprint must equal the
/// form's at decode, and its realized bytes must decode to that form at
/// replay. On aarch64 Mach-O the replay additionally binds the decoded
/// pointer load to exactly one committed `ImportBindingSlot`.
#[test]
fn import_thunk_claims_bind_to_the_declared_targets_closed_form() {
    // Both honest pairs replay every carried leg and still report the
    // behavioral remainder: thunk realization is custody, not certification.
    let (coff_evidence, coff_executable) = windows_thunk_pair();
    let coff_sidecar = native_sidecar_with_profile(
        &coff_executable,
        native_semantic_profile_identity(target::NativeTarget::windows_x64()),
        coff_evidence.to_bytes(),
    );
    assert_eq!(
        verify_native_proof_sidecar(
            &coff_executable,
            &coff_sidecar.to_bytes(),
            &offered_policy(&coff_sidecar)
        ),
        PccVerificationOutcome::Incomplete(PccIncompleteness::UnsupportedEvidence {
            product: PccProductKind::Native,
        }),
        "an honest Coff thunk pair replays its thunk leg"
    );

    let (macho_evidence, macho_executable) = macos_thunk_pair();
    let macho_sidecar = native_sidecar_with_profile(
        &macho_executable,
        native_semantic_profile_identity(target::NativeTarget::macos_arm64()),
        macho_evidence.to_bytes(),
    );
    assert_eq!(
        verify_native_proof_sidecar(
            &macho_executable,
            &macho_sidecar.to_bytes(),
            &offered_policy(&macho_sidecar)
        ),
        PccVerificationOutcome::Incomplete(PccIncompleteness::UnsupportedEvidence {
            product: PccProductKind::Native,
        }),
        "an honest MachO thunk pair replays its thunk↔slot pairing"
    );

    let malformed = |name: &'static str, wire: Vec<u8>| {
        assert!(
            matches!(
                NativePlacedImageEvidence::from_bytes(&wire),
                Err(NativeEvidenceError::Malformed(_))
            ),
            "{name} must reject as malformed evidence"
        );
    };

    // A thunk row whose claimed extent or footprint is not the closed
    // form's is not a representable claim.
    let mut mutated = coff_evidence.clone();
    mutated.inventory.regions[1].byte_count = 8;
    malformed("a Coff thunk claiming a foreign extent", mutated.to_bytes());
    let mut mutated = coff_evidence.clone();
    mutated.inventory.regions[1].footprint = None;
    malformed("a Coff thunk claiming no footprint", mutated.to_bytes());
    let mut mutated = macho_evidence.clone();
    mutated.inventory.regions[1].footprint = Some(StateFootprintEvidence::new(
        RegisterSet::new([MachineRegister::Aarch64X(17)]),
        MachineStateSet::new([MachineState::InstructionPointer]),
    ));
    malformed(
        "a MachO thunk claiming a foreign footprint",
        mutated.to_bytes(),
    );

    // An honestly-sealed thunk row over bytes that do not carry the closed
    // sequence decodes — the lie is in the bytes — and rejects at replay.
    let mut wrong_bytes_text = vec![0xabu8; 12];
    wrong_bytes_text.extend_from_slice(&[0x48, 0x89, 0x05, 0x78, 0x56, 0x34]);
    let wrong_coff = {
        let target = target::NativeTarget::windows_x64();
        let (extent, footprint) = super::import_thunk_form(target).expect("Coff realizes a thunk");
        let mut image = FinalImage::with_capacity(
            target,
            FinalImageMemory {
                text: wrong_bytes_text.clone(),
                ..FinalImageMemory::default()
            },
            Default::default(),
            0,
            0,
            0,
        );
        image.executable_regions.extend([
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::CompilerFunction,
                section_offset: 0,
                byte_count: 12,
                symbol: "entry".into(),
                footprint: None,
            },
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::ImportThunk,
                section_offset: 12,
                byte_count: extent,
                symbol: "host_call".into(),
                footprint: Some(footprint),
            },
        ]);
        let layout = FinalImageLayout::default();
        let mut executable = vec![0xffu8; TEXT_FILE_OFFSET as usize];
        executable.extend_from_slice(&wrong_bytes_text);
        executable.extend_from_slice(&[0x00u8; 32]);
        (
            NativePlacedImageEvidence::from_parts(
                target,
                TEXT_FILE_OFFSET,
                image::place_executable_regions(&image, layout).expect("the fixture places"),
                0,
                image::place_data_regions(&image, layout).expect("the fixture places"),
                0,
                image::PlacedDataRegionInventory::empty(),
            ),
            executable,
        )
    };
    let wrong_sidecar = native_sidecar_with_profile(
        &wrong_coff.1,
        native_semantic_profile_identity(target::NativeTarget::windows_x64()),
        wrong_coff.0.to_bytes(),
    );
    assert_eq!(
        rejecting_subject(verify_native_proof_sidecar(
            &wrong_coff.1,
            &wrong_sidecar.to_bytes(),
            &offered_policy(&wrong_sidecar)
        )),
        "native executable inventory",
        "a Coff thunk over non-jmp bytes must reject at replay"
    );

    // A MachO thunk over bytes outside the closed sequence rejects inside
    // the pairing decode even when every inventory digest is honest. The
    // inventory is honestly placed over bytes that are not a thunk, so the
    // custody seals verify and the sequence check is the failing leg.
    let wrong_macho = {
        let target = target::NativeTarget::macos_arm64();
        let (extent, footprint) = super::import_thunk_form(target).expect("MachO realizes a thunk");
        let layout = FinalImageLayout {
            text_address: 0x1_0000_0000,
            data_address: 0x1_0000_2000,
            ..FinalImageLayout::default()
        };
        let mut text = vec![0xabu8; 12];
        text.extend_from_slice(&[
            0x10, 0x00, 0x00, 0x14, // B X16 — not the closed ADRP
            0x10, 0x02, 0x40, 0xf9, 0x00, 0x02, 0x1f, 0xd6,
        ]);
        let mut image = FinalImage::with_capacity(
            target,
            FinalImageMemory {
                text: text.clone(),
                data: vec![0u8; 8],
                ..FinalImageMemory::default()
            },
            Default::default(),
            0,
            0,
            0,
        );
        image.executable_regions.extend([
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::CompilerFunction,
                section_offset: 0,
                byte_count: 12,
                symbol: "entry".into(),
                footprint: None,
            },
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::ImportThunk,
                section_offset: 12,
                byte_count: extent,
                symbol: "imported".into(),
                footprint: Some(footprint),
            },
        ]);
        image.data_regions.push(FinalDataRegion {
            origin: FinalDataRegionOrigin::ImportBindingSlot,
            section_offset: 0,
            byte_count: 8,
            symbol: "imported".into(),
        });
        let mut executable = vec![0xffu8; TEXT_FILE_OFFSET as usize];
        executable.extend_from_slice(&text);
        executable.extend_from_slice(&[0u8; 8]);
        executable.extend_from_slice(&[0x00u8; 32]);
        (
            NativePlacedImageEvidence::from_parts(
                target,
                TEXT_FILE_OFFSET,
                image::place_executable_regions(&image, layout).expect("the fixture places"),
                TEXT_FILE_OFFSET + text.len() as u64,
                image::place_data_regions(&image, layout).expect("the fixture places"),
                0,
                image::PlacedDataRegionInventory::empty(),
            ),
            executable,
        )
    };
    let wrong_sidecar = native_sidecar_with_profile(
        &wrong_macho.1,
        native_semantic_profile_identity(target::NativeTarget::macos_arm64()),
        wrong_macho.0.to_bytes(),
    );
    assert_eq!(
        rejecting_subject(verify_native_proof_sidecar(
            &wrong_macho.1,
            &wrong_sidecar.to_bytes(),
            &offered_policy(&wrong_sidecar)
        )),
        "native executable inventory",
        "a MachO thunk over non-ADRP bytes must reject at replay"
    );

    // The pairing is bidirectional: a committed binding slot that no thunk
    // loads rejects, and a thunk whose decoded pointer meets no committed
    // slot rejects. Both mutations keep their inventories honest — the slot
    // inventory placed under a shifted data_address seals correctly, so the
    // only failing leg is the thunk↔slot join itself.
    let shifted_slot = {
        let target = target::NativeTarget::macos_arm64();
        let (extent, footprint) = super::import_thunk_form(target).expect("MachO realizes a thunk");
        let layout = FinalImageLayout {
            text_address: 0x1_0000_0000,
            data_address: 0x1_0000_3000,
            ..FinalImageLayout::default()
        };
        let mut text = vec![0xabu8; 12];
        text.extend_from_slice(&[
            0x10, 0x00, 0x00, 0xd0, // ADRP X16, +0x2000
            0x10, 0x02, 0x40, 0xf9, // LDR X16, [X16, #0]
            0x00, 0x02, 0x1f, 0xd6, // BR X16
        ]);
        let data = [0u8; 8];
        let mut image = FinalImage::with_capacity(
            target,
            FinalImageMemory {
                text: text.clone(),
                data: data.to_vec(),
                ..FinalImageMemory::default()
            },
            Default::default(),
            0,
            0,
            0,
        );
        image.executable_regions.extend([
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::CompilerFunction,
                section_offset: 0,
                byte_count: 12,
                symbol: "entry".into(),
                footprint: None,
            },
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::ImportThunk,
                section_offset: 12,
                byte_count: extent,
                symbol: "imported".into(),
                footprint: Some(footprint),
            },
        ]);
        image.data_regions.push(FinalDataRegion {
            origin: FinalDataRegionOrigin::ImportBindingSlot,
            section_offset: 0,
            byte_count: 8,
            symbol: "imported".into(),
        });
        let mut executable = vec![0xffu8; TEXT_FILE_OFFSET as usize];
        executable.extend_from_slice(&text);
        executable.extend_from_slice(&data);
        executable.extend_from_slice(&[0x00u8; 32]);
        (
            NativePlacedImageEvidence::from_parts(
                target,
                TEXT_FILE_OFFSET,
                image::place_executable_regions(&image, layout).expect("the fixture places"),
                TEXT_FILE_OFFSET + text.len() as u64,
                image::place_data_regions(&image, layout).expect("the fixture places"),
                0,
                image::PlacedDataRegionInventory::empty(),
            ),
            executable,
        )
    };
    let shifted_sidecar = native_sidecar_with_profile(
        &shifted_slot.1,
        native_semantic_profile_identity(target::NativeTarget::macos_arm64()),
        shifted_slot.0.to_bytes(),
    );
    assert_eq!(
        rejecting_subject(verify_native_proof_sidecar(
            &shifted_slot.1,
            &shifted_sidecar.to_bytes(),
            &offered_policy(&shifted_sidecar)
        )),
        "native executable inventory",
        "a thunk whose decoded pointer binds no committed slot must reject"
    );

    // A slot that seals under a different origin is honest custody but no
    // longer pairs: the thunk's decoded pointer finds zero binding slots.
    let unbound = {
        let target = target::NativeTarget::macos_arm64();
        let (extent, footprint) = super::import_thunk_form(target).expect("MachO realizes a thunk");
        let layout = FinalImageLayout {
            text_address: 0x1_0000_0000,
            data_address: 0x1_0000_2000,
            ..FinalImageLayout::default()
        };
        let mut text = vec![0xabu8; 12];
        text.extend_from_slice(&[
            0x10, 0x00, 0x00, 0xd0, 0x10, 0x02, 0x40, 0xf9, 0x00, 0x02, 0x1f, 0xd6,
        ]);
        let data = [0u8; 8];
        let mut image = FinalImage::with_capacity(
            target,
            FinalImageMemory {
                text: text.clone(),
                data: data.to_vec(),
                ..FinalImageMemory::default()
            },
            Default::default(),
            0,
            0,
            0,
        );
        image.executable_regions.extend([
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::CompilerFunction,
                section_offset: 0,
                byte_count: 12,
                symbol: "entry".into(),
                footprint: None,
            },
            FinalExecutableRegion {
                origin: FinalExecutableRegionOrigin::ImportThunk,
                section_offset: 12,
                byte_count: extent,
                symbol: "imported".into(),
                footprint: Some(footprint),
            },
        ]);
        image.data_regions.push(FinalDataRegion {
            origin: FinalDataRegionOrigin::CompilerData,
            section_offset: 0,
            byte_count: 8,
            symbol: "imported".into(),
        });
        let mut executable = vec![0xffu8; TEXT_FILE_OFFSET as usize];
        executable.extend_from_slice(&text);
        executable.extend_from_slice(&data);
        executable.extend_from_slice(&[0x00u8; 32]);
        (
            NativePlacedImageEvidence::from_parts(
                target,
                TEXT_FILE_OFFSET,
                image::place_executable_regions(&image, layout).expect("the fixture places"),
                TEXT_FILE_OFFSET + text.len() as u64,
                image::place_data_regions(&image, layout).expect("the fixture places"),
                0,
                image::PlacedDataRegionInventory::empty(),
            ),
            executable,
        )
    };
    let unbound_sidecar = native_sidecar_with_profile(
        &unbound.1,
        native_semantic_profile_identity(target::NativeTarget::macos_arm64()),
        unbound.0.to_bytes(),
    );
    assert_eq!(
        rejecting_subject(verify_native_proof_sidecar(
            &unbound.1,
            &unbound_sidecar.to_bytes(),
            &offered_policy(&unbound_sidecar)
        )),
        "native executable inventory",
        "a thunk whose slot is not claimed as a binding slot must reject"
    );
}

/// The Coff pairing leg, exercised under honest seals: a thunk whose decoded
/// `jmp [rip+disp32]` names no committed `ImportBindingSlot`, a committed
/// slot no thunk loads, and a slot at the bound address under a different
/// symbol all reject at the pairing join — nothing about the inventories is
/// dishonest, only the thunk↔slot correspondence fails.
#[test]
fn coff_thunks_pair_bidirectionally_with_the_rdata_slots() {
    let subject = |name: &'static str, pair: &(NativePlacedImageEvidence, Vec<u8>)| {
        let sidecar = native_sidecar_with_profile(
            &pair.1,
            native_semantic_profile_identity(target::NativeTarget::windows_x64()),
            pair.0.to_bytes(),
        );
        assert_eq!(
            rejecting_subject(verify_native_proof_sidecar(
                &pair.1,
                &sidecar.to_bytes(),
                &offered_policy(&sidecar)
            )),
            "native executable inventory",
            "{name} must reject at replay"
        );
    };

    // The slot seals honestly but at a base the thunk's displacement does
    // not name — the decoded operand finds zero binding slots there.
    let shifted: [u8; 16] = std::array::from_fn(|index| (index * 13 + 7) as u8);
    subject(
        "a thunk whose displacement names no committed slot",
        &windows_thunk_pair_over(
            &shifted,
            vec![FinalDataRegion {
                origin: FinalDataRegionOrigin::ImportBindingSlot,
                section_offset: 8,
                byte_count: 8,
                symbol: "host_call".into(),
            }],
            COFF_SLOT_ADDRESS - 8 + 0x1000,
        ),
    );

    // A committed slot no thunk's displacement reaches is orphaned custody:
    // the bidirectional join refuses it even while the honest slot pairs.
    let extra: [u8; 24] = std::array::from_fn(|index| (index * 13 + 7) as u8);
    subject(
        "a committed slot no thunk loads",
        &windows_thunk_pair_over(
            &extra,
            vec![
                FinalDataRegion {
                    origin: FinalDataRegionOrigin::ImportBindingSlot,
                    section_offset: 8,
                    byte_count: 8,
                    symbol: "host_call".into(),
                },
                FinalDataRegion {
                    origin: FinalDataRegionOrigin::ImportBindingSlot,
                    section_offset: 16,
                    byte_count: 8,
                    symbol: "other_import".into(),
                },
            ],
            COFF_SLOT_ADDRESS - 8,
        ),
    );

    // The bound address is committed under another symbol — an address match
    // alone does not pair a thunk with a slot.
    let renamed: [u8; 16] = std::array::from_fn(|index| (index * 13 + 7) as u8);
    subject(
        "a slot at the bound address under another symbol",
        &windows_thunk_pair_over(
            &renamed,
            vec![FinalDataRegion {
                origin: FinalDataRegionOrigin::ImportBindingSlot,
                section_offset: 8,
                byte_count: 8,
                symbol: "other_import".into(),
            }],
            COFF_SLOT_ADDRESS - 8,
        ),
    );
}

/// The import-data leg's closed decode rules: every row in the import-data
/// inventory is an `ImportBindingSlot`, a populated inventory exists only
/// under (x86_64, Coff), and an empty inventory declares file offset 0.
#[test]
fn import_data_inventory_decodes_only_binding_slots_on_coff() {
    let (evidence, executable) = windows_thunk_pair();
    // The honest Coff section round-trips byte-identically and replays.
    let decoded = NativePlacedImageEvidence::from_bytes(&evidence.to_bytes()).expect("decodes");
    assert_eq!(decoded, evidence);
    decoded
        .replay_against(&executable)
        .expect("an honest Coff thunk↔slot pairing replays");

    let malformed = |name: &'static str, evidence: &NativePlacedImageEvidence| {
        assert!(
            matches!(
                NativePlacedImageEvidence::from_bytes(&evidence.to_bytes()),
                Err(NativeEvidenceError::Malformed(_))
            ),
            "{name} must reject as malformed evidence"
        );
    };

    // A populated import-data inventory under a target that emits no such
    // extent is a claim no realization path produces. Built over the Linux
    // fixture so no thunk row fires an earlier decode rule.
    let (mut foreign, _) = honest_pair();
    foreign.import_data_inventory = evidence.import_data_inventory.clone();
    foreign.import_data_file_offset = 64;
    malformed("a populated import-data inventory on Elf", &foreign);

    // A non-slot row inside the import-data inventory.
    let mut wrong_origin = evidence.clone();
    wrong_origin.import_data_inventory.regions[0].origin = FinalDataRegionOrigin::CompilerData;
    malformed(
        "a compiler-data row inside the import-data inventory",
        &wrong_origin,
    );

    // An empty inventory claiming a nonzero extent offset is never the
    // canonical encoding.
    let mut bad_offset = evidence.clone();
    bad_offset.import_data_inventory = image::PlacedDataRegionInventory::empty();
    bad_offset.import_data_file_offset = 64;
    malformed(
        "an empty import-data inventory at a nonzero offset",
        &bad_offset,
    );
}

// ---------------------------------------------------------------------
// Container-declared entry custody: the entry point a container declares
// to its loader is re-derived from the published bytes and must land on
// the start of a placed region the committed text inventory covers.
// ---------------------------------------------------------------------

/// A minimal ELF64 little-endian header declaring `entry_va` in `e_entry`,
/// zero-padded so the declared text extent still lands at
/// [`TEXT_FILE_OFFSET`].
fn elf64_header(entry_va: u64) -> Vec<u8> {
    let mut header = vec![0u8; TEXT_FILE_OFFSET as usize];
    header[..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
    header[4] = 2; // ELFCLASS64
    header[5] = 1; // little-endian
    header[24..32].copy_from_slice(&entry_va.to_le_bytes());
    header
}

/// An honest (x86_64, Elf) pair whose container declares `entry_va`: the
/// three-region [`placed_inventory`] fixture lands `entry`/`host_call`/`tail`
/// at `0x4010_0000`/`+8`/`+16` with unclassified gaps at `+4` and `+12`.
fn elf_entry_pair(entry_va: u64) -> (NativePlacedImageEvidence, Vec<u8>) {
    let text: [u8; TEXT_LEN] = std::array::from_fn(|index| (index * 7 + 3) as u8);
    let data: [u8; DATA_LEN] = std::array::from_fn(|index| (index * 11 + 5) as u8);
    let mut executable = elf64_header(entry_va);
    executable.extend_from_slice(&text);
    executable.extend_from_slice(&data);
    executable.extend_from_slice(&[0x00u8; 32]);
    let evidence = NativePlacedImageEvidence::from_parts(
        target::NativeTarget::linux_x64(),
        TEXT_FILE_OFFSET,
        placed_inventory(&text),
        DATA_FILE_OFFSET,
        placed_data_inventory(&data),
        0,
        image::PlacedDataRegionInventory::empty(),
    );
    (evidence, executable)
}

/// The file offset the PE fixture lays text at: the PE32+ header surface
/// through `ImageBase` already runs past 0xB8.
const COFF_TEXT_FILE_OFFSET: u64 = 0x200;

/// A minimal PE32+ container header: the DOS stub's `e_lfanew` names the
/// PE signature at 0x80, and the optional header carries `image_base` and
/// `entry_rva`. Padded so the declared text extent stays at
/// [`COFF_TEXT_FILE_OFFSET`].
fn pe32_plus_header(image_base: u64, entry_rva: u32) -> Vec<u8> {
    const PE_OFFSET: usize = 0x80;
    let mut header = vec![0u8; COFF_TEXT_FILE_OFFSET as usize];
    header[..2].copy_from_slice(&[b'M', b'Z']);
    header[0x3c..0x40].copy_from_slice(&(PE_OFFSET as u32).to_le_bytes());
    header[PE_OFFSET..PE_OFFSET + 4].copy_from_slice(&[b'P', b'E', 0, 0]);
    let optional = PE_OFFSET + 24;
    header[optional..optional + 2].copy_from_slice(&0x20bu16.to_le_bytes());
    header[optional + 16..optional + 20].copy_from_slice(&entry_rva.to_le_bytes());
    header[optional + 24..optional + 32].copy_from_slice(&image_base.to_le_bytes());
    header
}

/// An honest (x86_64, Coff) pair: one `entry` region covers the whole text
/// extent, placed at `image_base + entry_rva` so the declared entry and the
/// committed region agree iff the fixture caller keeps them equal.
fn coff_entry_pair(image_base: u64, entry_rva: u32) -> (NativePlacedImageEvidence, Vec<u8>) {
    const TEXT_RVA: u64 = 0x1000;
    let target = target::NativeTarget::windows_x64();
    let text: [u8; TEXT_LEN] = std::array::from_fn(|index| (index * 7 + 3) as u8);
    let data: [u8; DATA_LEN] = std::array::from_fn(|index| (index * 11 + 5) as u8);
    let mut image = FinalImage::with_capacity(
        target,
        FinalImageMemory {
            text: text.to_vec(),
            ..FinalImageMemory::default()
        },
        Default::default(),
        0,
        0,
        0,
    );
    image.executable_regions.push(FinalExecutableRegion {
        origin: FinalExecutableRegionOrigin::CompilerFunction,
        section_offset: 0,
        byte_count: TEXT_LEN,
        symbol: "entry".into(),
        footprint: None,
    });
    let executable_inventory = image::place_executable_regions(
        &image,
        FinalImageLayout {
            text_address: image_base + TEXT_RVA,
            ..FinalImageLayout::default()
        },
    )
    .expect("the Coff entry fixture places");
    let data_inventory = placed_data_inventory(&data);
    let mut executable = pe32_plus_header(image_base, entry_rva);
    executable.extend_from_slice(&text);
    executable.extend_from_slice(&data);
    executable.extend_from_slice(&[0x00u8; 32]);
    let evidence = NativePlacedImageEvidence::from_parts(
        target,
        COFF_TEXT_FILE_OFFSET,
        executable_inventory,
        COFF_TEXT_FILE_OFFSET + TEXT_LEN as u64,
        data_inventory,
        0,
        image::PlacedDataRegionInventory::empty(),
    );
    (evidence, executable)
}

/// A minimal Mach-O 64 little-endian header: `mach_header_64` declaring two
/// load commands — the emitted `__TEXT` `LC_SEGMENT_64` (`vmaddr`,
/// `fileoff`) and `LC_MAIN` carrying `entryoff`. `sizeofcmds` and `ncmds`
/// stay honest so the command walk is real.
fn macho64_header(vmaddr: u64, fileoff: u64, entryoff: u64) -> Vec<u8> {
    const SEGMENT_64_SIZE: usize = 72;
    const MAIN_SIZE: usize = 24;
    let mut header = vec![0u8; 32 + SEGMENT_64_SIZE + MAIN_SIZE];
    header[..4].copy_from_slice(&0xfeed_facfu32.to_le_bytes());
    header[16..20].copy_from_slice(&2u32.to_le_bytes());
    header[20..24].copy_from_slice(&((SEGMENT_64_SIZE + MAIN_SIZE) as u32).to_le_bytes());
    let segment = 32;
    header[segment..segment + 4].copy_from_slice(&0x19u32.to_le_bytes());
    header[segment + 4..segment + 8].copy_from_slice(&(SEGMENT_64_SIZE as u32).to_le_bytes());
    header[segment + 8..segment + 24].copy_from_slice(b"__TEXT\0\0\0\0\0\0\0\0\0\0");
    header[segment + 24..segment + 32].copy_from_slice(&vmaddr.to_le_bytes());
    header[segment + 40..segment + 48].copy_from_slice(&fileoff.to_le_bytes());
    let main = segment + SEGMENT_64_SIZE;
    header[main..main + 4].copy_from_slice(&0x8000_0028u32.to_le_bytes());
    header[main + 4..main + 8].copy_from_slice(&(MAIN_SIZE as u32).to_le_bytes());
    header[main + 8..main + 16].copy_from_slice(&entryoff.to_le_bytes());
    header
}

/// An honest (aarch64, MachO) pair: the emitted `__TEXT` segment maps the
/// header bytes themselves (`vmaddr` = `MACHO_EXECUTABLE_BASE`,
/// `fileoff` = 0), the text extent begins right after the load commands,
/// and `LC_MAIN` names `entryoff` inside it. One `entry` region covers the
/// whole extent, so the declared entry and the committed boundary agree iff
/// `vmaddr + entryoff` equals the placed `text_address`.
fn macho_entry_pair(entryoff: u64) -> (NativePlacedImageEvidence, Vec<u8>) {
    const MACHO_EXECUTABLE_BASE: u64 = 0x1_0000_0000;
    const TEXT_FILE_AT: u64 = 128;
    let target = target::NativeTarget::macos_arm64();
    let text: [u8; TEXT_LEN] = std::array::from_fn(|index| (index * 7 + 3) as u8);
    let data: [u8; DATA_LEN] = std::array::from_fn(|index| (index * 11 + 5) as u8);
    let mut image = FinalImage::with_capacity(
        target,
        FinalImageMemory {
            text: text.to_vec(),
            ..FinalImageMemory::default()
        },
        Default::default(),
        0,
        0,
        0,
    );
    image.executable_regions.push(FinalExecutableRegion {
        origin: FinalExecutableRegionOrigin::CompilerFunction,
        section_offset: 0,
        byte_count: TEXT_LEN,
        symbol: "entry".into(),
        footprint: None,
    });
    let executable_inventory = image::place_executable_regions(
        &image,
        FinalImageLayout {
            text_address: MACHO_EXECUTABLE_BASE + TEXT_FILE_AT,
            ..FinalImageLayout::default()
        },
    )
    .expect("the Mach-O entry fixture places");
    let data_inventory = placed_data_inventory(&data);
    let mut executable = macho64_header(MACHO_EXECUTABLE_BASE, 0, entryoff);
    assert_eq!(executable.len() as u64, TEXT_FILE_AT);
    executable.extend_from_slice(&text);
    executable.extend_from_slice(&data);
    executable.extend_from_slice(&[0x00u8; 32]);
    let evidence = NativePlacedImageEvidence::from_parts(
        target,
        TEXT_FILE_AT,
        executable_inventory,
        TEXT_FILE_AT + TEXT_LEN as u64,
        data_inventory,
        0,
        image::PlacedDataRegionInventory::empty(),
    );
    (evidence, executable)
}

fn entry_rejection(evidence: &NativePlacedImageEvidence, executable: &[u8]) -> String {
    evidence
        .replay_against(executable)
        .expect_err("a misplaced declared entry must fail custody replay")
}

/// The declared-entry leg across the three emitted container families: a
/// loader-visible entry that names a placed region start replays; one that
/// lands mid-region, on an unclassified gap boundary, or outside the
/// committed coverage rejects by name; a container carrying no checkable
/// entry claim leaves the leg silent.
#[test]
fn container_declared_entry_lands_on_a_committed_region_boundary() {
    // ELF64: `e_entry` is the entry virtual address outright.
    for entry in [0x4010_0000, 0x4010_0008, 0x4010_0010] {
        let (evidence, executable) = elf_entry_pair(entry);
        evidence
            .replay_against(&executable)
            .unwrap_or_else(|reason| panic!("region-start entry {entry:#x} must replay: {reason}"));
    }
    // Mid-region and gap-start entries are not placed boundaries — the gap
    // start shares a file boundary but claims no checked region.
    for entry in [0x4010_0002, 0x4010_0004, 0x4010_000c, 0x4010_1000] {
        let (evidence, executable) = elf_entry_pair(entry);
        let reason = entry_rejection(&evidence, &executable);
        assert!(
            reason.contains("does not start a placed executable region"),
            "entry {entry:#x} rejected for the wrong reason: {reason}"
        );
    }
    // A null entry and a container that does not parse as ELF64 both leave
    // the leg silent.
    for entry in [0] {
        let (evidence, executable) = elf_entry_pair(entry);
        evidence
            .replay_against(&executable)
            .expect("a null e_entry declares no checkable entry");
    }
    let (evidence, executable) = honest_pair();
    evidence
        .replay_against(&executable)
        .expect("a container without ELF magic declares no checkable entry");

    // The same rejection surfaces through the product leg as a named
    // rejection on the native inventory, not an opaque custody failure.
    let (evidence, executable) = elf_entry_pair(0x4010_0002);
    let sidecar = native_sidecar(&executable, evidence.to_bytes());
    let outcome =
        verify_native_proof_sidecar(&executable, &sidecar.to_bytes(), &offered_policy(&sidecar));
    assert!(
        rejecting_subject(outcome).contains("native executable inventory"),
        "a mid-region entry must reject on the native inventory"
    );

    // PE32+: `ImageBase + AddressOfEntryPoint` is the entry virtual address.
    const IMAGE_BASE: u64 = 0x1_4000_0000;
    const TEXT_RVA: u32 = 0x1000;
    let (evidence, executable) = coff_entry_pair(IMAGE_BASE, TEXT_RVA);
    evidence
        .replay_against(&executable)
        .expect("the Coff entry at the placed text start replays");
    for entry_rva in [TEXT_RVA + 1, TEXT_RVA + TEXT_LEN as u32] {
        let (evidence, executable) = coff_entry_pair(IMAGE_BASE, entry_rva);
        let reason = entry_rejection(&evidence, &executable);
        assert!(
            reason.contains("does not start a placed executable region"),
            "Coff entry RVA {entry_rva:#x} rejected for the wrong reason: {reason}"
        );
    }
    let (evidence, executable) = coff_entry_pair(IMAGE_BASE, 0);
    evidence
        .replay_against(&executable)
        .expect("a null AddressOfEntryPoint declares no checkable entry");

    // Mach-O 64: `LC_MAIN`'s `entryoff` maps through `__TEXT`'s
    // `vmaddr`/`fileoff`; the text extent begins at file offset 128.
    const TEXT_FILE_AT: u64 = 128;
    let (evidence, executable) = macho_entry_pair(TEXT_FILE_AT);
    evidence
        .replay_against(&executable)
        .expect("the Mach-O entry at the placed text start replays");
    for entryoff in [TEXT_FILE_AT + 1, TEXT_FILE_AT + TEXT_LEN as u64] {
        let (evidence, executable) = macho_entry_pair(entryoff);
        let reason = entry_rejection(&evidence, &executable);
        assert!(
            reason.contains("does not start a placed executable region"),
            "Mach-O entryoff {entryoff:#x} rejected for the wrong reason: {reason}"
        );
    }
    // An `entryoff` below `__TEXT`'s `fileoff` is not inside the segment —
    // no checkable entry claim: raise `fileoff` past `entryoff` so the
    // mapping underflows.
    let (evidence, mut executable) = macho_entry_pair(8);
    executable[72..80].copy_from_slice(&16u64.to_le_bytes());
    evidence
        .replay_against(&executable)
        .expect("an entry file offset outside __TEXT declares no checkable claim");
    // And a container whose command walk never names `LC_MAIN` declares no
    // checkable entry either.
    let (evidence, mut executable) = macho_entry_pair(TEXT_FILE_AT);
    executable[104..108].copy_from_slice(&0x2u32.to_le_bytes()); // LC_SEGMENT instead of LC_MAIN
    evidence
        .replay_against(&executable)
        .expect("a Mach-O container without LC_MAIN declares no checkable entry");
}

// ---------------------------------------------------------------------
// Container-declared loadable coverage: the declared extents are custody
// claims the container itself must ratify — pairwise disjoint, and each
// inside a loadable file range carrying its role (executable for text,
// writable for initialized data, any role for import data). The check is
// one-directional containment because the emitted layouts map bytes past
// the extents (ELF headers inside the executable `PT_LOAD`, PE raw
// padding past the extent end, Mach-O `__TEXT` covering its own header).
// ---------------------------------------------------------------------

/// The file offset the loadable fixtures lay text at: program headers and
/// section tables live in the header bytes, so the declared extents sit
/// past them at 0x200.
const LOADABLE_TEXT_AT: u64 = 0x200;

/// An ELF64 header whose program-header table at `e_phoff` 64 carries the
/// caller's `PT_LOAD` entries `(flags, file offset, file size)`; the bytes
/// pad to [`LOADABLE_TEXT_AT`] so the extents sit past the table. `e_entry`
/// stays null, keeping the entry leg silent.
fn elf64_loadable_header(loads: &[(u32, u64, u64)]) -> Vec<u8> {
    let mut header = vec![0u8; LOADABLE_TEXT_AT as usize];
    header[..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
    header[4] = 2; // ELFCLASS64
    header[5] = 1; // little-endian
    header[32..40].copy_from_slice(&64u64.to_le_bytes()); // e_phoff
    header[54..56].copy_from_slice(&56u16.to_le_bytes()); // e_phentsize
    header[56..58].copy_from_slice(&(loads.len() as u16).to_le_bytes()); // e_phnum
    for (index, (flags, offset, size)) in loads.iter().enumerate() {
        let at = 64 + index * 56;
        header[at..at + 4].copy_from_slice(&1u32.to_le_bytes()); // PT_LOAD
        header[at + 4..at + 8].copy_from_slice(&flags.to_le_bytes());
        header[at + 8..at + 16].copy_from_slice(&offset.to_le_bytes());
        header[at + 32..at + 40].copy_from_slice(&size.to_le_bytes());
    }
    header
}

/// An honest (x86_64, Elf) pair under the caller's `PT_LOAD` table: the
/// text extent at [`LOADABLE_TEXT_AT`], the data extent right after it.
fn elf_loadable_pair(loads: &[(u32, u64, u64)]) -> (NativePlacedImageEvidence, Vec<u8>) {
    let text: [u8; TEXT_LEN] = std::array::from_fn(|index| (index * 7 + 3) as u8);
    let data: [u8; DATA_LEN] = std::array::from_fn(|index| (index * 11 + 5) as u8);
    let mut executable = elf64_loadable_header(loads);
    executable.extend_from_slice(&text);
    executable.extend_from_slice(&data);
    executable.extend_from_slice(&[0x00u8; 32]);
    let evidence = NativePlacedImageEvidence::from_parts(
        target::NativeTarget::linux_x64(),
        LOADABLE_TEXT_AT,
        placed_inventory(&text),
        LOADABLE_TEXT_AT + TEXT_LEN as u64,
        placed_data_inventory(&data),
        0,
        image::PlacedDataRegionInventory::empty(),
    );
    (evidence, executable)
}

/// A PE32+ header whose section table carries the caller's sections
/// `(SizeOfRawData, PointerToRawData, characteristics)` — `num_sections`
/// and `SizeOfOptionalHeader` stay honest so the walk is real. The table
/// ends exactly at [`COFF_TEXT_FILE_OFFSET`]; `AddressOfEntryPoint` stays
/// null, keeping the entry leg silent.
fn pe32_plus_loadable_header(sections: &[(u32, u32, u32)]) -> Vec<u8> {
    const PE_OFFSET: usize = 0x80;
    const OPTIONAL_SIZE: u16 = 0xf0;
    let mut header = vec![0u8; COFF_TEXT_FILE_OFFSET as usize];
    header[..2].copy_from_slice(b"MZ");
    header[0x3c..0x40].copy_from_slice(&(PE_OFFSET as u32).to_le_bytes());
    header[PE_OFFSET..PE_OFFSET + 4].copy_from_slice(b"PE\0\0");
    let coff = PE_OFFSET + 4;
    header[coff + 2..coff + 4].copy_from_slice(&(sections.len() as u16).to_le_bytes());
    header[coff + 16..coff + 18].copy_from_slice(&OPTIONAL_SIZE.to_le_bytes());
    let optional = coff + 20;
    header[optional..optional + 2].copy_from_slice(&0x20bu16.to_le_bytes());
    let table = optional + OPTIONAL_SIZE as usize;
    for (index, (raw_size, raw_offset, characteristics)) in sections.iter().enumerate() {
        let at = table + index * 40;
        header[at + 16..at + 20].copy_from_slice(&raw_size.to_le_bytes());
        header[at + 20..at + 24].copy_from_slice(&raw_offset.to_le_bytes());
        header[at + 36..at + 40].copy_from_slice(&characteristics.to_le_bytes());
    }
    header
}

/// An honest (x86_64, Coff) pair under the caller's section table: the
/// text extent at [`COFF_TEXT_FILE_OFFSET`], the data extent right after
/// it, no import-data extent.
fn pe_loadable_pair(sections: &[(u32, u32, u32)]) -> (NativePlacedImageEvidence, Vec<u8>) {
    let target = target::NativeTarget::windows_x64();
    let text: [u8; TEXT_LEN] = std::array::from_fn(|index| (index * 7 + 3) as u8);
    let data: [u8; DATA_LEN] = std::array::from_fn(|index| (index * 11 + 5) as u8);
    let mut image = FinalImage::with_capacity(
        target,
        FinalImageMemory {
            text: text.to_vec(),
            ..FinalImageMemory::default()
        },
        Default::default(),
        0,
        0,
        0,
    );
    image.executable_regions.push(FinalExecutableRegion {
        origin: FinalExecutableRegionOrigin::CompilerFunction,
        section_offset: 0,
        byte_count: TEXT_LEN,
        symbol: "entry".into(),
        footprint: None,
    });
    let executable_inventory = image::place_executable_regions(
        &image,
        FinalImageLayout {
            text_address: 0x1_4000_1000,
            ..FinalImageLayout::default()
        },
    )
    .expect("the Coff loadable fixture places");
    let mut executable = pe32_plus_loadable_header(sections);
    executable.extend_from_slice(&text);
    executable.extend_from_slice(&data);
    executable.extend_from_slice(&[0x00u8; 32]);
    let evidence = NativePlacedImageEvidence::from_parts(
        target,
        COFF_TEXT_FILE_OFFSET,
        executable_inventory,
        COFF_TEXT_FILE_OFFSET + TEXT_LEN as u64,
        placed_data_inventory(&data),
        0,
        image::PlacedDataRegionInventory::empty(),
    );
    (evidence, executable)
}

/// The Coff thunk fixture inside a real PE32+ header carrying `sections`:
/// the thunk's `.rdata` slot extent lands right after the text extent at
/// [`COFF_TEXT_FILE_OFFSET`], so the section table decides whether the
/// import-data extent finds loadable coverage.
fn windows_loadable_thunk_pair(
    sections: &[(u32, u32, u32)],
) -> (NativePlacedImageEvidence, Vec<u8>) {
    let import_data: [u8; 16] = std::array::from_fn(|index| (index * 13 + 7) as u8);
    windows_thunk_pair_in(
        pe32_plus_loadable_header(sections),
        &import_data,
        vec![FinalDataRegion {
            origin: FinalDataRegionOrigin::ImportBindingSlot,
            section_offset: 8,
            byte_count: 8,
            symbol: "host_call".into(),
        }],
        COFF_SLOT_ADDRESS - 8,
    )
}

/// A Mach-O 64 fixture declaring `__TEXT` and `__DATA` `LC_SEGMENT_64`
/// commands with the caller's `initprot` bits plus `LC_MAIN`: `__TEXT`
/// maps the header bytes and the text extent from `fileoff` 0, `__DATA`
/// covers exactly the declared data extent, and `entryoff` names the text
/// start so the entry leg stays satisfied under every `initprot` choice.
fn macho_loadable_pair(
    text_initprot: u32,
    data_initprot: u32,
) -> (NativePlacedImageEvidence, Vec<u8>) {
    const MACHO_EXECUTABLE_BASE: u64 = 0x1_0000_0000;
    const SEGMENT_64_SIZE: usize = 72;
    const MAIN_SIZE: usize = 24;
    const TEXT_AT: u64 = (32 + 2 * SEGMENT_64_SIZE + MAIN_SIZE) as u64;
    const DATA_AT: u64 = TEXT_AT + TEXT_LEN as u64;
    let target = target::NativeTarget::macos_arm64();
    let text: [u8; TEXT_LEN] = std::array::from_fn(|index| (index * 7 + 3) as u8);
    let data: [u8; DATA_LEN] = std::array::from_fn(|index| (index * 11 + 5) as u8);
    let mut image = FinalImage::with_capacity(
        target,
        FinalImageMemory {
            text: text.to_vec(),
            ..FinalImageMemory::default()
        },
        Default::default(),
        0,
        0,
        0,
    );
    image.executable_regions.push(FinalExecutableRegion {
        origin: FinalExecutableRegionOrigin::CompilerFunction,
        section_offset: 0,
        byte_count: TEXT_LEN,
        symbol: "entry".into(),
        footprint: None,
    });
    let executable_inventory = image::place_executable_regions(
        &image,
        FinalImageLayout {
            text_address: MACHO_EXECUTABLE_BASE + TEXT_AT,
            ..FinalImageLayout::default()
        },
    )
    .expect("the Mach-O loadable fixture places");
    let data_inventory = placed_data_inventory(&data);
    let mut executable = vec![0u8; TEXT_AT as usize];
    executable[..4].copy_from_slice(&0xfeed_facfu32.to_le_bytes());
    executable[16..20].copy_from_slice(&3u32.to_le_bytes()); // ncmds
    executable[20..24].copy_from_slice(&((2 * SEGMENT_64_SIZE + MAIN_SIZE) as u32).to_le_bytes());
    // __TEXT: vmaddr, fileoff 0, filesize covering header and text.
    executable[32..36].copy_from_slice(&0x19u32.to_le_bytes());
    executable[36..40].copy_from_slice(&(SEGMENT_64_SIZE as u32).to_le_bytes());
    executable[40..56].copy_from_slice(b"__TEXT\0\0\0\0\0\0\0\0\0\0");
    executable[56..64].copy_from_slice(&MACHO_EXECUTABLE_BASE.to_le_bytes());
    executable[72..80].copy_from_slice(&0u64.to_le_bytes()); // fileoff
    executable[80..88].copy_from_slice(&DATA_AT.to_le_bytes()); // filesize
    executable[92..96].copy_from_slice(&text_initprot.to_le_bytes());
    // __DATA: fileoff covering exactly the declared data extent.
    let data_segment = 32 + SEGMENT_64_SIZE;
    executable[data_segment..data_segment + 4].copy_from_slice(&0x19u32.to_le_bytes());
    executable[data_segment + 4..data_segment + 8]
        .copy_from_slice(&(SEGMENT_64_SIZE as u32).to_le_bytes());
    executable[data_segment + 8..data_segment + 24].copy_from_slice(b"__DATA\0\0\0\0\0\0\0\0\0\0");
    executable[data_segment + 24..data_segment + 32]
        .copy_from_slice(&(MACHO_EXECUTABLE_BASE + DATA_AT).to_le_bytes());
    executable[data_segment + 40..data_segment + 48].copy_from_slice(&DATA_AT.to_le_bytes());
    executable[data_segment + 48..data_segment + 56]
        .copy_from_slice(&(DATA_LEN as u64).to_le_bytes());
    executable[data_segment + 60..data_segment + 64].copy_from_slice(&data_initprot.to_le_bytes());
    let main = 32 + 2 * SEGMENT_64_SIZE;
    executable[main..main + 4].copy_from_slice(&0x8000_0028u32.to_le_bytes());
    executable[main + 4..main + 8].copy_from_slice(&(MAIN_SIZE as u32).to_le_bytes());
    executable[main + 8..main + 16].copy_from_slice(&TEXT_AT.to_le_bytes());
    executable.extend_from_slice(&text);
    executable.extend_from_slice(&data);
    executable.extend_from_slice(&[0x00u8; 32]);
    let evidence = NativePlacedImageEvidence::from_parts(
        target,
        TEXT_AT,
        executable_inventory,
        DATA_AT,
        data_inventory,
        0,
        image::PlacedDataRegionInventory::empty(),
    );
    (evidence, executable)
}

fn loadable_rejection(pair: &(NativePlacedImageEvidence, Vec<u8>)) -> String {
    pair.0
        .replay_against(&pair.1)
        .expect_err("a miscovered declared extent must fail custody replay")
}

/// The loadable-coverage leg across the three emitted container families:
/// declared extents must be pairwise disjoint and sit inside the loadable
/// file ranges the container marks for their role — executable text inside
/// executable coverage, initialized data inside writable coverage, import
/// data inside any coverage. Containers declaring no loadable ranges, or
/// not parsing as the declared format, leave the leg silent.
#[test]
fn declared_extents_sit_inside_the_container_loadable_roles() {
    // ELF64: `PT_LOAD` `p_flags` ratify the roles — `R+X` over the header
    // bytes and the text extent, `R+W` over exactly the data extent.
    let pair = elf_loadable_pair(&[
        (5, 0, LOADABLE_TEXT_AT + TEXT_LEN as u64),
        (6, LOADABLE_TEXT_AT + TEXT_LEN as u64, DATA_LEN as u64),
    ]);
    pair.0
        .replay_against(&pair.1)
        .expect("an honest ELF loadable map replays");
    // A program-header table carrying no entries declares no loadable
    // claim — the leg stays silent.
    let pair = elf_loadable_pair(&[]);
    pair.0
        .replay_against(&pair.1)
        .expect("a phdr-less ELF declares no loadable coverage");
    // An executable `PT_LOAD` that ends inside the text extent does not
    // cover it.
    let pair = elf_loadable_pair(&[
        (5, 0, LOADABLE_TEXT_AT + TEXT_LEN as u64 - 4),
        (6, LOADABLE_TEXT_AT + TEXT_LEN as u64, DATA_LEN as u64),
    ]);
    let reason = loadable_rejection(&pair);
    assert!(
        reason.contains("executable text extent sits outside every executable loadable range"),
        "truncated PF_X coverage rejected for the wrong reason: {reason}"
    );
    // A `PT_LOAD` over the text extent without PF_X is not executable
    // coverage.
    let pair = elf_loadable_pair(&[
        (4, 0, LOADABLE_TEXT_AT + TEXT_LEN as u64),
        (6, LOADABLE_TEXT_AT + TEXT_LEN as u64, DATA_LEN as u64),
    ]);
    let reason = loadable_rejection(&pair);
    assert!(
        reason.contains("executable text extent sits outside every executable loadable range"),
        "read-only text coverage rejected for the wrong reason: {reason}"
    );
    // An executable-but-not-writable `PT_LOAD` is not data coverage.
    let pair = elf_loadable_pair(&[
        (5, 0, LOADABLE_TEXT_AT + TEXT_LEN as u64),
        (5, LOADABLE_TEXT_AT + TEXT_LEN as u64, DATA_LEN as u64),
    ]);
    let reason = loadable_rejection(&pair);
    assert!(
        reason.contains("initialized data extent sits outside every writable loadable range"),
        "non-writable data coverage rejected for the wrong reason: {reason}"
    );

    // Two extents may not double-cover the same bytes: declare the data
    // extent inside the text extent — both seals stay honest over the
    // committed bytes, so the overlap check is what refuses it.
    let text: [u8; TEXT_LEN] = std::array::from_fn(|index| (index * 7 + 3) as u8);
    let mut executable = elf64_loadable_header(&[
        (5, 0, LOADABLE_TEXT_AT + TEXT_LEN as u64),
        (6, LOADABLE_TEXT_AT + 4, DATA_LEN as u64),
    ]);
    executable.extend_from_slice(&text);
    executable.extend_from_slice(&[0x00u8; 32]);
    let evidence = NativePlacedImageEvidence::from_parts(
        target::NativeTarget::linux_x64(),
        LOADABLE_TEXT_AT,
        placed_inventory(&text),
        LOADABLE_TEXT_AT + 4,
        placed_data_inventory(&text[4..4 + DATA_LEN]),
        0,
        image::PlacedDataRegionInventory::empty(),
    );
    let reason = evidence
        .replay_against(&executable)
        .expect_err("overlapping declared extents must fail custody replay");
    assert!(
        reason.contains("executable text and initialized data extents overlap"),
        "overlapping extents rejected for the wrong reason: {reason}"
    );

    // PE32+: section characteristics ratify the roles —
    // `IMAGE_SCN_MEM_EXECUTE` on `.text`, `IMAGE_SCN_MEM_WRITE` on
    // `.data`.
    const SCN_EXEC_READ: u32 = 0x6000_0020;
    const SCN_WRITE_READ: u32 = 0xc000_0040;
    const SCN_READ_ONLY: u32 = 0x4000_0040;
    let text_end = COFF_TEXT_FILE_OFFSET + TEXT_LEN as u64;
    let data_end = text_end + DATA_LEN as u64;
    let pair = pe_loadable_pair(&[
        (TEXT_LEN as u32, COFF_TEXT_FILE_OFFSET as u32, SCN_EXEC_READ),
        (DATA_LEN as u32, text_end as u32, SCN_WRITE_READ),
    ]);
    pair.0
        .replay_against(&pair.1)
        .expect("an honest PE section table replays");
    // A section table carrying no sections declares no loadable claim.
    let pair = pe_loadable_pair(&[]);
    pair.0
        .replay_against(&pair.1)
        .expect("a sectionless PE declares no loadable coverage");
    // `.text` without `MEM_EXECUTE` is not executable coverage.
    let pair = pe_loadable_pair(&[
        (TEXT_LEN as u32, COFF_TEXT_FILE_OFFSET as u32, SCN_READ_ONLY),
        (DATA_LEN as u32, text_end as u32, SCN_WRITE_READ),
    ]);
    let reason = loadable_rejection(&pair);
    assert!(
        reason.contains("executable text extent sits outside every executable loadable range"),
        "read-only .text rejected for the wrong reason: {reason}"
    );
    // `.data` without `MEM_WRITE` is not writable coverage.
    let pair = pe_loadable_pair(&[
        (TEXT_LEN as u32, COFF_TEXT_FILE_OFFSET as u32, SCN_EXEC_READ),
        (DATA_LEN as u32, text_end as u32, SCN_READ_ONLY),
    ]);
    let reason = loadable_rejection(&pair);
    assert!(
        reason.contains("initialized data extent sits outside every writable loadable range"),
        "read-only .data rejected for the wrong reason: {reason}"
    );
    // A populated import-data extent must sit inside some loadable range
    // (it claims no role of its own): `.rdata` covering it honestly
    // replays; `.rdata` whose `PointerToRawData` misses the extent does
    // not.
    let pair = windows_loadable_thunk_pair(&[
        (18, COFF_TEXT_FILE_OFFSET as u32, SCN_EXEC_READ),
        (16, text_end as u32 - 2, SCN_READ_ONLY),
    ]);
    pair.0
        .replay_against(&pair.1)
        .expect("import data inside a declared .rdata replays");
    let pair = windows_loadable_thunk_pair(&[
        (18, COFF_TEXT_FILE_OFFSET as u32, SCN_EXEC_READ),
        (16, (data_end + 0x200) as u32, SCN_READ_ONLY),
    ]);
    let reason = loadable_rejection(&pair);
    assert!(
        reason.contains("import-data extent sits outside every loadable range"),
        "uncovered import data rejected for the wrong reason: {reason}"
    );

    // Mach-O 64: `LC_SEGMENT_64` `initprot` ratifies the roles.
    let pair = macho_loadable_pair(5, 3); // `__TEXT` R-X, `__DATA` R+W
    pair.0
        .replay_against(&pair.1)
        .expect("an honest Mach-O loadable map replays");
    // `__TEXT` without VM_PROT_EXECUTE is not executable coverage.
    let pair = macho_loadable_pair(1, 3);
    let reason = loadable_rejection(&pair);
    assert!(
        reason.contains("executable text extent sits outside every executable loadable range"),
        "non-executable __TEXT rejected for the wrong reason: {reason}"
    );
    // `__DATA` without VM_PROT_WRITE is not writable coverage.
    let pair = macho_loadable_pair(5, 5);
    let reason = loadable_rejection(&pair);
    assert!(
        reason.contains("initialized data extent sits outside every writable loadable range"),
        "read-only __DATA rejected for the wrong reason: {reason}"
    );

    // The rejection surfaces through the product leg as a named
    // native-inventory rejection, not an opaque custody failure.
    let pair = elf_loadable_pair(&[
        (4, 0, LOADABLE_TEXT_AT + TEXT_LEN as u64),
        (6, LOADABLE_TEXT_AT + TEXT_LEN as u64, DATA_LEN as u64),
    ]);
    let sidecar = native_sidecar(&pair.1, pair.0.to_bytes());
    let outcome =
        verify_native_proof_sidecar(&pair.1, &sidecar.to_bytes(), &offered_policy(&sidecar));
    assert!(
        rejecting_subject(outcome).contains("native executable inventory"),
        "uncovered text must reject on the native inventory"
    );
}
