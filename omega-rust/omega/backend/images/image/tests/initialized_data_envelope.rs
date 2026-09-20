//! In-crate replay coverage for the emitted-output initialized-data custody
//! boundary. `EmittedImageOutput::validate_final_initialized_data_relocation_envelope`
//! is the public route that proves final `.data` preserves every encoded byte
//! except the exact eight-byte slots named by checked Absolute64 data
//! relocations; until now it was reached only through image-emission's
//! `final_image_validation`, and the compact-versus-strong evidence property
//! named in the crate header (`compact_collision_cannot_substitute_strong_
//! compiler_text_evidence`) lived only in the `output.rs` unit tests.
//!
//! These tests pin the boundary inside the owning crate: a real relocation
//! plan is applied by the x86-64 patcher, the placed executable/data
//! inventories and the wrapped `EmittedImageOutput` replay the result, a byte
//! changed outside a declared slot fails closed, malformed or overlapping
//! data-slot declarations fail closed, text records stay out of data custody,
//! and a substituted strong digest cannot hide behind an identical compact
//! report fingerprint. The writer-owned import-data extent — PE's `.rdata`
//! IAT slots outside `.data` — replays through the same byte join on the
//! `final_import_data_bytes`/`import_data_regions` pair.

use image::{
    EmittedImageOutput, EncodedCompilerTextDigest, ExecutableImageOutput, FinalDataRegion,
    FinalDataRegionOrigin, FinalImageInput, FinalImageLayout, ImageOutputKind,
    apply_x86_64_relocations, build_final_image, emitted_direct_executable_output,
    place_data_extent, place_data_regions, place_executable_regions,
    validate_final_text_relocation_envelope, validate_placed_data_region_inventory,
    validate_placed_executable_region_inventory,
};
use object_file::{
    ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationPlan,
    RelocationRecord, SectionKind, SectionPlan, SymbolKind, SymbolPlan, SymbolSection,
};
use target::NativeTarget;

const TEXT_ADDRESS: u64 = 0x40_0000;
const DATA_ADDRESS: u64 = 0x41_0000;
const BSS_ADDRESS: u64 = 0x42_0000;

/// mov eax, edi; test eax, eax; setnz al; ret — eight bytes the entry symbol
/// owns completely so the placed executable inventory classifies every byte.
const TEXT_BYTES: [u8; 8] = [0x89, 0xf8, 0x85, 0xc0, 0x0f, 0x95, 0xc0, 0xc3];

/// A literal prefix, an eight-byte Absolute64 slot at 8..16, then a literal
/// tail: the envelope must explain every changed byte by the declared slot.
fn encoded_data() -> Vec<u8> {
    let mut data = vec![0x5a; 8];
    data.extend_from_slice(&[0; 8]);
    data.extend_from_slice(&[0x33; 8]);
    data
}

fn layout() -> FinalImageLayout {
    FinalImageLayout {
        text_address: TEXT_ADDRESS,
        data_address: DATA_ADDRESS,
        bss_address: BSS_ADDRESS,
    }
}

fn data_relocation(
    offset: usize,
    byte_width: usize,
    symbol: ObjectSymbolHandle,
    addend: i64,
    kind: RelocationKind,
) -> RelocationRecord {
    RelocationRecord {
        origin: RelocationOrigin::Materialization {
            object_symbol_handle: ObjectSymbolHandle::invalid(),
        },
        section: SectionKind::Data,
        offset,
        byte_width,
        symbol_handle: symbol,
        addend,
        kind,
    }
}

/// An object carrying one text function and one data cell. The entry symbol
/// covers the complete `.text` so the executable inventory has no
/// unclassified gap, and the non-empty `.data` image produces exactly one
/// compiler-data row.
fn object_plan(target: NativeTarget) -> (ObjectPlan, ObjectSymbolHandle) {
    let mut object = ObjectPlan::with_capacity(target, 2, 2);
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Text,
        size: TEXT_BYTES.len(),
        alignment: 16,
    });
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Data,
        size: 24,
        alignment: 8,
    });
    let entry = object.layout.symbols.insert(SymbolPlan {
        name: "entry".into(),
        section: SymbolSection::Section(SectionKind::Text),
        offset: 0,
        size: TEXT_BYTES.len(),
        kind: SymbolKind::Function,
        import_library: String::new(),
    });
    object.layout.entry_symbol = entry;
    object.layout.symbols.insert(SymbolPlan {
        name: "cell".into(),
        section: SymbolSection::Section(SectionKind::Data),
        offset: 0,
        size: 8,
        kind: SymbolKind::Object,
        import_library: String::new(),
    });
    (object, entry)
}

