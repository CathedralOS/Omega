//! A PIE, ad-hoc-signed, dyld-linked Mach-O arm64 MH_EXECUTE, bound eagerly at
//! load with no lazy stubs anywhere in it.
//!
//! The load base is 0x1_0000_0000 - four gigabytes of `__PAGEZERO` below
//! anything real, so a null dereference faults on an unmapped page rather than
//! reading the header. Pages are `MACHO_ARM64_PAGE_SIZE` = 0x4000, sixteen
//! kilobytes, and `CODE_SIGNATURE_PAGE_SIZE_POWER` is 14 because the code
//! directory stores that page size as a log2 exponent; the two constants must
//! move together or every signed page hashes at the wrong stride. Note this is
//! NOT the ADRP page: ADRP's page is four kilobytes and lives in `image`'s
//! relocation patcher. Two page sizes, adjacent in the same emitter, meaning
//! different things.
//!
//! `emit_macho_aarch64_executable` runs in an order most of which is forced:
//!
//! ```text
//!   install_import_thunks     preflights the dylib roster, THEN mutates .text
//!   macho_bind_info           the bind opcode stream for those thunks
//!   macho_rebase_info         the pointers dyld must slide for a PIE
//!   plan_macho_image          every offset, now that all four sizes are known
//!   patch_import_thunks / apply_aarch64_relocations
//!   validate_patched_preferred_pointers   rebased pointers still point right
//!   validate_import_thunk_footprints      thunk opcodes survived relocation
//!   ... write header, load commands, segments, linkedit ...
//!   macho_ad_hoc_code_signature           hashes the finished file, so LAST
//! ```
//!
//! The signature has to be last because it hashes every byte before it, and its
//! size has to be known before that because `LC_CODE_SIGNATURE` and the
//! `__LINKEDIT` extent are written earlier. That is why the blob length is
//! computed twice - see the @Cleanup below.
//!
//! Binding is eager and total. `write_macho_dyld_info_command` writes rebase and
//! bind offsets and then six zero words: weak bind offset and size, lazy bind
//! offset and size, export trie offset and size, all literally zero. The bind
//! stream is `0x51` (`SET_TYPE_IMM | BIND_TYPE_POINTER`) followed by `0x90`
//! (`DO_BIND`) per symbol. There is no `dyld_stub_binder` and no
//! `__la_symbol_ptr` section, because there is nothing left to resolve after
//! load.

//! Only imports an actual relocation points at get a thunk, a slot and a bind
//! entry; the rest of the host binding catalog is dropped. Emitting one per
//! catalog row is simpler and would make a program that touches only the
//! filesystem drag in libobjc, Foundation, AppKit and CoreGraphics as load-time
//! dependencies, because a bind entry naming a dylib is a reason to map it.
//!
//! The dylib roster is a `Vec` scanned linearly - `ensure_dylib` with `.any`,
//! ordinal assignment with `.position` - where a `HashMap` keyed by install name
//! is the obvious choice. Two things make the scan right. The roster is hard
//! capped at 15 entries, because the bind opcode carries the dylib ordinal in a
//! four-bit immediate, so the scan is bounded by the format itself. And the
//! ordinal IS the vector index plus one, with libSystem deliberately first at
//! ordinal 1 - an ordering a hash map would not preserve.
//!
//! `plan_dylibs` runs to completion before the first `image.memory.text.extend`,
//! and that is a transaction boundary rather than an accident of reading order.
//! A rejection discovered midway through installation would otherwise leave the
//! `FinalImage` with text extended, symbols rewritten and executable regions
//! pushed - a half-mutated value the caller has no way to roll back.

//! `image` supplies `FinalImage`, `apply_aarch64_relocations` and
//! `place_executable_regions`. This is the only emitter in the tree that both
//! binds imports and signs its output.
//!
//! @Cleanup: the 88-byte code directory header size is written as a literal in
//! two places, `code_signature_size` and `macho_ad_hoc_code_signature`, which
//! independently recompute the same offset arithmetic over identifier length,
//! special slots and page count. Nothing derives 88 from the field widths it
//! stands for. The only check that the two agree is
//! `debug_assert_eq!(code_signature.len(), plan.code_signature_size)` in
//! `emit_macho_aarch64_executable` - a DEBUG assert, compiled out of a release
//! build, where a disagreement would instead ship an executable whose
//! `LC_CODE_SIGNATURE` length does not match its actual signature blob.
//!
//! @Note: "lazy binding" is the wrong term for what this emitter produces, and
//! it appears in at least two places that describe it -
//! `image-emission/src/final_image_validation.rs:214` and the terminal-Psi
//! wiki page. In Mach-O that phrase names a specific mechanism, `dyld_stub_binder`
//! resolving through `__la_symbol_ptr` on first call, and this emitter uses none
//! of it. A reader who takes the phrase literally goes looking for a section
//! that is not there.

use diagnostics::Diagnostic;
use image::{
    ExecutableImageOutput, FinalImage, apply_aarch64_relocations, place_executable_regions,
};

mod bytes;
mod code_signature;
mod constants;
mod entry;
mod imports;
mod layout;
mod load_commands;
mod plan;
mod rebases;

use code_signature::macho_ad_hoc_code_signature;
use entry::macho_entry_text_offset;
use imports::{
    install_import_thunks, macho_bind_info, patch_import_thunks, validate_import_thunk_footprints,
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
use rebases::macho_rebase_info;

pub fn emit_macho_aarch64_executable(
    mut image: FinalImage,
) -> Result<ExecutableImageOutput, Diagnostic> {
    let imports = install_import_thunks(&mut image)?;
    let import_thunks = imports.thunks;
    // The exact ordered set of dylibs and every image-local ordinal were
    // preflighted before import-thunk installation mutated the final image.
    let dylibs = imports.dylibs;
    let bind_info = macho_bind_info(&import_thunks);
    let rebase_info = macho_rebase_info(&image)?;
    let plan = plan_macho_image(
        &image,
        import_thunks.len(),
        rebase_info.bytes.len(),
        bind_info.len(),
        &dylibs,
    );
    let entry_offset = plan.text_offset + macho_entry_text_offset(&image)?;
    let layout = plan.final_image_layout();

    patch_import_thunks(&mut image, &layout, &import_thunks)?;
    apply_aarch64_relocations(&mut image, &layout, "Mach-O direct executable")?;
    rebase_info.validate_patched_preferred_pointers(&image, &layout)?;
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
    if plan.has_dyld_info {
        write_macho_dyld_info_command(
            &mut bytes,
            plan.rebase_offset,
            rebase_info.bytes.len(),
            plan.bind_offset,
            bind_info.len(),
        );
    }
    write_macho_linkedit_segment(
        &mut bytes,
        plan.linkedit_vmaddr,
        plan.linkedit_offset,
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
    bytes.resize(plan.rebase_offset, 0);
    bytes.extend(&rebase_info.bytes);
    bytes.resize(plan.bind_offset, 0);
    bytes.extend(bind_info);
    bytes.resize(plan.code_signature_offset, 0);
    let code_signature = macho_ad_hoc_code_signature(&bytes, plan.text_file_size);
    debug_assert_eq!(code_signature.len(), plan.code_signature_size);
    bytes.extend(code_signature);

    Ok(ExecutableImageOutput {
        final_image_layout: layout,
        final_text_bytes: image.memory.text.clone(),
        final_data_bytes: image.memory.data.clone(),
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
