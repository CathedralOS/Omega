use omega_image::{
    ExecutableImageOutput, FinalImage, apply_aarch64_relocations, place_executable_regions,
};
use psi_diagnostics::Diagnostic;

mod bytes;
mod code_signature;
mod constants;
mod entry;
mod imports;
mod layout;
mod load_commands;
mod plan;

use code_signature::macho_ad_hoc_code_signature;
use entry::macho_entry_text_offset;
use imports::{
    install_import_thunks, macho_bind_info, macho_dylib_list, patch_import_thunks,
    validate_import_thunk_footprints,
};
use load_commands::{
    write_empty_macho_dysymtab_command, write_empty_macho_symtab_command,
    write_macho_code_signature_command, write_macho_dyld_info_command,
    write_macho_executable_build_version_command, write_macho_executable_data_segment,
    write_macho_executable_header, write_macho_executable_text_segment,
    write_macho_linkedit_segment, write_macho_load_dylib_command,
    write_macho_load_dylinker_command, write_macho_main_command, write_macho_pagezero_segment,
    write_macho_uuid_command,
};
use plan::plan_macho_image;

pub fn emit_macho_aarch64_executable(
    mut image: FinalImage,
) -> Result<ExecutableImageOutput, Diagnostic> {
    let import_thunks = install_import_thunks(&mut image)?;
    // The ordered set of dylibs to load (libSystem first, then libobjc/etc.); each
    // import binds against its library's ordinal (index + 1).
    let dylibs = macho_dylib_list(&import_thunks);
    let bind_info = macho_bind_info(&import_thunks, &dylibs);
    let plan = plan_macho_image(&image, import_thunks.len(), bind_info.len(), &dylibs);
    let entry_offset = plan.text_offset + macho_entry_text_offset(&image)?;
    let layout = plan.final_image_layout();

    patch_import_thunks(&mut image, &layout, &import_thunks)?;
    apply_aarch64_relocations(&mut image, &layout, "Mach-O direct executable")?;
    validate_import_thunk_footprints(&mut image, &import_thunks)?;
    let executable_regions = place_executable_regions(&image, layout)?;

    let mut bytes = Vec::new();
    write_macho_executable_header(&mut bytes, plan.command_count, plan.sizeofcmds);
    write_macho_pagezero_segment(&mut bytes);
    write_macho_executable_text_segment(
        &mut bytes,
        plan.text_offset,
        image.memory.text.len(),
        plan.text_file_size,
    );
    if plan.has_data_segment {
        write_macho_executable_data_segment(
            &mut bytes,
            plan.data_offset,
            image.memory.data.len(),
            image.memory.bss_size,
            plan.data_vm_size,
            image.memory.bss_alignment,
        );
    }
    write_macho_load_dylinker_command(&mut bytes);
    write_macho_uuid_command(&mut bytes);
    write_macho_executable_build_version_command(&mut bytes);
    write_macho_main_command(&mut bytes, entry_offset);
    // One LC_LOAD_DYLIB per linked dylib, in ordinal order (libSystem first).
    for dylib in &dylibs {
        write_macho_load_dylib_command(&mut bytes, dylib);
    }
    if plan.has_imports {
        write_macho_dyld_info_command(&mut bytes, plan.bind_offset, bind_info.len());
    }
    write_macho_linkedit_segment(
        &mut bytes,
        plan.linkedit_vmaddr,
        plan.bind_offset,
        plan.linkedit_filesize,
        plan.linkedit_vmsize,
    );
    write_empty_macho_symtab_command(&mut bytes);
    write_empty_macho_dysymtab_command(&mut bytes);
    write_macho_code_signature_command(
        &mut bytes,
        plan.code_signature_offset,
        plan.code_signature_size,
    );
    bytes.resize(plan.text_offset, 0);
    bytes.extend(&image.memory.text);
    if plan.has_data_segment {
        bytes.resize(plan.data_offset, 0);
        bytes.extend(&image.memory.data);
    }
    bytes.resize(plan.bind_offset, 0);
    bytes.extend(bind_info);
    bytes.resize(plan.code_signature_offset, 0);
    let code_signature = macho_ad_hoc_code_signature(&bytes, plan.text_file_size);
    debug_assert_eq!(code_signature.len(), plan.code_signature_size);
    bytes.extend(code_signature);

    Ok(ExecutableImageOutput {
        final_text_bytes: image.memory.text.clone(),
        bytes,
        file_name: "omega-program".to_owned(),
        format: "mach-o-arm64-executable".to_owned(),
        text_bytes: image.memory.text.len(),
        data_bytes: image.memory.data.len(),
        bss_bytes: image.memory.bss_size,
        symbols: image.symbol_table.symbols.len(),
        imports: image.symbol_table.imports.len(),
        relocations: image.relocation_table.relocations.len(),
        executable_regions,
    })
}