/// The real emitted-output route: encoded bytes in, declared data slot
/// patched in place, both placed inventories resolved, and the executable
/// output wrapped into the emitted custody carrier.
fn emitted_output(
    target: NativeTarget,
) -> (
    EmittedImageOutput,
    Vec<u8>,
    RelocationPlan,
    ObjectSymbolHandle,
) {
    let (object, entry) = object_plan(target);
    let mut relocations = RelocationPlan::with_target(target);
    // data[8..16] = address of `entry` + 7 once patched.
    relocations.push_record(data_relocation(8, 8, entry, 7, RelocationKind::Absolute64));
    let encoded = encoded_data();
    let mut image = build_final_image(FinalImageInput {
        target,
        object: &object,
        relocations: &relocations,
        text_bytes: &TEXT_BYTES,
        data_bytes: &encoded,
    });
    apply_x86_64_relocations(&mut image, &layout(), "test image")
        .expect("the declared data relocation applies");
    assert_eq!(
        u64::from_le_bytes(image.memory.data[8..16].try_into().unwrap()),
        TEXT_ADDRESS + 7,
        "absolute64 = entry(0x400000) + addend 7",
    );
    let executable_regions =
        place_executable_regions(&image, layout()).expect("executable regions place");
    let data_regions = place_data_regions(&image, layout()).expect("data regions place");
    let output = emitted_direct_executable_output(ExecutableImageOutput {
        bytes: b"container".to_vec(),
        final_image_layout: layout(),
        final_text_bytes: image.memory.text.clone(),
        final_data_bytes: image.memory.data.clone(),
        final_import_data_bytes: Vec::new(),
        file_name: "omega-program".into(),
        format: "elf".into(),
        text_bytes: image.memory.text.len(),
        data_bytes: image.memory.data.len(),
        bss_bytes: image.memory.bss_size,
        symbols: image.symbol_table.symbols.len(),
        imports: image.symbol_table.imports.len(),
        relocations: image.relocation_table.relocations.len(),
        executable_regions,
        data_regions,
        import_data_regions: image::PlacedDataRegionInventory::empty(),
    });
    (output, encoded, relocations, entry)
}

#[test]
fn applied_data_relocations_replay_through_the_emitted_output_envelope() {
    let (output, encoded, relocations, _entry) = emitted_output(NativeTarget::linux_x64());
    assert_eq!(output.kind, ImageOutputKind::DirectExecutable);

    output
        .validate_final_initialized_data_relocation_envelope(&encoded, &relocations)
        .expect("the patched data replays against its declared slot");
    output
        .validate_final_initialized_data_relocation_envelope(&encoded, &relocations)
        .expect("independent replay succeeds again");

    // Every byte of both sections classified, and the placed inventories
    // replay over the exact final bytes they claim to describe.
    assert!(output.executable_regions.unclassified_gaps.is_empty());
    assert!(output.data_regions.unclassified_gaps.is_empty());
    validate_placed_executable_region_inventory(
        &output.executable_regions,
        &output.final_text_bytes,
    )
    .expect("the placed executable inventory replays over final text");
    validate_placed_data_region_inventory(&output.data_regions, &output.final_data_bytes)
        .expect("the placed data inventory replays over final data");

    // The data inventory commits the PATCHED image: replaying it against the
    // pre-relocation encoding rejects, because the slot bytes differ.
    assert!(
        validate_placed_data_region_inventory(&output.data_regions, &encoded).is_err(),
        "the placed data inventory binds the relocated bytes, not the encoded input",
    );
}

