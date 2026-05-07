use crate::emitter::{EmittedNativeOutput, NativeOutputKind};
use crate::final_image::{FinalImageLayout, apply_aarch64_relocations, build_final_image};
use crate::plan::NativePlan;
use omega_core::diagnostics::Diagnostic;
use sha2::{Digest, Sha256};

const MACHO_EXECUTABLE_BASE: u64 = 0x1_0000_0000;
const MACHO_ARM64_PAGE_SIZE: usize = 0x4000;
const MACHO_HEADER_SIZE: usize = 32;
const MACHO_SEGMENT_COMMAND_SIZE: usize = 72;
const MACHO_SECTION_SIZE: usize = 80;
const MACHO_LOAD_DYLINKER_COMMAND_SIZE: usize = 32;
const MACHO_MAIN_COMMAND_SIZE: usize = 24;
const MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE: usize = 32;
const MACHO_LOAD_LIBSYSTEM_COMMAND_SIZE: usize = 56;
const MACHO_SYMTAB_COMMAND_SIZE: usize = 24;
const MACHO_DYSYMTAB_COMMAND_SIZE: usize = 80;
const MACHO_CODE_SIGNATURE_COMMAND_SIZE: usize = 16;
const MACHO_HEADER_FLAGS_NOUNDEFS: u32 = 0x1;
const MACHO_HEADER_FLAGS_DYLDLINK: u32 = 0x4;
const MACHO_HEADER_FLAGS_TWOLEVEL: u32 = 0x80;
const MACHO_HEADER_FLAGS_PIE: u32 = 0x20_0000;
const CODE_SIGNATURE_PAGE_SIZE: usize = 4096;
const CODE_SIGNATURE_PAGE_SIZE_POWER: u8 = 12;

