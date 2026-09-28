//! In-crate replay coverage for `validate_final_text_relocation_envelope`,
//! the format-neutral proof that final `.text` preserves every encoded bit
//! except the exact immediate fields named by checked relocation records.
//! Until now this route was reached only through image-emission's
//! `final_image_validation`; these tests pin the boundary inside the owning
//! crate: relocations applied by the x86-64 and AArch64 patchers replay
//! through the envelope, each relocation kind masks only its declared field
//! bits, an emitter-appended suffix stays outside text custody, malformed
//! or overlapping fields fail closed, and the returned evidence digests
//! bind the exact relocation plan rather than its presentation order.

use image::{
    FinalImageInput, FinalImageLayout, apply_aarch64_relocations, apply_x86_64_relocations,
    build_final_image, validate_final_text_relocation_envelope,
};
use object_file::{
    ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, RelocationPlan,
    RelocationRecord, SectionKind, SectionPlan, SymbolKind, SymbolPlan, SymbolSection,
};
use target::NativeTarget;

const TEXT_ADDRESS: u64 = 0x40_0000;
const DATA_ADDRESS: u64 = 0x41_0000;
const BSS_ADDRESS: u64 = 0x42_0000;

fn layout() -> FinalImageLayout {
    FinalImageLayout {
        text_address: TEXT_ADDRESS,
        data_address: DATA_ADDRESS,
        bss_address: BSS_ADDRESS,
    }
}

fn text_relocation(
    offset: usize,
    byte_width: usize,
    symbol: ObjectSymbolHandle,
    addend: i64,
    kind: RelocationKind,
) -> RelocationRecord {
    RelocationRecord {
        origin: RelocationOrigin::Instruction {
            function_symbol_handle: ObjectSymbolHandle::invalid(),
            selected_instruction_index: 0,
        },
        section: SectionKind::Text,
        offset,
        byte_width,
        symbol_handle: symbol,
        addend,
        kind,
    }
}

fn object_symbol(
    object: &mut ObjectPlan,
    name: &str,
    section: SectionKind,
    offset: usize,
    size: usize,
    kind: SymbolKind,
) -> ObjectSymbolHandle {
    object.layout.symbols.insert(SymbolPlan {
        name: name.into(),
        section: SymbolSection::Section(section),
        offset,
        size,
        kind,
        import_library: String::new(),
    })
}

#[test]
fn declared_fields_may_change_and_the_evidence_replays_deterministically() {
    let encoded = [0xe8, 0, 0, 0, 0, 0xc3, 0x90, 0x90];
    let mut final_text = encoded.to_vec();
    final_text[1..5].copy_from_slice(&3i32.to_le_bytes());
    let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
    relocations.push_record(text_relocation(
        1,
        4,
        ObjectSymbolHandle::invalid(),
        0,
        RelocationKind::X86_64Relative32,
    ));

    let evidence = validate_final_text_relocation_envelope(&encoded, &final_text, &relocations)
        .expect("a relocation writing inside its declared field replays");
    assert_eq!(evidence.text_relocation_count, 1);
    assert_eq!(evidence.checked_instruction_validation_count, 0);
    assert!(
        evidence.has_valid_derivation_digest(),
        "the returned derivation digest commits every reported field",
    );

    let replayed = validate_final_text_relocation_envelope(&encoded, &final_text, &relocations)
        .expect("independent replay succeeds again");
    assert_eq!(
        evidence, replayed,
        "independent replay derives identical evidence",
    );
}

