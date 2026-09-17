//! In-crate replay coverage for the fail-closed arms of the two native
//! relocation patchers. The sibling target `text_relocation_envelope` pins
//! the applied path - declared fields patched, evidence replayed. This
//! target pins the rejections a malformed or hostile relocation plan must
//! produce: every failure surfaces as a `Diagnostic` rather than a panic
//! or a silent patch, and a rejected single-record plan leaves the encoded
//! section bytes bit-identical, so the same text envelope still replays
//! them as unchanged.

use image::{
    FinalImage, FinalImageInput, FinalImageLayout, FinalImageRelocation, FinalImageSection,
    FinalImageSymbol, apply_aarch64_relocations, apply_x86_64_relocations, build_final_image,
    validate_final_text_relocation_envelope,
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

fn record(
    section: SectionKind,
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
        section,
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
    section: SymbolSection,
    offset: usize,
    size: usize,
    kind: SymbolKind,
) -> ObjectSymbolHandle {
    object.layout.symbols.insert(SymbolPlan {
        name: name.into(),
        section,
        offset,
        size,
        kind,
        import_library: String::new(),
    })
}

fn object_with_text(target: NativeTarget, text_size: usize) -> ObjectPlan {
    let mut object = ObjectPlan::with_capacity(target, 4, 1);
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Text,
        size: text_size,
        alignment: 16,
    });
    object
}

fn image_with(
    target: NativeTarget,
    object: &ObjectPlan,
    relocations: &RelocationPlan,
    text: &[u8],
    data: &[u8],
) -> FinalImage {
    build_final_image(FinalImageInput {
        target,
        object,
        relocations,
        text_bytes: text,
        data_bytes: data,
    })
}

#[test]
fn x86_64_rel32_rejects_symbols_without_a_materialized_address() {
    let target = NativeTarget::linux_x64();
    // call <rel32>; ret; pad
    let encoded: Vec<u8> = [0xe8, 0, 0, 0, 0, 0xc3, 0x90, 0x90].into();
    let mut object = object_with_text(target, encoded.len());
    let entry = object_symbol(
        &mut object,
        "entry",
        SymbolSection::Section(SectionKind::Text),
        0,
        8,
        SymbolKind::Function,
    );
    // A declared extern that was never bound: the symbol row exists, but
    // with no section there is no address to subtract the field end from.
    let external = object_symbol(
        &mut object,
        "external_callee",
        SymbolSection::None,
        0,
        0,
        SymbolKind::Function,
    );
    object.layout.entry_symbol = entry;

    let mut relocations = RelocationPlan::with_target(target);
    relocations.push_record(record(
        SectionKind::Text,
        1,
        4,
        external,
        0,
        RelocationKind::X86_64Relative32,
    ));
    let mut image = image_with(target, &object, &relocations, &encoded, &[]);

    let diagnostic = apply_x86_64_relocations(&mut image, &layout(), "test image")
        .expect_err("a sectionless symbol has no address to relocate to");
    assert!(
        diagnostic
            .message
            .contains("test image relocation references unknown symbol `external_callee`"),
        "unexpected diagnostic: {}",
        diagnostic.message,
    );
    assert_eq!(
        image.memory.text, encoded,
        "the rejected plan must not patch the encoded text",
    );
    validate_final_text_relocation_envelope(&encoded, &image.memory.text, &relocations)
        .expect("the untouched text still replays against the rejected plan");

    // A record that never named a symbol reaches the same arm with an empty
    // spelling - the build downgrades the dangling handle rather than
    // storing a coordinate that could resolve to the wrong row.
    let mut dangling = RelocationPlan::with_target(target);
    dangling.push_record(record(
        SectionKind::Text,
        1,
        4,
        ObjectSymbolHandle::invalid(),
        0,
        RelocationKind::X86_64Relative32,
    ));
    let mut dangling_image = image_with(target, &object, &dangling, &encoded, &[]);
    let diagnostic = apply_x86_64_relocations(&mut dangling_image, &layout(), "test image")
        .expect_err("a dangling symbol handle has no address");
    assert!(
        diagnostic
            .message
            .contains("relocation references unknown symbol ``"),
        "unexpected diagnostic: {}",
        diagnostic.message,
    );
    assert_eq!(dangling_image.memory.text, encoded);
}