pub fn emit_macho_arm64_executable(
    native_plan: &NativePlan,
) -> Result<EmittedNativeOutput, Diagnostic> {
    let mut image = build_final_image(native_plan);
    if !image.imports.is_empty() {
        return Err(Diagnostic::error(
            "Mach-O direct executable cannot import dynamic symbols yet",
        ));
    }

    let data_section_count = usize::from(!image.data.is_empty()) + usize::from(image.bss_size > 0);
    let has_data_segment = data_section_count > 0;
    let command_count = 10 + usize::from(has_data_segment);
    let sizeofcmds = MACHO_SEGMENT_COMMAND_SIZE
        + (MACHO_SEGMENT_COMMAND_SIZE + MACHO_SECTION_SIZE)
        + usize::from(has_data_segment)
            * (MACHO_SEGMENT_COMMAND_SIZE + data_section_count * MACHO_SECTION_SIZE)
        + MACHO_LOAD_DYLINKER_COMMAND_SIZE
        + MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE
        + MACHO_MAIN_COMMAND_SIZE
        + MACHO_LOAD_LIBSYSTEM_COMMAND_SIZE
        + MACHO_SYMTAB_COMMAND_SIZE
        + MACHO_DYSYMTAB_COMMAND_SIZE
        + MACHO_SEGMENT_COMMAND_SIZE
        + MACHO_CODE_SIGNATURE_COMMAND_SIZE;
    let text_offset = align_to(MACHO_HEADER_SIZE + sizeofcmds, 16);
    let data_offset = align_to(text_offset + image.text.len(), MACHO_ARM64_PAGE_SIZE);
    let text_address = MACHO_EXECUTABLE_BASE + text_offset as u64;
    let data_address = MACHO_EXECUTABLE_BASE + data_offset as u64;
    let bss_address = align_to_u64(
        data_address + image.data.len() as u64,
        image.bss_alignment as u64,
    );
    let layout = FinalImageLayout {
        text_address,
        data_address,
        bss_address,
    };

    apply_aarch64_relocations(&mut image, &layout, "Mach-O direct executable")?;

    let text_file_size = if has_data_segment {
        data_offset
    } else {
        align_to(text_offset + image.text.len(), MACHO_ARM64_PAGE_SIZE)
    };
    let data_memory_size = if has_data_segment {
        (bss_address - data_address)
            .checked_add(image.bss_size as u64)
            .expect("Mach-O data memory size overflow")
    } else {
        0
    };
    let data_vm_size = align_to_u64(data_memory_size, MACHO_ARM64_PAGE_SIZE as u64);

    let unsigned_file_end = if has_data_segment {
        data_offset + image.data.len()
    } else {
        text_offset + image.text.len()
    };
    let code_signature_offset = align_to(unsigned_file_end, MACHO_ARM64_PAGE_SIZE);
    let code_signature_size = code_signature_size(code_signature_offset);
    let linkedit_vmaddr = MACHO_EXECUTABLE_BASE + code_signature_offset as u64;
    let linkedit_vmsize = align_to(code_signature_size, MACHO_ARM64_PAGE_SIZE);

    let mut bytes = Vec::new();
    write_macho_executable_header(&mut bytes, command_count, sizeofcmds);
    write_macho_pagezero_segment(&mut bytes);
    write_macho_executable_text_segment(&mut bytes, text_offset, image.text.len(), text_file_size);
    if has_data_segment {
        write_macho_executable_data_segment(
            &mut bytes,
            data_offset,
            image.data.len(),
            image.bss_size,
            data_vm_size,
            image.bss_alignment,
        );
    }
    write_macho_load_dylinker_command(&mut bytes);
    write_macho_executable_build_version_command(&mut bytes);
    write_macho_main_command(&mut bytes, text_offset);
    write_macho_load_libsystem_command(&mut bytes);
    write_macho_linkedit_segment(
        &mut bytes,
        linkedit_vmaddr,
        code_signature_offset,
        code_signature_size,
        linkedit_vmsize,
    );
    write_empty_macho_symtab_command(&mut bytes);
    write_empty_macho_dysymtab_command(&mut bytes);
    write_macho_code_signature_command(&mut bytes, code_signature_offset, code_signature_size);
    bytes.resize(text_offset, 0);
    bytes.extend(&image.text);
    if has_data_segment {
        bytes.resize(data_offset, 0);
        bytes.extend(&image.data);
    }
    bytes.resize(code_signature_offset, 0);
    let code_signature = macho_ad_hoc_code_signature(&bytes);
    debug_assert_eq!(code_signature.len(), code_signature_size);
    bytes.extend(code_signature);

    Ok(EmittedNativeOutput {
        bytes,
        file_name: "omega-program".to_owned(),
        format: "mach-o-arm64-executable".to_owned(),
        kind: NativeOutputKind::DirectExecutable,
        text_bytes: image.text.len(),
        data_bytes: image.data.len(),
        bss_bytes: image.bss_size,
        symbols: image.symbols.len(),
        relocations: image.relocations.len(),
        final_image_symbols: image.symbols.len(),
        final_image_imports: image.imports.len(),
        final_image_relocations: image.relocations.len(),
    })
}

fn write_macho_executable_header(bytes: &mut Vec<u8>, command_count: usize, sizeofcmds: usize) {
    write_macho_header_for(
        bytes,
        2,
        command_count,
        sizeofcmds,
        MACHO_HEADER_FLAGS_NOUNDEFS
            | MACHO_HEADER_FLAGS_DYLDLINK
            | MACHO_HEADER_FLAGS_TWOLEVEL
            | MACHO_HEADER_FLAGS_PIE,
    );
}