#[test]
fn applied_x86_64_relocations_replay_through_the_text_envelope() {
    let target = NativeTarget::linux_x64();
    let mut object = ObjectPlan::with_capacity(target, 1, 2);
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Text,
        size: 20,
        alignment: 16,
    });
    // call <callee>; ret; pad | callee: ret; pad | absolute64 slot
    let caller = object_symbol(
        &mut object,
        "caller",
        SectionKind::Text,
        0,
        8,
        SymbolKind::Function,
    );
    let callee = object_symbol(
        &mut object,
        "callee",
        SectionKind::Text,
        8,
        4,
        SymbolKind::Function,
    );
    object.layout.entry_symbol = caller;

    let encoded: Vec<u8> = [
        0xe8, 0, 0, 0, 0, // call rel32, field at 1..5
        0xc3, 0x90, 0x90, // ret; pad
        0xc3, 0x90, 0x90, 0x90, // callee at 8
        0, 0, 0, 0, 0, 0, 0, 0, // absolute64 slot at 12..20
    ]
    .into();
    let mut relocations = RelocationPlan::with_target(target);
    relocations.push_record(text_relocation(
        1,
        4,
        callee,
        0,
        RelocationKind::X86_64Relative32,
    ));
    relocations.push_record(text_relocation(
        12,
        8,
        callee,
        7,
        RelocationKind::Absolute64,
    ));

    let mut image = build_final_image(FinalImageInput {
        target,
        object: &object,
        relocations: &relocations,
        text_bytes: &encoded,
        data_bytes: &[],
    });
    apply_x86_64_relocations(&mut image, &layout(), "test image")
        .expect("declared x86-64 relocations apply");

    // The patcher writes exactly what the layout dictates: rel32 measures
    // from the end of its own field, absolute64 carries the full address.
    assert_eq!(
        i32::from_le_bytes(image.memory.text[1..5].try_into().unwrap()),
        3,
        "rel32 = callee(0x400008) - (0x400000 + 1 + 4)",
    );
    assert_eq!(
        u64::from_le_bytes(image.memory.text[12..20].try_into().unwrap()),
        0x40_000f,
        "absolute64 = callee(0x400008) + addend 7",
    );

    let evidence =
        validate_final_text_relocation_envelope(&encoded, &image.memory.text, &relocations)
            .expect("applied relocations replay through the envelope");
    assert_eq!(evidence.text_relocation_count, 2);
    assert!(evidence.has_valid_derivation_digest());

    // The same patched bytes do not replay against a plan that forgot its
    // records: every changed byte is then unexplained mutation.
    let forgotten = RelocationPlan::with_target(target);
    assert!(
        validate_final_text_relocation_envelope(&encoded, &image.memory.text, &forgotten).is_err(),
        "patched fields require their declared relocation records",
    );

    // A substituted encoded input fails closed even where the patch matched:
    // byte 0 sits outside every declared field.
    let mut substituted = encoded.clone();
    substituted[0] = 0xe9;
    assert!(
        validate_final_text_relocation_envelope(&substituted, &image.memory.text, &relocations)
            .is_err(),
        "a substituted encoded text is not covered by the same envelope",
    );

    // A foreign relocation kind cannot launder through the x86-64 patcher.
    let mut foreign = RelocationPlan::with_target(target);
    foreign.push_record(text_relocation(
        0,
        4,
        callee,
        0,
        RelocationKind::Aarch64Branch26,
    ));
    let mut foreign_image = build_final_image(FinalImageInput {
        target,
        object: &object,
        relocations: &foreign,
        text_bytes: &encoded,
        data_bytes: &[],
    });
    assert!(
        apply_x86_64_relocations(&mut foreign_image, &layout(), "test image").is_err(),
        "the x86-64 patcher refuses an AArch64 relocation kind",
    );
}

