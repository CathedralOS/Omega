//! A PIE, ad-hoc-signed, dyld-linked Mach-O MH_EXECUTE for the AArch64 and
//! x86-64 macOS ABIs, bound eagerly at load with no lazy stubs anywhere in it.
//!
//! The load base is 0x1_0000_0000 - four gigabytes of `__PAGEZERO` below
//! anything real, so a null dereference faults on an unmapped page rather than
//! reading the header. The loader page is per-ISA (`isa.rs`): 16 KiB on arm64,
//! 4 KiB on x86-64, and the code directory stores that same size as a log2
//! exponent; the two move together or every signed page hashes at the wrong
//! stride. Note this is NOT the ADRP page: ADRP's page is four kilobytes on
//! every build, and lives in `image`'s relocation patcher. Distinct page
//! sizes, adjacent in the same emitter, meaning different things.
//!
//! `emit_macho_aarch64_executable` / `emit_macho_x86_64_executable` run in an
//! order most of which is forced:
//!
//! ```text
//!   install_import_thunks     preflights the dylib roster, THEN mutates .text
//!   macho_bind_info_for_segment  the bind opcode stream for those thunks
//!   macho_rebase_info_for_segment the pointers dyld must slide for a PIE
//!   plan_macho_container      every offset, now that all four sizes are known
//!   patch_import_thunks / isa relocation application
//!   validate_patched_preferred_pointers   rebased pointers still point right
//!   validate_import_thunk_footprints      thunk opcodes survived relocation
//!   ... write header, load commands, segments, linkedit ...
//!   macho_ad_hoc_code_signature_for_container   hashes the finished file, so LAST
//!   validate_macho_aarch64_loader_mapping / loader_fixups
//!       read-only checks of the actual container and exact loader writes
//! ```
//!
//! The signature has to be last because it hashes every byte before it, and its
//! size has to be known before that because `LC_CODE_SIGNATURE` and the
//! `__LINKEDIT` extent are written earlier. One `CodeSignatureLayout` supplies
//! both: the plan asks it for the extent of the planned signature offset and
//! the writer fills the same offsets for the bytes it actually hashed, so the
//! recorded length and the emitted blob are one derivation rather than two that
//! a release build could let drift.
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

//! `image` supplies `FinalImage`, the per-ISA relocation applicators and
//! `place_executable_regions`. This is the only emitter in the tree that both
//! binds imports and signs its output.
//!
//! `code_signature.rs` derives the signature blob once: its CodeDirectory
//! prefix is summed from the field widths it stands for, and the extent
//! `LC_CODE_SIGNATURE` records is that same blob's own length, checked against
//! the finished container's `__LINKEDIT` extent and file end.
//!
//! `file_layout/` places every byte and `dyld_linking/` describes the file to
//! the loader; `entry_symbol.rs` and `code_signature.rs` stay at the root
//! beside the emitter.

pub(crate) use optimization_core::codec_bytes;

use diagnostics::Diagnostic;
use image::{
    ExecutableImageOutput, FinalImage, PlacedDataRegionInventory, place_data_regions,
    place_executable_regions,
};

mod shared_component;
pub use shared_component::{
    MachoComponentExport, MachoSharedComponentEmissionError, MachoSharedComponentImage,
    emit_macho_shared_component,
};
mod code_signature;
pub use code_signature::code_signature_identifier;
mod dyld_linking;
mod entry_symbol;
mod file_layout;
mod isa;
#[cfg(test)]
mod tests;

use dyld_linking::imports::{
    install_import_thunks, patch_import_thunks, validate_import_thunk_footprints,
};
pub use dyld_linking::imports::{
    validate_macho_aarch64_import_binding_pairing, validate_macho_x86_64_import_binding_pairing,
};
use dyld_linking::load_commands::{
    write_empty_macho_dysymtab_command, write_empty_macho_symtab_command,
    write_macho_code_signature_command, write_macho_dyld_info_command,
    write_macho_executable_build_version_command, write_macho_executable_data_segment,
    write_macho_executable_header, write_macho_executable_text_segment,
    write_macho_linkedit_segment, write_macho_load_dylib_command,
    write_macho_load_dylinker_command, write_macho_main_command, write_macho_pagezero_segment,
    write_macho_uuid_command,
};
pub use dyld_linking::loader_fixups::{
    MachoImportPointer, MachoRebasePointer, validate_macho_aarch64_loader_fixups,
    validate_macho_aarch64_object_fixups, validate_macho_x86_64_object_fixups,
};
pub use dyld_linking::loader_mapping::{
    validate_macho_aarch64_loader_mapping, validate_macho_x86_64_loader_mapping,
};
use entry_symbol::macho_entry_text_offset;
use isa::MachoIsa;