fn write_macho_header_for(
    bytes: &mut Vec<u8>,
    file_type: u32,
    command_count: usize,
    sizeofcmds: usize,
    flags: u32,
) {
    write_u32(bytes, 0xfeedfacf);
    write_u32(bytes, 0x0100000c);
    write_u32(bytes, 0);
    write_u32(bytes, file_type);
    write_u32(
        bytes,
        u32::try_from(command_count).expect("Mach-O command count overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(sizeofcmds).expect("Mach-O load command size overflow"),
    );
    write_u32(bytes, flags);
    write_u32(bytes, 0);
}

fn write_macho_pagezero_segment(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0x19);
    write_u32(bytes, MACHO_SEGMENT_COMMAND_SIZE as u32);
    write_fixed_string_16(bytes, "__PAGEZERO");
    write_u64(bytes, 0);
    write_u64(bytes, MACHO_EXECUTABLE_BASE);
    write_u64(bytes, 0);
    write_u64(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

fn write_macho_executable_text_segment(
    bytes: &mut Vec<u8>,
    text_offset: usize,
    text_size: usize,
    text_file_size: usize,
) {
    write_u32(bytes, 0x19);
    write_u32(
        bytes,
        u32::try_from(MACHO_SEGMENT_COMMAND_SIZE + MACHO_SECTION_SIZE)
            .expect("Mach-O text segment command size overflow"),
    );
    write_fixed_string_16(bytes, "__TEXT");
    write_u64(bytes, MACHO_EXECUTABLE_BASE);
    write_u64(
        bytes,
        align_to_u64(text_file_size as u64, MACHO_ARM64_PAGE_SIZE as u64),
    );
    write_u64(bytes, 0);
    write_u64(
        bytes,
        u64::try_from(text_file_size).expect("Mach-O text file size overflow"),
    );
    write_u32(bytes, 5);
    write_u32(bytes, 5);
    write_u32(bytes, 1);
    write_u32(bytes, 0);

    write_fixed_string_16(bytes, "__text");
    write_fixed_string_16(bytes, "__TEXT");
    write_u64(bytes, MACHO_EXECUTABLE_BASE + text_offset as u64);
    write_u64(
        bytes,
        u64::try_from(text_size).expect("Mach-O text size overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(text_offset).expect("Mach-O text offset overflow"),
    );
    write_u32(bytes, 2);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0x80000400);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

fn write_macho_executable_data_segment(
    bytes: &mut Vec<u8>,
    data_offset: usize,
    data_size: usize,
    bss_size: usize,
    data_vm_size: u64,
    bss_alignment: usize,
) {
    let section_count = usize::from(data_size > 0) + usize::from(bss_size > 0);
    write_u32(bytes, 0x19);
    write_u32(
        bytes,
        u32::try_from(MACHO_SEGMENT_COMMAND_SIZE + section_count * MACHO_SECTION_SIZE)
            .expect("Mach-O data segment command size overflow"),
    );
    write_fixed_string_16(bytes, "__DATA");
    write_u64(bytes, MACHO_EXECUTABLE_BASE + data_offset as u64);
    write_u64(bytes, data_vm_size);
    write_u64(
        bytes,
        u64::try_from(data_offset).expect("Mach-O data file offset overflow"),
    );
    write_u64(
        bytes,
        u64::try_from(data_size).expect("Mach-O data file size overflow"),
    );
    write_u32(bytes, 3);
    write_u32(bytes, 3);
    write_u32(
        bytes,
        u32::try_from(section_count).expect("Mach-O data section count overflow"),
    );
    write_u32(bytes, 0);

    if data_size > 0 {
        write_macho_executable_data_section(bytes, data_offset, data_size);
    }
    if bss_size > 0 {
        write_macho_executable_bss_section(bytes, data_offset + data_size, bss_size, bss_alignment);
    }
}

fn write_macho_executable_data_section(bytes: &mut Vec<u8>, data_offset: usize, data_size: usize) {
    write_fixed_string_16(bytes, "__data");
    write_fixed_string_16(bytes, "__DATA");
    write_u64(bytes, MACHO_EXECUTABLE_BASE + data_offset as u64);
    write_u64(
        bytes,
        u64::try_from(data_size).expect("Mach-O data section size overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(data_offset).expect("Mach-O data section offset overflow"),
    );
    write_u32(bytes, 3);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

fn write_macho_executable_bss_section(
    bytes: &mut Vec<u8>,
    bss_address_offset: usize,
    bss_size: usize,
    bss_alignment: usize,
) {
    write_fixed_string_16(bytes, "__bss");
    write_fixed_string_16(bytes, "__DATA");
    write_u64(bytes, MACHO_EXECUTABLE_BASE + bss_address_offset as u64);
    write_u64(
        bytes,
        u64::try_from(bss_size).expect("Mach-O bss section size overflow"),
    );
    write_u32(bytes, 0);
    write_u32(bytes, alignment_power(bss_alignment));
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 1);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

fn write_macho_load_dylinker_command(bytes: &mut Vec<u8>) {
    let start = bytes.len();
    write_u32(bytes, 0xe);
    write_u32(bytes, MACHO_LOAD_DYLINKER_COMMAND_SIZE as u32);
    write_u32(bytes, 12);
    bytes.extend(b"/usr/lib/dyld\0");
    bytes.resize(start + MACHO_LOAD_DYLINKER_COMMAND_SIZE, 0);
}

fn write_macho_main_command(bytes: &mut Vec<u8>, entry_offset: usize) {
    write_u32(bytes, 0x80000028);
    write_u32(bytes, MACHO_MAIN_COMMAND_SIZE as u32);
    write_u64(
        bytes,
        u64::try_from(entry_offset).expect("Mach-O entry offset overflow"),
    );
    write_u64(bytes, 0);
}

fn write_macho_executable_build_version_command(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0x32);
    write_u32(bytes, MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE as u32);
    write_u32(bytes, 1);
    write_u32(bytes, 13 << 16);
    write_u32(bytes, 13 << 16);
    write_u32(bytes, 1);
    write_u32(bytes, 3);
    write_u32(bytes, 0);
}

fn write_macho_load_libsystem_command(bytes: &mut Vec<u8>) {
    let start = bytes.len();
    write_u32(bytes, 0xc);
    write_u32(bytes, MACHO_LOAD_LIBSYSTEM_COMMAND_SIZE as u32);
    write_u32(bytes, 24);
    write_u32(bytes, 2);
    write_u32(bytes, 1351 << 16);
    write_u32(bytes, 1 << 16);
    bytes.extend(b"/usr/lib/libSystem.B.dylib\0");
    bytes.resize(start + MACHO_LOAD_LIBSYSTEM_COMMAND_SIZE, 0);
}

fn write_macho_linkedit_segment(
    bytes: &mut Vec<u8>,
    vmaddr: u64,
    file_offset: usize,
    file_size: usize,
    vm_size: usize,
) {
    write_u32(bytes, 0x19);
    write_u32(bytes, MACHO_SEGMENT_COMMAND_SIZE as u32);
    write_fixed_string_16(bytes, "__LINKEDIT");
    write_u64(bytes, vmaddr);
    write_u64(
        bytes,
        u64::try_from(vm_size).expect("Mach-O LINKEDIT vm size overflow"),
    );
    write_u64(
        bytes,
        u64::try_from(file_offset).expect("Mach-O LINKEDIT file offset overflow"),
    );
    write_u64(
        bytes,
        u64::try_from(file_size).expect("Mach-O LINKEDIT file size overflow"),
    );
    write_u32(bytes, 1);
    write_u32(bytes, 1);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

fn write_macho_code_signature_command(
    bytes: &mut Vec<u8>,
    code_signature_offset: usize,
    code_signature_size: usize,
) {
    write_u32(bytes, 0x1d);
    write_u32(bytes, MACHO_CODE_SIGNATURE_COMMAND_SIZE as u32);
    write_u32(
        bytes,
        u32::try_from(code_signature_offset).expect("Mach-O code signature offset overflow"),
    );
    write_u32(
        bytes,
        u32::try_from(code_signature_size).expect("Mach-O code signature size overflow"),
    );
}

fn write_empty_macho_symtab_command(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0x2);
    write_u32(bytes, MACHO_SYMTAB_COMMAND_SIZE as u32);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
    write_u32(bytes, 0);
}

fn write_empty_macho_dysymtab_command(bytes: &mut Vec<u8>) {
    write_u32(bytes, 0xb);
    write_u32(bytes, MACHO_DYSYMTAB_COMMAND_SIZE as u32);
    for _ in 0..18 {
        write_u32(bytes, 0);
    }
}

fn alignment_power(alignment: usize) -> u32 {
    alignment.max(1).trailing_zeros()
}

fn align_to(value: usize, alignment: usize) -> usize {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}

fn align_to_u64(value: u64, alignment: u64) -> u64 {
    let alignment = alignment.max(1);
    value.div_ceil(alignment) * alignment
}

fn code_signature_size(code_limit: usize) -> usize {
    let page_count = code_slot_count(code_limit);
    let identifier = code_signature_identifier();
    let code_directory_header_size = 88usize;
    let code_directory_length =
        align_to(code_directory_header_size + identifier.len() + 1, 4) + page_count * 32;
    let super_blob_length = 20 + code_directory_length;
    align_to(super_blob_length, 16)
}

fn macho_ad_hoc_code_signature(code_bytes: &[u8]) -> Vec<u8> {
    let code_limit = code_bytes.len();
    let page_count = code_slot_count(code_limit);
    let identifier = code_signature_identifier();
    let code_directory_header_size = 88usize;
    let identifier_offset = code_directory_header_size;
    let hash_offset = align_to(identifier_offset + identifier.len() + 1, 4);
    let code_directory_length = hash_offset + page_count * 32;
    let super_blob_length = 20 + code_directory_length;

    let mut bytes = Vec::with_capacity(align_to(super_blob_length, 16));
    write_be_u32(&mut bytes, 0xfade0cc0);
    write_be_u32(
        &mut bytes,
        u32::try_from(super_blob_length).expect("code signature size overflow"),
    );
    write_be_u32(&mut bytes, 1);
    write_be_u32(&mut bytes, 0);
    write_be_u32(&mut bytes, 20);

    write_be_u32(&mut bytes, 0xfade0c02);
    write_be_u32(
        &mut bytes,
        u32::try_from(code_directory_length).expect("CodeDirectory size overflow"),
    );
    write_be_u32(&mut bytes, 0x20400);
    write_be_u32(&mut bytes, 0x2);
    write_be_u32(
        &mut bytes,
        u32::try_from(hash_offset).expect("CodeDirectory hash offset overflow"),
    );
    write_be_u32(
        &mut bytes,
        u32::try_from(identifier_offset).expect("CodeDirectory identifier offset overflow"),
    );
    write_be_u32(&mut bytes, 0);
    write_be_u32(
        &mut bytes,
        u32::try_from(page_count).expect("CodeDirectory page count overflow"),
    );
    write_be_u32(
        &mut bytes,
        u32::try_from(code_limit).expect("CodeDirectory code limit overflow"),
    );
    bytes.push(32);
    bytes.push(2);
    bytes.push(0);
    bytes.push(CODE_SIGNATURE_PAGE_SIZE_POWER);
    write_be_u32(&mut bytes, 0);
    write_be_u32(&mut bytes, 0);
    write_be_u32(&mut bytes, 0);
    write_be_u32(&mut bytes, 0);
    write_be_u64(
        &mut bytes,
        u64::try_from(code_limit).expect("CodeDirectory code limit overflow"),
    );
    write_be_u64(&mut bytes, MACHO_EXECUTABLE_BASE);
    write_be_u64(&mut bytes, 0);
    write_be_u64(&mut bytes, 0);

    debug_assert_eq!(bytes.len(), 20 + identifier_offset);
    bytes.extend(identifier.as_bytes());
    bytes.push(0);
    bytes.resize(20 + hash_offset, 0);

    for page_index in 0..page_count {
        let start = page_index * CODE_SIGNATURE_PAGE_SIZE;
        let end = (start + CODE_SIGNATURE_PAGE_SIZE).min(code_limit);
        let digest = Sha256::digest(&code_bytes[start..end]);
        bytes.extend(digest);
    }

    bytes.resize(align_to(super_blob_length, 16), 0);
    bytes
}

fn code_slot_count(code_limit: usize) -> usize {
    code_limit.div_ceil(CODE_SIGNATURE_PAGE_SIZE)
}

fn code_signature_identifier() -> &'static str {
    "omega-program"
}

fn write_be_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_be_bytes());
}

fn write_be_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_be_bytes());
}

fn write_fixed_string_16(bytes: &mut Vec<u8>, value: &str) {
    let value_bytes = value.as_bytes();
    assert!(
        value_bytes.len() <= 16,
        "fixed Mach-O string is longer than 16 bytes"
    );
    bytes.extend(value_bytes);
    bytes.resize(bytes.len() + (16 - value_bytes.len()), 0);
}

fn write_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend(value.to_le_bytes());
}

fn write_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend(value.to_le_bytes());
}