#[test]
fn applied_aarch64_relocations_replay_through_the_text_envelope() {
    let target = NativeTarget::linux_arm64();
    let mut object = ObjectPlan::with_capacity(target, 2, 2);
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Text,
        size: 16,
        alignment: 4,
    });
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Data,
        size: 0x40,
        alignment: 8,
    });
    let entry = object_symbol(
        &mut object,
        "entry",
        SectionKind::Text,
        0,
        16,
        SymbolKind::Function,
    );
    let callee = object_symbol(
        &mut object,
        "callee",
        SectionKind::Text,
        12,
        4,
        SymbolKind::Function,
    );
    let data_cell = object_symbol(
        &mut object,
        "cell",
        SectionKind::Data,
        0x20,
        8,
        SymbolKind::Object,
    );
    object.layout.entry_symbol = entry;

    let encoded: Vec<u8> = [
        0x00, 0x00, 0x00, 0x90, // adrp x0, #0 — Page21 field at 0..4
        0x00, 0x00, 0x00, 0x91, // add x0, x0, #0 — PageOffset12 field at 4..8
        0x00, 0x00, 0x00, 0x14, // b #0 — Branch26 field at 8..12
        0xc0, 0x03, 0x5f, 0xd6, // callee at 12: ret
    ]
    .into();
    let mut relocations = RelocationPlan::with_target(target);
    relocations.push_record(text_relocation(
        0,
        4,
        data_cell,
        0,
        RelocationKind::Aarch64Page21,
    ));
    relocations.push_record(text_relocation(
        4,
        4,
        data_cell,
        0,
        RelocationKind::Aarch64PageOffset12,
    ));
    relocations.push_record(text_relocation(
        8,
        4,
        callee,
        0,
        RelocationKind::Aarch64Branch26,
    ));

    let mut image = build_final_image(FinalImageInput {
        target,
        object: &object,
        relocations: &relocations,
        text_bytes: &encoded,
        data_bytes: &[0; 0x40],
    });
    apply_aarch64_relocations(&mut image, &layout(), "test image")
        .expect("declared AArch64 relocations apply");

    // adrp x0, cell@page(0x410000) from insn page 0x400000: delta 16 pages,
    // immlo = 0, immhi = 4.
    assert_eq!(
        u32::from_le_bytes(image.memory.text[0..4].try_into().unwrap()),
        0x9000_0080,
    );
    // add x0, x0, cell@pageoff(0x20): immediate 0x20 into bits 10..22.
    assert_eq!(
        u32::from_le_bytes(image.memory.text[4..8].try_into().unwrap()),
        0x9100_8000,
    );
    // b callee(0x40000c) from 0x400008: one instruction forward.
    assert_eq!(
        u32::from_le_bytes(image.memory.text[8..12].try_into().unwrap()),
        0x1400_0001,
    );

    let evidence =
        validate_final_text_relocation_envelope(&encoded, &image.memory.text, &relocations)
            .expect("applied relocations replay through the envelope");
    assert_eq!(evidence.text_relocation_count, 3);
    assert!(evidence.has_valid_derivation_digest());

    // The ADRP mask does not cover the opcode bits: a patched text whose
    // low bit drifted outside the declared field fails closed.
    let mut drifted = image.memory.text.clone();
    drifted[0] ^= 0x01;
    assert!(
        validate_final_text_relocation_envelope(&encoded, &drifted, &relocations).is_err(),
        "bits outside a declared field remain pinned",
    );
}

#[test]
fn each_relocation_kind_masks_exactly_its_declared_field_bits() {
    // The envelope carries the patch masks a second time as little-endian
    // byte arrays; this table pins that second copy against the fields the
    // ISA actually owns.
    for (kind, width, mask) in [
        (RelocationKind::X86_64Relative32, 4, &[0xff; 4][..]),
        (RelocationKind::Absolute64, 8, &[0xff; 8][..]),
        (
            RelocationKind::Aarch64Page21,
            4,
            &[0xe0, 0xff, 0xff, 0x60][..],
        ),
        (
            RelocationKind::Aarch64PageOffset12,
            4,
            &[0x00, 0xfc, 0x3f, 0x00][..],
        ),
        (
            RelocationKind::Aarch64Branch26,
            4,
            &[0xff, 0xff, 0xff, 0x03][..],
        ),
    ] {
        let encoded = vec![0u8; width + 8];
        let mut relocations = RelocationPlan::with_target(NativeTarget::host());
        relocations.push_record(text_relocation(
            4,
            width,
            ObjectSymbolHandle::invalid(),
            0,
            kind,
        ));

        // Every mutable bit may change at once and the field still replays.
        let mut inside = encoded.clone();
        for (index, mutable) in mask.iter().enumerate() {
            inside[4 + index] |= *mutable;
        }
        validate_final_text_relocation_envelope(&encoded, &inside, &relocations).unwrap_or_else(
            |_| panic!("{kind:?} must accept changes inside its declared field bits"),
        );

        // A single fixed bit inside the field stays pinned.
        if let Some((index, mutable)) = mask.iter().enumerate().find(|(_, mask)| **mask != 0xff) {
            let mut fixed = encoded.clone();
            fixed[4 + index] = !*mutable;
            assert!(
                validate_final_text_relocation_envelope(&encoded, &fixed, &relocations).is_err(),
                "{kind:?} must reject changes to fixed field bits",
            );
        }

        // Bytes before and after the field stay pinned.
        for slot in [0, width + 7] {
            let mut outside = encoded.clone();
            outside[slot] = 0x01;
            assert!(
                validate_final_text_relocation_envelope(&encoded, &outside, &relocations).is_err(),
                "{kind:?} must reject changes outside its declared field",
            );
        }
    }
}

