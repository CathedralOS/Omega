use omega_core::diagnostics::Diagnostic;
use omega_image::{
    ExecutableImageOutput, FinalImage, FinalImageLayout, FinalImageSection,
    apply_aarch64_relocations,
};

mod bytes;
mod code_signature;
mod constants;
mod layout;
mod load_commands;

use code_signature::{code_signature_size, macho_ad_hoc_code_signature};
use constants::{
    MACHO_ARM64_PAGE_SIZE, MACHO_CODE_SIGNATURE_COMMAND_SIZE, MACHO_DYLD_INFO_COMMAND_SIZE,
    MACHO_DYSYMTAB_COMMAND_SIZE, MACHO_EXECUTABLE_BASE,
    MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE, MACHO_HEADER_SIZE,
    MACHO_LOAD_DYLINKER_COMMAND_SIZE, MACHO_LOAD_LIBSYSTEM_COMMAND_SIZE, MACHO_MAIN_COMMAND_SIZE,
    MACHO_SECTION_SIZE, MACHO_SEGMENT_COMMAND_SIZE, MACHO_SYMTAB_COMMAND_SIZE,
    MACHO_UUID_COMMAND_SIZE,
};
use layout::{align_to, align_to_u64};
use load_commands::{
    write_empty_macho_dysymtab_command, write_empty_macho_symtab_command,
    write_macho_code_signature_command, write_macho_dyld_info_command,
    write_macho_executable_build_version_command, write_macho_executable_data_segment,
    write_macho_executable_header, write_macho_executable_text_segment,
    write_macho_linkedit_segment, write_macho_load_dylinker_command,
    write_macho_load_libsystem_command, write_macho_main_command, write_macho_pagezero_segment,
    write_macho_uuid_command,
};

