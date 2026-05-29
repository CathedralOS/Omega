use omega_core::diagnostics::Diagnostic;
use omega_image::{ExecutableImageOutput, FinalImage, FinalImageLayout, apply_aarch64_relocations};

mod bytes;
mod code_signature;
mod constants;
mod entry;
mod imports;
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
use entry::macho_entry_text_offset;
use imports::{install_import_thunks, macho_bind_info, patch_import_thunks};
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
    let entry_offset = text_offset + macho_entry_text_offset(&image)?;
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
    write_macho_main_command(&mut bytes, entry_offset);
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
    let code_signature = macho_ad_hoc_code_signature(&bytes, text_file_size);
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