#[test]
fn malformed_relocation_fields_fail_closed() {
    let encoded = vec![0u8; 16];

    // Final text shorter than the encoded input loses compiler code.
    let mut relocations = RelocationPlan::with_target(NativeTarget::linux_x64());
    relocations.push_record(text_relocation(
        1,
        4,
        ObjectSymbolHandle::invalid(),
        0,
        RelocationKind::X86_64Relative32,
    ));
    assert!(
        validate_final_text_relocation_envelope(&encoded, &encoded[..8], &relocations).is_err(),
        "truncated final text is rejected",
    );

    // A record's declared width must match the field its kind owns.
    let mut wrong_width = RelocationPlan::with_target(NativeTarget::linux_x64());
    wrong_width.push_record(text_relocation(
        1,
        8,
        ObjectSymbolHandle::invalid(),
        0,
        RelocationKind::X86_64Relative32,
    ));
    assert!(
        validate_final_text_relocation_envelope(&encoded, &encoded, &wrong_width).is_err(),
        "a widened field is rejected",
    );

    // Two declared fields may not overlap.
    let mut overlapping = RelocationPlan::with_target(NativeTarget::linux_x64());
    overlapping.push_record(text_relocation(
        1,
        4,
        ObjectSymbolHandle::invalid(),
        0,
        RelocationKind::X86_64Relative32,
    ));
    overlapping.push_record(text_relocation(
        4,
        8,
        ObjectSymbolHandle::invalid(),
        0,
        RelocationKind::Absolute64,
    ));
    assert!(
        validate_final_text_relocation_envelope(&encoded, &encoded, &overlapping).is_err(),
        "overlapping declared fields are rejected",
    );

    // A field may not extend beyond the encoded text it claims to patch.
    let mut out_of_bounds = RelocationPlan::with_target(NativeTarget::linux_x64());
    out_of_bounds.push_record(text_relocation(
        12,
        8,
        ObjectSymbolHandle::invalid(),
        0,
        RelocationKind::Absolute64,
    ));
    assert!(
        validate_final_text_relocation_envelope(&encoded, &encoded, &out_of_bounds).is_err(),
        "a field past the encoded text is rejected",
    );

    // Non-text records are not text fields: even a malformed data record is
    // this envelope's sibling's concern, not a constraint on `.text`.
    let mut data_only = RelocationPlan::with_target(NativeTarget::linux_x64());
    let mut data_record = text_relocation(
        usize::MAX - 3,
        4,
        ObjectSymbolHandle::invalid(),
        0,
        RelocationKind::X86_64Relative32,
    );
    data_record.section = SectionKind::Data;
    data_only.push_record(data_record);
    validate_final_text_relocation_envelope(&encoded, &encoded, &data_only)
        .expect("non-text relocation records do not constrain the text envelope");
}

#[test]
fn emitter_appended_suffix_stays_outside_text_custody() {
    // PE and Mach-O emitters append import thunks after the compiler's own
    // code, so a longer final text is admitted and only the encoded-length
    // prefix is validated or committed.
    let encoded = vec![0xcc; 8];
    let relocations = RelocationPlan::with_target(NativeTarget::windows_x64());
    let mut suffixed = encoded.clone();
    suffixed.extend_from_slice(&[0xde, 0xad, 0xbe, 0xef]);

    let unpadded = validate_final_text_relocation_envelope(&encoded, &encoded, &relocations)
        .expect("the exact final text replays");
    let padded = validate_final_text_relocation_envelope(&encoded, &suffixed, &relocations)
        .expect("an emitter-appended suffix is admitted");
    assert_eq!(
        padded, unpadded,
        "the appended suffix enters no custody commitment",
    );
}