#[test]
fn x86_64_rel32_patches_at_the_signed_displacement_edge_and_rejects_past_it() {
    let target = NativeTarget::linux_x64();
    let encoded: Vec<u8> = [0xe8, 0, 0, 0, 0, 0xc3, 0x90, 0x90].into();
    let mut object = object_with_text(target, encoded.len());
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Data,
        size: 8,
        alignment: 8,
    });
    let entry = object_symbol(
        &mut object,
        "entry",
        SymbolSection::Section(SectionKind::Text),
        0,
        8,
        SymbolKind::Function,
    );
    let cell = object_symbol(
        &mut object,
        "cell",
        SymbolSection::Section(SectionKind::Data),
        0,
        8,
        SymbolKind::Object,
    );
    object.layout.entry_symbol = entry;

    let mut relocations = RelocationPlan::with_target(target);
    relocations.push_record(record(
        SectionKind::Text,
        1,
        4,
        cell,
        0,
        RelocationKind::X86_64Relative32,
    ));
    let data = [0u8; 8];

    // The rel32 delta is measured from the end of the four-byte field:
    // relocation_address = text_address + 1 + 4. A data symbol exactly
    // i32::MAX above that end still fits the field.
    let mut in_range = image_with(target, &object, &relocations, &encoded, &data);
    apply_x86_64_relocations(
        &mut in_range,
        &FinalImageLayout {
            text_address: TEXT_ADDRESS,
            data_address: TEXT_ADDRESS + 5 + i32::MAX as u64,
            bss_address: BSS_ADDRESS,
        },
        "test image",
    )
    .expect("a delta of i32::MAX still fits the displacement");
    assert_eq!(
        i32::from_le_bytes(in_range.memory.text[1..5].try_into().unwrap()),
        i32::MAX,
    );

    // One byte further and the same plan must refuse rather than wrap.
    let mut out_of_range = image_with(target, &object, &relocations, &encoded, &data);
    let diagnostic = apply_x86_64_relocations(
        &mut out_of_range,
        &FinalImageLayout {
            text_address: TEXT_ADDRESS,
            data_address: TEXT_ADDRESS + 5 + i32::MAX as u64 + 1,
            bss_address: BSS_ADDRESS,
        },
        "test image",
    )
    .expect_err("a delta of i32::MAX + 1 cannot be encoded");
    assert!(
        diagnostic
            .message
            .contains("x86_64 relative relocation is out of range"),
        "unexpected diagnostic: {}",
        diagnostic.message,
    );
    assert_eq!(
        out_of_range.memory.text, encoded,
        "the rejected plan must not patch the encoded text",
    );
    validate_final_text_relocation_envelope(&encoded, &out_of_range.memory.text, &relocations)
        .expect("the untouched text still replays against the rejected plan");

    // The negative edge is symmetric: i32::MIN applies, one byte below
    // refuses.
    let mut negative_edge = image_with(target, &object, &relocations, &encoded, &data);
    let high_text = 0x1_0000_0000u64;
    let negative_relocation_end = high_text + 5;
    apply_x86_64_relocations(
        &mut negative_edge,
        &FinalImageLayout {
            text_address: high_text,
            data_address: negative_relocation_end - (i32::MAX as u64) - 1,
            bss_address: BSS_ADDRESS,
        },
        "test image",
    )
    .expect("a delta of i32::MIN still fits the displacement");
    assert_eq!(
        i32::from_le_bytes(negative_edge.memory.text[1..5].try_into().unwrap()),
        i32::MIN,
    );

    let mut negative_out_of_range = image_with(target, &object, &relocations, &encoded, &data);
    let diagnostic = apply_x86_64_relocations(
        &mut negative_out_of_range,
        &FinalImageLayout {
            text_address: high_text,
            data_address: negative_relocation_end - (i32::MAX as u64) - 2,
            bss_address: BSS_ADDRESS,
        },
        "test image",
    )
    .expect_err("a delta of i32::MIN - 1 cannot be encoded");
    assert!(
        diagnostic
            .message
            .contains("x86_64 relative relocation is out of range"),
        "unexpected diagnostic: {}",
        diagnostic.message,
    );
    assert_eq!(negative_out_of_range.memory.text, encoded);
}