#[test]
fn emitted_output_data_envelope_rejects_mutations_outside_declared_slots() {
    let target = NativeTarget::linux_x64();
    let (output, encoded, relocations, _entry) = emitted_output(target);

    // Bytes inside the declared slot are exactly the mutable field: an
    // emitter-side byte there is admitted by the envelope.
    let mut inside = output.clone();
    inside.final_data_bytes[9] ^= 0xff;
    inside
        .validate_final_initialized_data_relocation_envelope(&encoded, &relocations)
        .expect("a byte inside the declared slot may differ");

    // A byte outside the slot is unexplained mutation.
    let mut outside = output.clone();
    outside.final_data_bytes[0] ^= 1;
    let error = outside
        .validate_final_initialized_data_relocation_envelope(&encoded, &relocations)
        .expect_err("a byte outside the declared slot must be rejected");
    assert!(
        error
            .message
            .contains("changed outside a declared Absolute64 relocation slot"),
        "unexpected diagnostic: {}",
        error.message,
    );

    // The data image is exactly sized: neither truncation nor an appended
    // byte may stand in for it.
    let mut truncated = output.clone();
    truncated.final_data_bytes.pop();
    assert!(
        truncated
            .validate_final_initialized_data_relocation_envelope(&encoded, &relocations)
            .is_err(),
        "a truncated data image must be rejected",
    );
    let mut extended = output.clone();
    extended.final_data_bytes.push(0);
    assert!(
        extended
            .validate_final_initialized_data_relocation_envelope(&encoded, &relocations)
            .is_err(),
        "an extended data image must be rejected",
    );
}

#[test]
fn malformed_data_relocation_plans_fail_closed_at_the_output() {
    let target = NativeTarget::linux_x64();
    let (output, encoded, _relocations, entry) = emitted_output(target);
    let slot = |offset, width, kind| {
        let mut plan = RelocationPlan::with_target(target);
        plan.push_record(data_relocation(offset, width, entry, 0, kind));
        plan
    };

    // Only Absolute64 width-8 slots may mutate initialized data: every other
    // shape is rejected before any byte comparison runs.
    for (label, plan) in [
        (
            "a relative-kind slot",
            slot(8, 4, RelocationKind::X86_64Relative32),
        ),
        ("a narrow slot", slot(8, 4, RelocationKind::Absolute64)),
        ("a misaligned slot", slot(4, 8, RelocationKind::Absolute64)),
        (
            "an out-of-bounds slot",
            slot(24, 8, RelocationKind::Absolute64),
        ),
    ] {
        let error = output
            .validate_final_initialized_data_relocation_envelope(&encoded, &plan)
            .expect_err("a malformed declared slot must be rejected");
        assert!(
            !error.message.is_empty(),
            "{label}: rejection carries a diagnostic",
        );
    }

    // An offset whose slot end cannot be represented fails closed rather than
    // wrapping into a different byte range.
    let mut overflowing = RelocationPlan::with_target(target);
    overflowing.push_record(data_relocation(
        usize::MAX - 7,
        8,
        entry,
        0,
        RelocationKind::Absolute64,
    ));
    assert!(
        output
            .validate_final_initialized_data_relocation_envelope(&encoded, &overflowing)
            .is_err(),
        "an unrepresentable slot end must be rejected",
    );

    // Two declared slots may not overlap.
    let mut overlapping = RelocationPlan::with_target(target);
    overlapping.push_record(data_relocation(8, 8, entry, 0, RelocationKind::Absolute64));
    overlapping.push_record(data_relocation(8, 8, entry, 0, RelocationKind::Absolute64));
    assert!(
        output
            .validate_final_initialized_data_relocation_envelope(&encoded, &overlapping)
            .is_err(),
        "overlapping declared slots must be rejected",
    );

    // The envelope is deliberately asymmetric: a text record is the text
    // envelope's concern and imposes no data custody, even a malformed one —
    // it neither declares a slot nor trips the malformed-slot checks. With an
    // unchanged data image the record is simply invisible here.
    let mut text_only = RelocationPlan::with_target(target);
    text_only.push_record(RelocationRecord {
        section: SectionKind::Text,
        ..data_relocation(
            usize::MAX - 3,
            4,
            entry,
            0,
            RelocationKind::X86_64Relative32,
        )
    });
    output
        .validate_final_initialized_data_relocation_envelope(&output.final_data_bytes, &text_only)
        .expect("text relocation records do not constrain the data envelope");

    // An empty plan explains no mutation: the encoded bytes must be carried
    // through unchanged to replay.
    let empty = RelocationPlan::with_target(target);
    assert!(
        output
            .validate_final_initialized_data_relocation_envelope(&encoded, &empty)
            .is_err(),
        "the patched slot is unexplained mutation under an empty plan",
    );
}