#[test]
fn envelope_digest_binds_the_exact_plan_not_its_presentation_order() {
    let encoded = vec![0u8; 24];
    let unchanged = encoded.clone();

    let mut base = RelocationPlan::with_target(NativeTarget::linux_x64());
    base.push_record(text_relocation(
        0,
        4,
        ObjectSymbolHandle::invalid(),
        0,
        RelocationKind::X86_64Relative32,
    ));
    let base_evidence = validate_final_text_relocation_envelope(&encoded, &unchanged, &base)
        .expect("base plan replays");

    // Kind, offset and addend are each committed: changing any one produces
    // a different envelope, so substituted plans cannot share custody.
    for drifted_record in [
        text_relocation(
            0,
            8,
            ObjectSymbolHandle::invalid(),
            0,
            RelocationKind::Absolute64,
        ),
        text_relocation(
            4,
            4,
            ObjectSymbolHandle::invalid(),
            0,
            RelocationKind::X86_64Relative32,
        ),
        text_relocation(
            0,
            4,
            ObjectSymbolHandle::invalid(),
            8,
            RelocationKind::X86_64Relative32,
        ),
    ] {
        let mut drifted_plan = RelocationPlan::with_target(NativeTarget::linux_x64());
        drifted_plan.push_record(drifted_record);
        let drifted_evidence =
            validate_final_text_relocation_envelope(&encoded, &unchanged, &drifted_plan)
                .expect("a different declared field still replays");
        assert_ne!(
            drifted_evidence.relocation_envelope_digest, base_evidence.relocation_envelope_digest,
            "the envelope digest binds kind, offset and addend",
        );
        assert_ne!(
            drifted_evidence.derivation_digest, base_evidence.derivation_digest,
            "the derivation digest cannot be reused across plans",
        );
    }

    // Record insertion order is presentation, not custody: the canonical
    // ordering makes the two plans interchangeable.
    let mut reordered = RelocationPlan::with_target(NativeTarget::linux_x64());
    reordered.push_record(text_relocation(
        8,
        8,
        ObjectSymbolHandle::invalid(),
        0,
        RelocationKind::Absolute64,
    ));
    reordered.push_record(text_relocation(
        0,
        4,
        ObjectSymbolHandle::invalid(),
        0,
        RelocationKind::X86_64Relative32,
    ));
    let reordered_evidence =
        validate_final_text_relocation_envelope(&encoded, &unchanged, &reordered)
            .expect("the reordered plan replays");

    let mut ordered = RelocationPlan::with_target(NativeTarget::linux_x64());
    ordered.push_record(text_relocation(
        0,
        4,
        ObjectSymbolHandle::invalid(),
        0,
        RelocationKind::X86_64Relative32,
    ));
    ordered.push_record(text_relocation(
        8,
        8,
        ObjectSymbolHandle::invalid(),
        0,
        RelocationKind::Absolute64,
    ));
    let ordered_evidence = validate_final_text_relocation_envelope(&encoded, &unchanged, &ordered)
        .expect("the ordered plan replays");
    assert_eq!(
        reordered_evidence.relocation_envelope_digest, ordered_evidence.relocation_envelope_digest,
        "the canonical envelope is independent of record insertion order",
    );

    // The encoded digest commits the encoded input even where no field ever
    // declared it: identical byte equality cannot launder a different source.
    let mut substituted = encoded.clone();
    substituted[20] = 0x5a;
    let substituted_evidence =
        validate_final_text_relocation_envelope(&substituted, &substituted, &base)
            .expect("an unchanged substitution still replays byte equality");
    assert_ne!(
        substituted_evidence.encoded_text_digest, base_evidence.encoded_text_digest,
        "the encoded digest distinguishes the substituted input",
    );
}
