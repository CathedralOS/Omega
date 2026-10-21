//! Image-local relocation tests: a dynamic image that imports one procedure
//! and materializes the address of its own compiler-private callback.

use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::candidates::{
    Candidate, derive_contents, validate_candidate,
};
use crate::dynamic_executable::file_assembly::resolved_procedure_linkage::compatibility_fingerprint::non_authoritative_resolved_linkage_compatibility_fingerprint;
use crate::{
    ValidatedElfDynamicFileEnvelope, apply_elf_dynamic_address_fixups,
    apply_elf_section_header_placements, emit_elf_dynamic_executable,
    plan_elf_dynamic_load_layout,
    plan_elf_indexed_section_payloads,
    plan_elf_relative_section_payload_layout,
    serialize_elf_dynamic_file_envelope,
};
use arena::Handle;
use image::{
    FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
    FinalImageSection, FinalImageSymbol,
};
use object_file::{RelocationKind, SymbolKind};
use target::{
    ForeignLocatorCandidate, TargetProfile,
    normalize_foreign_locator,
};

/// `.text` offset of the compiler-private callback function.
const PRIVATE_CALLBACK_OFFSET: usize = 40;
/// `.text` offset of the instruction that materializes the callback address.
const CALLBACK_MATERIALIZATION_OFFSET: usize = 8;
/// `.data` offset of an eight-byte pointer to the private callback.
const CALLBACK_POINTER_DATA_OFFSET: usize = 8;

/// One import-bearing image: an import call at `.text` 0, the callback
/// address materialization at `.text` 8 (x86-64 `LEA rax, [rip+rel32]`;
/// AArch64 `ADRP x0` + `ADD x0, x0, #lo12`), the entry at `.text` 24, the
/// private callback at `.text` 40, and a `.data` pointer to that callback.
fn callback_image(target: TargetProfile) -> FinalImage {
    let mut image = FinalImage::with_capacity(
        target.native_target(),
        FinalImageMemory {
            text: vec![0; 64],
            data: (0_u8..24).map(|byte| byte.wrapping_add(0x30)).collect(),
            bss_size: 16,
            bss_alignment: 8,
        },
        Handle::invalid(),
        3,
        1,
        4,
    );
    image.memory.data[CALLBACK_POINTER_DATA_OFFSET..CALLBACK_POINTER_DATA_OFFSET + 8].fill(0);
    let entry = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "_start".to_owned(),
        section: FinalImageSection::Text,
        offset: 24,
        size: 4,
        kind: SymbolKind::Function,
    });
    image.symbol_table.entry_symbol = entry;
    let private_callback = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "__omega_private_callback".to_owned(),
        section: FinalImageSection::Text,
        offset: PRIVATE_CALLBACK_OFFSET,
        size: 8,
        kind: SymbolKind::Function,
    });
    let import = image.symbol_table.symbols.insert(FinalImageSymbol {
        name: "__omega_callback_registrar".to_owned(),
        section: FinalImageSection::None,
        offset: 0,
        size: 0,
        kind: SymbolKind::Import,
    });
    image.symbol_table.imports.insert(FinalImageImport {
        symbol_handle: import,
        import: FinalImageImportPlan::Normalized(
            (normalize_foreign_locator(
                ForeignLocatorCandidate::ElfVersioned {
                    object: b"libcallback-registrar.so".to_vec(),
                    symbol: b"register_callback".to_vec(),
                    version: b"CALLBACK_1".to_vec(),
                },
                target,
            )
            .unwrap(),)
                .into(),
        ),
    });

    let mut relocate = |section, offset, byte_width, symbol_handle, kind| {
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                section,
                offset,
                byte_width,
                symbol_handle,
                addend: 0,
                kind,
            });
    };
    match target {
        TargetProfile::LinuxX64 => {
            relocate(
                FinalImageSection::Text,
                1,
                4,
                import,
                RelocationKind::X86_64Relative32,
            );
            relocate(
                FinalImageSection::Text,
                CALLBACK_MATERIALIZATION_OFFSET + 3,
                4,
                private_callback,
                RelocationKind::X86_64Relative32,
            );
        }
        TargetProfile::LinuxArm64 => {
            relocate(
                FinalImageSection::Text,
                0,
                4,
                import,
                RelocationKind::Aarch64Branch26,
            );
            relocate(
                FinalImageSection::Text,
                CALLBACK_MATERIALIZATION_OFFSET,
                4,
                private_callback,
                RelocationKind::Aarch64Page21,
            );
            relocate(
                FinalImageSection::Text,
                CALLBACK_MATERIALIZATION_OFFSET + 4,
                4,
                private_callback,
                RelocationKind::Aarch64PageOffset12,
            );
        }
        _ => unreachable!(),
    }
    relocate(
        FinalImageSection::Data,
        CALLBACK_POINTER_DATA_OFFSET,
        8,
        private_callback,
        RelocationKind::Absolute64,
    );

    match target {
        TargetProfile::LinuxX64 => {
            image.memory.text[0] = 0xe8;
            image.memory.text[CALLBACK_MATERIALIZATION_OFFSET..CALLBACK_MATERIALIZATION_OFFSET + 3]
                .copy_from_slice(&[0x48, 0x8d, 0x05]);
        }
        TargetProfile::LinuxArm64 => {
            image.memory.text[0..4].copy_from_slice(&[0, 0, 0, 0x94]);
            image.memory.text[CALLBACK_MATERIALIZATION_OFFSET..CALLBACK_MATERIALIZATION_OFFSET + 8]
                .copy_from_slice(&[0, 0, 0, 0x90, 0, 0, 0, 0x91]);
        }
        _ => unreachable!(),
    }
    image
}