#[test]
fn placed_inventories_reject_substituted_rows_and_bytes() {
    let (output, _encoded, _relocations, _entry) = emitted_output(NativeTarget::linux_x64());

    // A substituted placement row is not the placed region the final bytes
    // produced: the recomputed address and digests do not follow it.
    let mut moved = output.clone();
    moved.data_regions.regions[0].address += 8;
    assert!(
        validate_placed_data_region_inventory(&moved.data_regions, &moved.final_data_bytes)
            .is_err(),
        "a moved data region must not replay",
    );

    let mut resized = output.clone();
    resized.data_regions.regions[0].byte_count -= 8;
    assert!(
        validate_placed_data_region_inventory(&resized.data_regions, &resized.final_data_bytes)
            .is_err(),
        "a shrunken region leaves an unexplained tail gap",
    );

    // Byte drift in the final image invalidates every retained digest lane:
    // the data digest, the region byte digest, and the inventory join.
    let mut drifted = output.clone();
    drifted.final_data_bytes[0] ^= 1;
    assert!(
        validate_placed_data_region_inventory(&drifted.data_regions, &drifted.final_data_bytes)
            .is_err(),
        "a mutated final byte must not replay against the stored inventory",
    );
    let mut drifted_text = output.clone();
    drifted_text.final_text_bytes[0] ^= 1;
    assert!(
        validate_placed_executable_region_inventory(
            &drifted_text.executable_regions,
            &drifted_text.final_text_bytes,
        )
        .is_err(),
        "a mutated text byte must not replay against the stored inventory",
    );
}

#[test]
fn compact_fingerprint_cannot_substitute_strong_text_evidence_at_the_output() {
    let target = NativeTarget::linux_x64();
    let (mut output, _encoded, relocations, _entry) = emitted_output(target);

    // The emitted output retains the compiler-text validation derived by the
    // public envelope over the exact encoded/final text pair.
    let evidence = validate_final_text_relocation_envelope(
        &TEXT_BYTES,
        &output.final_text_bytes,
        &relocations,
    )
    .expect("the emitted final text replays against its plan");
    assert!(evidence.has_valid_derivation_digest());
    output.compiler_text_validation = Some(evidence);

    // Independent replay derives identical evidence.
    let replayed = validate_final_text_relocation_envelope(
        &TEXT_BYTES,
        &output.final_text_bytes,
        &relocations,
    )
    .expect("independent replay succeeds again");
    assert_eq!(replayed, evidence);

    // A foreign encoded text produces a different strong digest under the
    // same domain: substitute it while keeping every compact report field.
    let foreign = validate_final_text_relocation_envelope(
        &[0xc3],
        &[0xc3],
        &RelocationPlan::with_target(target),
    )
    .expect("a foreign encoded text replays");
    assert_ne!(foreign.encoded_text_digest, evidence.encoded_text_digest);

    let mut substituted = evidence;
    substituted.encoded_text_digest = foreign.encoded_text_digest;
    // The compact lane cannot separate the substitution: the report
    // fingerprints are unchanged because they were never commitments to the
    // digest fields.
    assert_eq!(
        substituted.encoded_text_report_fingerprint,
        evidence.encoded_text_report_fingerprint,
    );
    assert_eq!(
        substituted.derivation_report_fingerprint,
        evidence.derivation_report_fingerprint,
    );
    // The strong lane catches it twice: the retained derivation no longer
    // covers the substituted digest, and even a freshly recomputed derivation
    // keeps the evidence distinct from the replayed original.
    assert!(!substituted.has_valid_derivation_digest());
    substituted.derivation_digest = substituted.recomputed_derivation_digest();
    assert!(substituted.has_valid_derivation_digest());
    assert_ne!(substituted, evidence);
    assert_ne!(substituted.derivation_digest, evidence.derivation_digest);
    assert_ne!(
        output.compiler_text_validation,
        Some(substituted),
        "the emitted output does not retain substituted evidence",
    );

    // A bare substituted digest value that names nothing replayable rejects
    // the same way — the compact lane still cannot authorize it.
    let mut forged = evidence;
    forged.encoded_text_digest = EncodedCompilerTextDigest::from_digest([0xa5; 32]);
    forged.derivation_digest = forged.recomputed_derivation_digest();
    assert!(forged.has_valid_derivation_digest());
    assert_ne!(forged, evidence);
}