#[test]
fn x86_64_rejects_addend_targets_that_overflow_the_address_space() {
    let target = NativeTarget::linux_x64();
    let mut object = object_with_text(target, 8);
    let entry = object_symbol(
        &mut object,
        "entry",
        SymbolSection::Section(SectionKind::Text),
        0,
        8,
        SymbolKind::Function,
    );
    object.layout.entry_symbol = entry;

    // Pushing a symbol near the top of the address space further with a
    // positive addend wraps the target; the addend is signed, so dragging
    // a low address below zero wraps the other way.
    for (text_address, addend) in [(u64::MAX - 0x100, 0x200i64), (0x1000, -0x2000)] {
        let mut relocations = RelocationPlan::with_target(target);
        relocations.push_record(record(
            SectionKind::Data,
            0,
            8,
            entry,
            addend,
            RelocationKind::Absolute64,
        ));
        let mut image = image_with(target, &object, &relocations, &[0; 8], &[0; 8]);

        let diagnostic = apply_x86_64_relocations(
            &mut image,
            &FinalImageLayout {
                text_address,
                data_address: DATA_ADDRESS,
                bss_address: BSS_ADDRESS,
            },
            "test image",
        )
        .expect_err("an overflowing addend cannot produce a target address");
        assert!(
            diagnostic.message.contains(&format!(
                "x86_64 relocation target overflows after addend {addend}"
            )),
            "unexpected diagnostic: {}",
            diagnostic.message,
        );
        assert_eq!(
            image.memory.data,
            vec![0; 8],
            "the rejected plan must not patch the data image",
        );
    }
}

#[test]
fn x86_64_rejects_patches_to_sections_without_backing_bytes() {
    let target = NativeTarget::linux_x64();
    let encoded: Vec<u8> = [0xe8, 0, 0, 0, 0, 0xc3, 0x90, 0x90].into();
    let mut object = object_with_text(target, encoded.len());
    let entry = object_symbol(
        &mut object,
        "entry",
        SymbolSection::Section(SectionKind::Text),
        0,
        8,
        SymbolKind::Function,
    );
    object.layout.entry_symbol = entry;

    // `.bss` occupies layout but has no materialized bytes to write into;
    // both field kinds refuse it.
    for (kind, width, expected) in [
        (
            RelocationKind::Absolute64,
            8,
            "x86_64 absolute relocation targets non-materialized section Bss",
        ),
        (
            RelocationKind::X86_64Relative32,
            4,
            "x86_64 relative relocation targets non-materialized section Bss",
        ),
    ] {
        let mut relocations = RelocationPlan::with_target(target);
        relocations.push_record(record(SectionKind::Bss, 0, width, entry, 0, kind));
        let mut image = image_with(target, &object, &relocations, &encoded, &[]);

        let diagnostic = apply_x86_64_relocations(&mut image, &layout(), "test image")
            .expect_err("a relocation into .bss has no bytes to patch");
        assert!(
            diagnostic.message.contains(expected),
            "unexpected diagnostic: {}",
            diagnostic.message,
        );
        assert_eq!(image.memory.text, encoded);
    }

    // A record naming no section at all is not expressible in a
    // `RelocationPlan`, but emitters can mutate the final image directly;
    // the layout lookup refuses it before any byte is touched.
    let mut image = image_with(
        target,
        &object,
        &RelocationPlan::with_target(target),
        &encoded,
        &[],
    );
    let symbol = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "entry".into(),
        section: FinalImageSection::Text,
        offset: 0,
        size: 8,
        kind: SymbolKind::Function,
    });
    image
        .relocation_table
        .relocations
        .insert(FinalImageRelocation {
            section: FinalImageSection::None,
            offset: 0,
            byte_width: 4,
            symbol_handle: symbol,
            addend: 0,
            kind: RelocationKind::X86_64Relative32,
        });
    let diagnostic = apply_x86_64_relocations(&mut image, &layout(), "test image")
        .expect_err("a sectionless record has no address base");
    assert!(
        diagnostic
            .message
            .contains("x86_64 relative relocation has no section"),
        "unexpected diagnostic: {}",
        diagnostic.message,
    );
    assert_eq!(image.memory.text, encoded);

    // And the byte cursor stays bounds-checked: a field that runs past the
    // section it claims to patch is a diagnostic, not a panic.
    let mut relocations = RelocationPlan::with_target(target);
    relocations.push_record(record(
        SectionKind::Text,
        6,
        4,
        entry,
        0,
        RelocationKind::X86_64Relative32,
    ));
    let mut image = image_with(target, &object, &relocations, &encoded, &[]);
    let diagnostic = apply_x86_64_relocations(&mut image, &layout(), "test image")
        .expect_err("a field past the text end cannot be patched");
    assert!(
        diagnostic
            .message
            .contains("x86_64 relocation offset 6 is outside text section"),
        "unexpected diagnostic: {}",
        diagnostic.message,
    );
    assert_eq!(image.memory.text, encoded);
}