use crate::test_support::interpreter;

fn envelope(target: TargetProfile) -> ValidatedElfDynamicFileEnvelope {
    let linkage = crate::test_support::resolved_linkage(callback_image(target), target);
    let headers = crate::test_support::section_header_table_from_linkage(linkage);
    let payloads = plan_elf_indexed_section_payloads(headers).unwrap();
    let relative = plan_elf_relative_section_payload_layout(payloads).unwrap();
    let load = plan_elf_dynamic_load_layout(relative).unwrap();
    let placed = apply_elf_section_header_placements(load).unwrap();
    let resolved = apply_elf_dynamic_address_fixups(placed).unwrap();
    serialize_elf_dynamic_file_envelope(resolved).unwrap()
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

/// Map one virtual address to its file offset through the file's own
/// `PT_LOAD` program headers, so the check reads the bytes a loader maps
/// rather than the writer's intermediate `.text`/`.data` copies.
fn file_offset_of(file: &[u8], address: u64) -> usize {
    const PT_LOAD: u32 = 1;
    let program_header_offset = read_u64(file, 32) as usize;
    let program_header_size = usize::from(u16::from_le_bytes([file[54], file[55]]));
    let program_header_count = usize::from(u16::from_le_bytes([file[56], file[57]]));
    for index in 0..program_header_count {
        let header = program_header_offset + index * program_header_size;
        if read_u32(file, header) != PT_LOAD {
            continue;
        }
        let file_offset = read_u64(file, header + 8);
        let virtual_address = read_u64(file, header + 16);
        let file_size = read_u64(file, header + 32);
        if (virtual_address..virtual_address + file_size).contains(&address) {
            return (file_offset + (address - virtual_address)) as usize;
        }
    }
    panic!("address 0x{address:x} is not file-backed by any PT_LOAD segment");
}

/// Decode the callback address the materializing instruction(s) at
/// `CALLBACK_MATERIALIZATION_OFFSET` compute when `.text` loads at
/// `text_address`.
fn materialized_callback_address(target: TargetProfile, text: &[u8], text_address: u64) -> u64 {
    let instruction_address = text_address + CALLBACK_MATERIALIZATION_OFFSET as u64;
    match target {
        TargetProfile::LinuxX64 => {
            let displacement = read_u32(text, CALLBACK_MATERIALIZATION_OFFSET + 3).cast_signed();
            (instruction_address + 7).wrapping_add_signed(i64::from(displacement))
        }
        TargetProfile::LinuxArm64 => {
            let adrp = read_u32(text, CALLBACK_MATERIALIZATION_OFFSET);
            let add = read_u32(text, CALLBACK_MATERIALIZATION_OFFSET + 4);
            let page_immediate = (((adrp >> 5) & 0x7ffff) << 2) | ((adrp >> 29) & 0b11);
            let signed_pages = i64::from(((page_immediate << 11) as i32) >> 11);
            let page = (instruction_address & !0xfff).wrapping_add_signed(signed_pages * 4096);
            page + u64::from((add >> 10) & 0xfff)
        }
        _ => unreachable!(),
    }
}

#[test]
fn dynamic_image_resolves_callback_address_and_data_pointer_to_the_private_function() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let admitted = emit_elf_dynamic_executable(callback_image(target), interpreter(target))
            .expect("import-bearing image with a private callback emits");
        let output = admitted.output();
        let text_address = output.final_image_layout.text_address;
        let data_address = output.final_image_layout.data_address;
        let private_callback = text_address + PRIVATE_CALLBACK_OFFSET as u64;

        assert_eq!(
            materialized_callback_address(target, &output.final_text_bytes, text_address),
            private_callback,
            "{target:?} callback address materialization must name the exact private function",
        );
        assert_eq!(
            read_u64(&output.final_data_bytes, CALLBACK_POINTER_DATA_OFFSET),
            private_callback,
            "{target:?} initialized-data callback pointer must hold the private function address",
        );

        // The published file carries the same patched fields at the offsets
        // its PT_LOAD segments map, not only the writer's final-byte copies.
        let text_file_offset = file_offset_of(&output.bytes, text_address);
        let file_text = &output.bytes[text_file_offset..text_file_offset + 64];
        assert_eq!(file_text, output.final_text_bytes.as_slice());
        assert_eq!(
            materialized_callback_address(target, file_text, text_address),
            private_callback,
        );
        let data_file_offset = file_offset_of(&output.bytes, data_address);
        assert_eq!(
            read_u64(
                &output.bytes,
                data_file_offset + CALLBACK_POINTER_DATA_OFFSET
            ),
            private_callback,
        );
        // Bytes outside the relocated fields are the encoded ones.
        assert_eq!(
            output.final_data_bytes[..CALLBACK_POINTER_DATA_OFFSET],
            callback_image(target).memory.data[..CALLBACK_POINTER_DATA_OFFSET],
        );

        // The retained image the admission hands back carries the relocated
        // memory it published.
        assert_eq!(admitted.image().memory.text, output.final_text_bytes);
        assert_eq!(admitted.image().memory.data, output.final_data_bytes);
    }
}