/// The executable leaf published today; it doubles as the ad-hoc signing
/// label when the build authored no application identifier
/// (wiki/spec/build/macos_application.md). The universal leaf is replaced when
/// the validated application name reaches image emission; an authored
/// identifier is bound through [`emit_macho_aarch64_executable_signed`]
/// instead.
const OMEGA_EXECUTABLE_LEAF: &str = "omega-program";

/// Emit with the console fallback: the executable leaf is the ad-hoc
/// CodeDirectory label because no authored application identifier was bound.
pub fn emit_macho_aarch64_executable(
    image: FinalImage,
) -> Result<ExecutableImageOutput, Diagnostic> {
    emit_macho_aarch64_executable_signed(image, OMEGA_EXECUTABLE_LEAF)
}

/// Emit the x86-64 executable with the same fallback signing label.
pub fn emit_macho_x86_64_executable(
    image: FinalImage,
) -> Result<ExecutableImageOutput, Diagnostic> {
    emit_macho_x86_64_executable_signed(image, OMEGA_EXECUTABLE_LEAF)
}

/// Emit one signed executable binding `code_signature_identifier` as the
/// CodeDirectory identity.
///
/// The identifier must be the build-validated authored application
/// identifier; it enters the planned `LC_CODE_SIGNATURE` extent before any
/// load command is written and is embedded verbatim in the CodeDirectory
/// after the finished bytes are hashed. Publication cannot substitute or
/// re-sign with a different identity.
pub fn emit_macho_aarch64_executable_signed(
    image: FinalImage,
    code_signature_identifier: &str,
) -> Result<ExecutableImageOutput, Diagnostic> {
    emit_macho_executable_signed(image, code_signature_identifier, MachoIsa::Aarch64)
}

/// Emit one signed x86-64 executable binding `code_signature_identifier` as
/// the CodeDirectory identity, under the same contract as the AArch64 emit.
pub fn emit_macho_x86_64_executable_signed(
    image: FinalImage,
    code_signature_identifier: &str,
) -> Result<ExecutableImageOutput, Diagnostic> {
    emit_macho_executable_signed(image, code_signature_identifier, MachoIsa::X86_64)
}

fn emit_macho_executable_signed(
    image: FinalImage,
    code_signature_identifier: &str,
    isa: MachoIsa,
) -> Result<ExecutableImageOutput, Diagnostic> {
    let MachoContainerContents {
        image,
        bytes,
        layout,
        executable_regions,
        data_regions,
    } = emit_macho_container_contents(image, code_signature_identifier, isa, None, &[])?;
    Ok(ExecutableImageOutput {
        final_image_layout: layout,
        final_text_bytes: image.memory.text.clone(),
        final_data_bytes: image.memory.data.clone(),
        // Mach-O binding slots already live inside `image.memory.data`, so the
        // emitted file carries no separate import-data extent.
        final_import_data_bytes: Vec::new(),
        bytes,
        file_name: OMEGA_EXECUTABLE_LEAF.to_owned(),
        format: isa.format_name().to_owned(),
        text_bytes: image.memory.text.len(),
        data_bytes: image.memory.data.len(),
        bss_bytes: image.memory.bss_size,
        symbols: image.symbol_table.symbols.len(),
        imports: image.symbol_table.imports.len(),
        relocations: image.relocation_table.relocations.len(),
        executable_regions,
        data_regions,
        import_data_regions: PlacedDataRegionInventory::empty(),
    })
}
struct MachoContainerContents {
    image: FinalImage,
    bytes: Vec<u8>,
    layout: image::FinalImageLayout,
    executable_regions: image::PlacedExecutableRegionInventory,
    data_regions: image::PlacedDataRegionInventory,
}