pub fn emit_macho_aarch64_executable(
    mut image: FinalImage,
) -> Result<ExecutableImageOutput, Diagnostic> {
    let import_thunks = install_import_thunks(&mut image);
    let has_imports = !import_thunks.is_empty();
    let data_section_count = usize::from(!image.data.is_empty()) + usize::from(image.bss_size > 0);
    let has_data_segment = data_section_count > 0;
    let command_count = 11 + usize::from(has_data_segment) + usize::from(has_imports);
    let sizeofcmds = MACHO_SEGMENT_COMMAND_SIZE
        + (MACHO_SEGMENT_COMMAND_SIZE + MACHO_SECTION_SIZE)
        + usize::from(has_data_segment)
            * (MACHO_SEGMENT_COMMAND_SIZE + data_section_count * MACHO_SECTION_SIZE)
        + MACHO_LOAD_DYLINKER_COMMAND_SIZE
        + MACHO_UUID_COMMAND_SIZE
        + MACHO_EXECUTABLE_BUILD_VERSION_COMMAND_SIZE
        + MACHO_MAIN_COMMAND_SIZE
        + MACHO_LOAD_LIBSYSTEM_COMMAND_SIZE
        + usize::from(has_imports) * MACHO_DYLD_INFO_COMMAND_SIZE
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

    patch_import_thunks(&mut image, &layout, &import_thunks)?;
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
    let bind_info = macho_bind_info(&import_thunks);
    let bind_offset = align_to(unsigned_file_end, MACHO_ARM64_PAGE_SIZE);
    let code_signature_offset = align_to(bind_offset + bind_info.len(), MACHO_ARM64_PAGE_SIZE);
    let code_signature_size = code_signature_size(code_signature_offset);
    let linkedit_vmaddr = MACHO_EXECUTABLE_BASE + bind_offset as u64;
    let linkedit_filesize = code_signature_offset + code_signature_size - bind_offset;
    let linkedit_vmsize = align_to(linkedit_filesize, MACHO_ARM64_PAGE_SIZE);

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
    write_macho_uuid_command(&mut bytes);
    write_macho_executable_build_version_command(&mut bytes);
    write_macho_main_command(&mut bytes, text_offset);
    write_macho_load_libsystem_command(&mut bytes);
    if has_imports {
        write_macho_dyld_info_command(&mut bytes, bind_offset, bind_info.len());
    }
    write_macho_linkedit_segment(
        &mut bytes,
        linkedit_vmaddr,
        bind_offset,
        linkedit_filesize,
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
    bytes.resize(bind_offset, 0);
    bytes.extend(bind_info);
    bytes.resize(code_signature_offset, 0);
    let code_signature = macho_ad_hoc_code_signature(&bytes);
    debug_assert_eq!(code_signature.len(), code_signature_size);
    bytes.extend(code_signature);

    Ok(ExecutableImageOutput {
        bytes,
        file_name: "omega-program".to_owned(),
        format: "mach-o-arm64-executable".to_owned(),
        text_bytes: image.text.len(),
        data_bytes: image.data.len(),
        bss_bytes: image.bss_size,
        symbols: image.symbols.len(),
        imports: image.imports.len(),
        relocations: image.relocations.len(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MachoImportThunk {
    symbol: String,
    text_offset: usize,
    data_offset: usize,
}

fn install_import_thunks(image: &mut FinalImage) -> Vec<MachoImportThunk> {
    let imports = image
        .imports
        .iter()
        .map(|(_, import)| import.symbol.clone())
        .collect::<Vec<_>>();
    let mut thunks = Vec::new();

    for symbol in imports {
        let text_offset = image.text.len();
        image.data.resize(align_to(image.data.len(), 8), 0);
        let data_offset = image.data.len();
        image.text.extend([0u8; 12]);
        image.data.extend([0u8; 8]);

        image.symbols.for_each_mut(|_, image_symbol| {
            if image_symbol.name == symbol {
                image_symbol.section = FinalImageSection::Text;
                image_symbol.offset = text_offset;
                image_symbol.size = 12;
            }
        });

        thunks.push(MachoImportThunk {
            symbol,
            text_offset,
            data_offset,
        });
    }

    thunks
}

fn patch_import_thunks(
    image: &mut FinalImage,
    layout: &FinalImageLayout,
    thunks: &[MachoImportThunk],
) -> Result<(), Diagnostic> {
    for thunk in thunks {
        let instruction_address = layout.text_address + thunk.text_offset as u64;
        let pointer_address = layout.data_address + thunk.data_offset as u64;
        patch_aarch64_adrp(
            &mut image.text,
            thunk.text_offset,
            instruction_address,
            pointer_address,
        )?;
        patch_aarch64_ldr_x_from_page(
            &mut image.text,
            thunk.text_offset + 4,
            pointer_address,
            16,
            16,
        )?;
        write_u32_at(&mut image.text, thunk.text_offset + 8, 0xd61f_0200)?;
    }

    Ok(())
}

fn macho_bind_info(thunks: &[MachoImportThunk]) -> Vec<u8> {
    let mut bytes = Vec::new();

    for thunk in thunks {
        bytes.push(0x11);
        bytes.push(0x40);
        bytes.extend(thunk.symbol.as_bytes());
        bytes.push(0);
        bytes.push(0x51);
        bytes.push(0x72);
        write_uleb128(&mut bytes, thunk.data_offset as u64);
        bytes.push(0x90);
    }
    if !thunks.is_empty() {
        bytes.push(0);
    }

    bytes
}

fn write_uleb128(bytes: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        bytes.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn patch_aarch64_adrp(
    text: &mut [u8],
    offset: usize,
    instruction_address: u64,
    symbol_address: u64,
) -> Result<(), Diagnostic> {
    let instruction_page = instruction_address & !0xfff;
    let symbol_page = symbol_address & !0xfff;
    let page_delta = (symbol_page as i64 - instruction_page as i64) / 4096;

    if !(-(1 << 20)..(1 << 20)).contains(&page_delta) {
        return Err(Diagnostic::error(format!(
            "Mach-O AArch64 import thunk ADRP is out of range: {page_delta} page(s)"
        )));
    }

    let immediate = (page_delta as u32) & 0x1f_ffff;
    let immediate_low = immediate & 0b11;
    let immediate_high = (immediate >> 2) & 0x7ffff;
    let instruction = 0x9000_0000 | (immediate_low << 29) | (immediate_high << 5) | 16;
    write_u32_at(text, offset, instruction)
}

fn patch_aarch64_ldr_x_from_page(
    text: &mut [u8],
    offset: usize,
    symbol_address: u64,
    register: u8,
    base_register: u8,
) -> Result<(), Diagnostic> {
    let page_offset = symbol_address & 0xfff;
    if !page_offset.is_multiple_of(8) {
        return Err(Diagnostic::error(
            "Mach-O AArch64 import thunk pointer is not 8-byte aligned",
        ));
    }
    let scaled_offset = u32::try_from(page_offset / 8)
        .expect("Mach-O AArch64 import thunk pointer offset overflow");
    if scaled_offset > 0xfff {
        return Err(Diagnostic::error(
            "Mach-O AArch64 import thunk pointer page offset is too large",
        ));
    }
    let instruction =
        0xf940_0000 | (scaled_offset << 10) | (u32::from(base_register) << 5) | u32::from(register);
    write_u32_at(text, offset, instruction)
}

fn write_u32_at(text: &mut [u8], offset: usize, value: u32) -> Result<(), Diagnostic> {
    let Some(slot) = text.get_mut(offset..offset + 4) else {
        return Err(Diagnostic::error(format!(
            "Mach-O AArch64 import thunk patch offset {offset} is out of bounds"
        )));
    };
    slot.copy_from_slice(&value.to_le_bytes());
    Ok(())
}