#[test]
fn aarch64_distinguishes_imports_from_sectionless_symbols() {
    let target = NativeTarget::linux_arm64();
    let encoded: Vec<u8> = [0x90; 16].into();
    let mut object = object_with_text(target, encoded.len());
    let entry = object_symbol(
        &mut object,
        "entry",
        SymbolSection::Section(SectionKind::Text),
        0,
        16,
        SymbolKind::Function,
    );
    let import = object_symbol(
        &mut object,
        "host_call",
        SymbolSection::None,
        0,
        0,
        SymbolKind::Import,
    );
    object.layout.symbols.get_mut(import).import_library = "libhost.so".into();
    // A sectionless symbol that is not an import: still no address, but the
    // plain unknown-symbol arm owns it.
    let loose = object_symbol(
        &mut object,
        "loose_cell",
        SymbolSection::None,
        0,
        0,
        SymbolKind::Object,
    );
    object.layout.entry_symbol = entry;

    let mut relocations = RelocationPlan::with_target(target);
    relocations.push_record(record(
        SectionKind::Text,
        8,
        4,
        import,
        0,
        RelocationKind::Aarch64Branch26,
    ));
    let mut image = image_with(target, &object, &relocations, &encoded, &[]);
    let diagnostic = apply_aarch64_relocations(&mut image, &layout(), "test image")
        .expect_err("an import symbol cannot be bound by the static patcher");
    assert!(
        diagnostic.message.contains(
            "test image cannot import `host_call` yet; use syscalls or add dynamic binding"
        ),
        "unexpected diagnostic: {}",
        diagnostic.message,
    );
    assert_eq!(image.memory.text, encoded);

    let mut relocations = RelocationPlan::with_target(target);
    relocations.push_record(record(
        SectionKind::Text,
        8,
        4,
        loose,
        0,
        RelocationKind::Aarch64Branch26,
    ));
    let mut image = image_with(target, &object, &relocations, &encoded, &[]);
    let diagnostic = apply_aarch64_relocations(&mut image, &layout(), "test image")
        .expect_err("a sectionless symbol has no address");
    assert!(
        diagnostic
            .message
            .contains("relocation references unknown symbol `loose_cell`"),
        "unexpected diagnostic: {}",
        diagnostic.message,
    );
    assert_eq!(image.memory.text, encoded);
    validate_final_text_relocation_envelope(&encoded, &image.memory.text, &relocations)
        .expect("the untouched text still replays against the rejected plan");
}