#[test]
fn substituted_callback_displacement_or_data_pointer_rejects_on_replay() {
    for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
        let envelope = envelope(target);
        let contents = derive_contents(&envelope).unwrap();
        let fingerprint =
            non_authoritative_resolved_linkage_compatibility_fingerprint(&envelope, &contents);
        let mut candidate = Candidate {
            envelope,
            contents,
            non_authoritative_resolved_linkage_compatibility_fingerprint: fingerprint,
        };

        // Move the materialized callback address 16 (x86-64 rel32) or 4
        // (AArch64 ADD imm12) bytes off the private function. The
        // procedure-fixup ledger owns no byte of that field, so only the
        // image-local replay can accept it.
        let field = match target {
            TargetProfile::LinuxX64 => CALLBACK_MATERIALIZATION_OFFSET + 3,
            TargetProfile::LinuxArm64 => CALLBACK_MATERIALIZATION_OFFSET + 5,
            _ => unreachable!(),
        };
        candidate.contents.source_text_bytes[field] ^= 0x10;
        candidate.non_authoritative_resolved_linkage_compatibility_fingerprint =
            non_authoritative_resolved_linkage_compatibility_fingerprint(
                &candidate.envelope,
                &candidate.contents,
            );
        let error = validate_candidate(candidate)
            .err()
            .expect("text drift rejects");
        assert!(
            error.diagnostic.message.contains("unowned byte"),
            "{}",
            error.diagnostic.message
        );

        let mut candidate = error.candidate;
        candidate.contents.source_text_bytes[field] ^= 0x10;
        candidate.contents.source_data_bytes[CALLBACK_POINTER_DATA_OFFSET] ^= 0x08;
        candidate.non_authoritative_resolved_linkage_compatibility_fingerprint =
            non_authoritative_resolved_linkage_compatibility_fingerprint(
                &candidate.envelope,
                &candidate.contents,
            );
        let error = validate_candidate(candidate)
            .err()
            .expect("data drift rejects");
        assert!(
            error.diagnostic.message.contains("source data"),
            "{}",
            error.diagnostic.message
        );

        let mut candidate = error.candidate;
        candidate.contents.source_data_bytes[CALLBACK_POINTER_DATA_OFFSET] ^= 0x08;
        candidate.non_authoritative_resolved_linkage_compatibility_fingerprint =
            non_authoritative_resolved_linkage_compatibility_fingerprint(
                &candidate.envelope,
                &candidate.contents,
            );
        validate_candidate(candidate)
            .map(drop)
            .unwrap_or_else(|error| panic!("restored candidate replays: {}", error.diagnostic));
    }
}