#[test]
fn emitted_output_replays_import_slot_custody_over_the_import_data_extent() {
    const RDATA_ADDRESS: u64 = 0x1400_0300;
    let (mut output, _encoded, _relocations, _entry) = emitted_output(NativeTarget::linux_x64());

    // [.rdata]: a 20-byte descriptor/lookup-table head, two 8-byte IAT slots,
    // then a 4-byte name tail — the custody shape `image-pe` places.
    let rdata: Vec<u8> = (0usize..40).map(|index| (index * 11 + 5) as u8).collect();
    let slots = vec![
        FinalDataRegion {
            origin: FinalDataRegionOrigin::ImportBindingSlot,
            section_offset: 20,
            byte_count: 8,
            symbol: "KERNEL32.dll!ExitProcess".into(),
        },
        FinalDataRegion {
            origin: FinalDataRegionOrigin::ImportBindingSlot,
            section_offset: 28,
            byte_count: 8,
            symbol: "KERNEL32.dll!CreateFileW".into(),
        },
    ];
    let import_inventory = place_data_extent(&rdata, slots, RDATA_ADDRESS, "import-data")
        .expect("the import extent places");
    // The bytes custody does not classify stay gaps; only the slots are rows.
    assert_eq!(import_inventory.unclassified_gaps.len(), 2);
    output.final_import_data_bytes = rdata.clone();
    output.import_data_regions = import_inventory;

    // The IAT's custody is in the data inventory: the carried inventory
    // replays over the emitted extent bytes.
    validate_placed_data_region_inventory(
        &output.import_data_regions,
        &output.final_import_data_bytes,
    )
    .expect("the import-slot inventory replays over the emitted extent");

    // Every one-sided join fails closed: a populated inventory claiming an
    // absent extent, and emitted extent bytes under the canonical empty
    // inventory, both reject.
    let mut without_extent = output.clone();
    without_extent.final_import_data_bytes = Vec::new();
    assert!(
        validate_placed_data_region_inventory(
            &without_extent.import_data_regions,
            &without_extent.final_import_data_bytes,
        )
        .is_err(),
        "a populated inventory over an absent extent must reject",
    );
    let mut without_inventory = output.clone();
    without_inventory.import_data_regions = image::PlacedDataRegionInventory::empty();
    assert!(
        validate_placed_data_region_inventory(
            &without_inventory.import_data_regions,
            &without_inventory.final_import_data_bytes,
        )
        .is_err(),
        "emitted extent bytes under the empty inventory must reject",
    );

    // A mutated slot byte or a shifted slot row breaks the byte join the
    // thunk-slot pairing verifier stands on.
    let mut drifted = output.clone();
    drifted.final_import_data_bytes[20] ^= 0xff;
    assert!(
        validate_placed_data_region_inventory(
            &drifted.import_data_regions,
            &drifted.final_import_data_bytes,
        )
        .is_err(),
        "a mutated IAT byte must not replay against the stored inventory",
    );
    let mut moved = output.clone();
    moved.import_data_regions.regions[0].address += 8;
    assert!(
        validate_placed_data_region_inventory(
            &moved.import_data_regions,
            &moved.final_import_data_bytes,
        )
        .is_err(),
        "a moved binding-slot row must not replay",
    );
}