#[test]
fn aarch64_rejects_foreign_kinds_overflowing_addends_and_non_text_fields() {
    let target = NativeTarget::linux_arm64();
    let encoded: Vec<u8> = [0x90; 16].into();
    let mut object = object_with_text(target, encoded.len());
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Data,
        size: 8,
        alignment: 8,
    });
    let entry = object_symbol(
        &mut object,
        "entry",
        SymbolSection::Section(SectionKind::Text),
        0,
        16,
        SymbolKind::Function,
    );
    let cell = object_symbol(
        &mut object,
        "cell",
        SymbolSection::Section(SectionKind::Data),
        0,
        8,
        SymbolKind::Object,
    );
    object.layout.entry_symbol = entry;

    // An x86-64 record routed to the AArch64 patcher names the mismatch.
    let mut relocations = RelocationPlan::with_target(target);
    relocations.push_record(record(
        SectionKind::Text,
        0,
        4,
        entry,
        0,
        RelocationKind::X86_64Relative32,
    ));
    let mut image = image_with(target, &object, &relocations, &encoded, &[]);
    let diagnostic = apply_aarch64_relocations(&mut image, &layout(), "test image")
        .expect_err("the AArch64 patcher refuses an x86-64 relocation kind");
    assert!(
        diagnostic
            .message
            .contains("AArch64 image received x86_64 relocation"),
        "unexpected diagnostic: {}",
        diagnostic.message,
    );
    assert_eq!(image.memory.text, encoded);

    // An addend that wraps the resolved address is refused before the kind
    // is even read.
    let mut relocations = RelocationPlan::with_target(target);
    relocations.push_record(record(
        SectionKind::Data,
        0,
        8,
        entry,
        -0x2000,
        RelocationKind::Absolute64,
    ));
    let mut image = image_with(target, &object, &relocations, &encoded, &[0; 8]);
    let diagnostic = apply_aarch64_relocations(
        &mut image,
        &FinalImageLayout {
            text_address: 0x1000,
            data_address: DATA_ADDRESS,
            bss_address: BSS_ADDRESS,
        },
        "test image",
    )
    .expect_err("an addend that wraps the target address cannot produce one");
    assert!(
        diagnostic
            .message
            .contains("AArch64 relocation target overflows after addend -8192"),
        "unexpected diagnostic: {}",
        diagnostic.message,
    );

    // `.bss` has no bytes for an absolute slot either.
    let mut relocations = RelocationPlan::with_target(target);
    relocations.push_record(record(
        SectionKind::Bss,
        0,
        8,
        entry,
        0,
        RelocationKind::Absolute64,
    ));
    let mut image = image_with(target, &object, &relocations, &encoded, &[]);
    let diagnostic = apply_aarch64_relocations(&mut image, &layout(), "test image")
        .expect_err("a relocation into .bss has no bytes to patch");
    assert!(
        diagnostic
            .message
            .contains("AArch64 absolute relocation targets non-materialized section Bss"),
        "unexpected diagnostic: {}",
        diagnostic.message,
    );

    // Every instruction-field kind must land in `.text`: the field bits are
    // ISA immediates, not data bytes.
    for kind in [
        RelocationKind::Aarch64Page21,
        RelocationKind::Aarch64PageOffset12,
        RelocationKind::Aarch64Branch26,
    ] {
        let mut relocations = RelocationPlan::with_target(target);
        relocations.push_record(record(SectionKind::Data, 0, 4, cell, 0, kind));
        let mut image = image_with(target, &object, &relocations, &encoded, &[0; 8]);
        let diagnostic = apply_aarch64_relocations(&mut image, &layout(), "test image")
            .expect_err("an instruction field kind refuses a non-text home");
        assert!(
            diagnostic
                .message
                .contains("AArch64 instruction relocation targets non-text section Data"),
            "unexpected diagnostic: {}",
            diagnostic.message,
        );
        assert_eq!(
            image.memory.data,
            vec![0; 8],
            "the rejected plan must not patch the data image",
        );
    }
}