fn emit_macho_container_contents(
    mut image: FinalImage,
    code_signature_identifier: &str,
    isa: MachoIsa,
    identity: Option<&dyld_linking::load_commands::MachoDylib>,
    exports: &[MachoComponentExport],
) -> Result<MachoContainerContents, Diagnostic> {
    if identity.is_some() != !exports.is_empty() {
        return Err(Diagnostic::error(
            "Mach-O container kind differs from its exact export roster",
        ));
    }
    if code_signature_identifier.is_empty() {
        return Err(Diagnostic::error(
            "Mach-O code signature identifier is empty",
        ));
    }
    if MachoIsa::from_format(image.target.object_format, image.target.architecture) != Some(isa) {
        return Err(Diagnostic::error(format!(
            "Mach-O {} emission cannot supply an image whose target is {:?}",
            isa.format_name(),
            image.target,
        )));
    }
    let imports = install_import_thunks(&mut image, isa)?;
    let import_thunks = imports.thunks;
    // The exact ordered set of dylibs and every image-local ordinal were
    // preflighted before import-thunk installation mutated the final image.
    let dylibs = imports.dylibs;
    let data_segment = file_layout::plan::macho_data_segment_index(&image, identity.is_some());
    let bind_info = dyld_linking::imports::macho_bind_info_for_segment(
        &import_thunks,
        data_segment.unwrap_or(0),
    )?;
    let rebase_info =
        dyld_linking::rebases::macho_rebase_info_for_segment(&image, data_segment.unwrap_or(0))?;
    let mut plan = file_layout::plan::plan_macho_container(
        &image,
        import_thunks.len(),
        rebase_info.bytes.len(),
        bind_info.len(),
        &dylibs,
        code_signature_identifier,
        isa,
        identity,
        0,
    );
    let export_bytes = shared_component::export_trie(&image, exports, plan.text_offset)?;
    if identity.is_some() {
        let text_offset = plan.text_offset;
        plan = file_layout::plan::plan_macho_container(
            &image,
            import_thunks.len(),
            rebase_info.bytes.len(),
            bind_info.len(),
            &dylibs,
            code_signature_identifier,
            isa,
            identity,
            export_bytes.len(),
        );
        if plan.text_offset != text_offset || plan.data_segment_index != data_segment {
            return Err(Diagnostic::error(
                "Mach-O export layout changed its admitted segment geometry",
            ));
        }
    }
    let entry_offset = if identity.is_none() {
        Some(plan.text_offset + macho_entry_text_offset(&image)?)
    } else {
        None
    };
    let layout = plan.final_image_layout();

    patch_import_thunks(&mut image, &layout, &import_thunks, isa)?;
    image::apply_relocations(&mut image, &layout, "Mach-O direct executable")?;
    let rebase_pointers = rebase_info.validate_patched_preferred_pointers(&image, &layout)?;
    validate_import_thunk_footprints(&mut image, &import_thunks, isa)?;
    let executable_regions = place_executable_regions(&image, layout)?;
    let data_regions = place_data_regions(&image, layout)?;

    let mut bytes = Vec::new();
    if identity.is_some() {
        dyld_linking::load_commands::write_macho_dylib_header(
            &mut bytes,
            plan.command_count,
            plan.sizeofcmds,
            isa,
        );
    } else {
        write_macho_executable_header(&mut bytes, plan.command_count, plan.sizeofcmds, isa);
        write_macho_pagezero_segment(&mut bytes);
    }
    write_macho_executable_text_segment(
        &mut bytes,
        plan.text_offset,
        image.memory.text.len(),
        plan.text_file_size,
        isa.page_size(),
    );
    if plan.has_data_segment {
        write_macho_executable_data_segment(
            &mut bytes,
            plan.data_offset,
            image.memory.data.len(),
            plan.bss_address,
            image.memory.bss_size,
            plan.data_vm_size,
            image.memory.bss_alignment,
        );
    }
    if identity.is_none() {
        write_macho_load_dylinker_command(&mut bytes);
    }
    write_macho_uuid_command(&mut bytes);
    write_macho_executable_build_version_command(&mut bytes);
    if let Some(entry_offset) = entry_offset {
        write_macho_main_command(&mut bytes, entry_offset);
    }
    if let Some(identity) = identity {
        dyld_linking::load_commands::write_macho_id_dylib_command(&mut bytes, identity);
    }
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
    if identity.is_some() {
        dyld_linking::load_commands::write_macho_exports_trie_command(
            &mut bytes,
            plan.export_offset,
            export_bytes.len(),
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
    bytes.resize(plan.export_offset, 0);
    bytes.extend(&export_bytes);
    bytes.resize(plan.code_signature_offset, 0);
    let code_signature = code_signature::macho_ad_hoc_code_signature_for_container(
        &bytes,
        plan.text_file_size,
        code_signature_identifier,
        isa,
        identity.is_none(),
    );
    // This blob and `plan.code_signature_size` come from one layout; the
    // container mapping check below pins the recorded extent to the file end.
    bytes.extend(code_signature);

    dyld_linking::loader_mapping::validate_macho_container_mapping(
        &bytes,
        layout,
        &image.memory.text,
        &image.memory.data,
        image.memory.bss_size,
        isa,
        identity,
    )?;
    let import_pointers: Vec<_> = import_thunks
        .iter()
        .map(|thunk| MachoImportPointer {
            data_offset: thunk.data_offset,
            install_name: &thunk.library,
            symbol: &thunk.bind_symbol,
        })
        .collect();
    dyld_linking::loader_fixups::validate_macho_loader_fixups_for_segment(
        &bytes,
        &image.memory.data,
        &rebase_pointers,
        &import_pointers,
        data_segment.unwrap_or(0),
    )?;
    if identity.is_some() {
        shared_component::validate_export_trie(&bytes, &image, exports, plan.text_offset)?;
    }

    Ok(MachoContainerContents {
        image,
        bytes,
        layout,
        executable_regions,
        data_regions,
    })
}