#[test]
fn aarch64_field_relocations_pin_their_range_and_alignment_boundaries() {
    let target = NativeTarget::linux_arm64();
    // adrp x0, #0; add x0, x0, #0; b #0; ret
    let encoded: Vec<u8> = [
        0x00, 0x00, 0x00, 0x90, 0x00, 0x00, 0x00, 0x91, 0x00, 0x00, 0x00, 0x14, 0xc0, 0x03, 0x5f,
        0xd6,
    ]
    .into();
    let mut object = object_with_text(target, encoded.len());
    object.layout.sections.insert(SectionPlan {
        kind: SectionKind::Data,
        size: 8,
        alignment: 8,
    });
    let entry = object_symbol(
        &mut object,
        "entry",
        SymbolSection::Section(SectionKind::Text),
        0,
        16,
        SymbolKind::Function,
    );
    let cell = object_symbol(
        &mut object,
        "cell",
        SymbolSection::Section(SectionKind::Data),
        0,
        8,
        SymbolKind::Object,
    );
    object.layout.entry_symbol = entry;

    // ADRP admits a signed 21-bit page count: |delta| < 2^20 pages. The
    // largest in-range target applies and the first out-of-range page
    // refuses.
    let mut relocations = RelocationPlan::with_target(target);
    relocations.push_record(record(
        SectionKind::Text,
        0,
        4,
        cell,
        0,
        RelocationKind::Aarch64Page21,
    ));

    let in_range_pages: u64 = (1 << 20) - 1;
    let mut image = image_with(target, &object, &relocations, &encoded, &[]);
    apply_aarch64_relocations(
        &mut image,
        &FinalImageLayout {
            text_address: TEXT_ADDRESS,
            data_address: TEXT_ADDRESS + in_range_pages * 4096,
            bss_address: BSS_ADDRESS,
        },
        "test image",
    )
    .expect("a page delta of 2^20 - 1 still fits ADRP");
    let patched = u32::from_le_bytes(image.memory.text[0..4].try_into().unwrap());
    let decoded_page_delta = ((patched >> 29) & 0b11) | (((patched >> 5) & 0x7ffff) << 2);
    assert_eq!(decoded_page_delta, in_range_pages as u32);

    let mut image = image_with(target, &object, &relocations, &encoded, &[]);
    let diagnostic = apply_aarch64_relocations(
        &mut image,
        &FinalImageLayout {
            text_address: TEXT_ADDRESS,
            data_address: TEXT_ADDRESS + (1u64 << 20) * 4096,
            bss_address: BSS_ADDRESS,
        },
        "test image",
    )
    .expect_err("a page delta of 2^20 cannot be encoded in ADRP");
    assert!(
        diagnostic
            .message
            .contains("AArch64 ADRP relocation is out of range: 1048576 page(s)"),
        "unexpected diagnostic: {}",
        diagnostic.message,
    );
    assert_eq!(image.memory.text, encoded);
    validate_final_text_relocation_envelope(&encoded, &image.memory.text, &relocations)
        .expect("the untouched text still replays against the rejected plan");

    // B/BL admit a signed 28-bit byte distance, instruction-aligned: the
    // stored immediate counts instructions, so |delta| < 2^25 applies and
    // exactly 2^25 refuses.
    let mut relocations = RelocationPlan::with_target(target);
    relocations.push_record(record(
        SectionKind::Text,
        8,
        4,
        cell,
        0,
        RelocationKind::Aarch64Branch26,
    ));

    let in_range_instructions: u64 = (1 << 25) - 1;
    let mut image = image_with(target, &object, &relocations, &encoded, &[]);
    apply_aarch64_relocations(
        &mut image,
        &FinalImageLayout {
            text_address: TEXT_ADDRESS,
            // delta = data - (text + 8); +8 keeps the count at 2^25 - 1.
            data_address: TEXT_ADDRESS + 8 + in_range_instructions * 4,
            bss_address: BSS_ADDRESS,
        },
        "test image",
    )
    .expect("an instruction delta of 2^25 - 1 still fits B");
    let patched = u32::from_le_bytes(image.memory.text[8..12].try_into().unwrap());
    assert_eq!(patched & 0x03ff_ffff, in_range_instructions as u32);

    let mut image = image_with(target, &object, &relocations, &encoded, &[]);
    let diagnostic = apply_aarch64_relocations(
        &mut image,
        &FinalImageLayout {
            text_address: TEXT_ADDRESS,
            data_address: TEXT_ADDRESS + 8 + (1u64 << 25) * 4,
            bss_address: BSS_ADDRESS,
        },
        "test image",
    )
    .expect_err("an instruction delta of 2^25 cannot be encoded in B");
    assert!(
        diagnostic
            .message
            .contains("AArch64 branch relocation is out of range: 33554432 instruction(s)"),
        "unexpected diagnostic: {}",
        diagnostic.message,
    );
    assert_eq!(image.memory.text, encoded);

    // Alignment is checked before range: a target two bytes away is not an
    // instruction boundary at any distance.
    let mut relocations = RelocationPlan::with_target(target);
    relocations.push_record(record(
        SectionKind::Text,
        8,
        4,
        entry,
        2,
        RelocationKind::Aarch64Branch26,
    ));
    let mut image = image_with(target, &object, &relocations, &encoded, &[]);
    let diagnostic = apply_aarch64_relocations(&mut image, &layout(), "test image")
        .expect_err("a non-aligned branch target cannot be encoded");
    assert!(
        diagnostic
            .message
            .contains("AArch64 branch relocation target is not instruction-aligned"),
        "unexpected diagnostic: {}",
        diagnostic.message,
    );
    assert_eq!(image.memory.text, encoded);

    // And the field cursor stays bounds-checked against the encoded bytes.
    let mut relocations = RelocationPlan::with_target(target);
    relocations.push_record(record(
        SectionKind::Text,
        14,
        4,
        cell,
        0,
        RelocationKind::Aarch64PageOffset12,
    ));
    let mut image = image_with(target, &object, &relocations, &encoded, &[]);
    let diagnostic = apply_aarch64_relocations(&mut image, &layout(), "test image")
        .expect_err("a field past the text end cannot be patched");
    assert!(
        diagnostic
            .message
            .contains("AArch64 relocation offset 14 is outside text section"),
        "unexpected diagnostic: {}",
        diagnostic.message,
    );
    assert_eq!(image.memory.text, encoded);
}
