//! Writers for the Mach-O header and load commands of a dyld-linked
//! executable. Each writer appends one command's bytes to the output; this
//! module does not choose the order, sizes or offsets.
//!
//! `emit_macho_executable_signed` in `lib.rs` calls the writers in file
//! order, with sizes and offsets from the image plan: the Mach-O header
//! (`header`); the `__PAGEZERO`, `__TEXT` and optional `__DATA` segments
//! with their sections (`segments`); `LC_LOAD_DYLINKER`, `LC_UUID` (written
//! by `header`), `LC_BUILD_VERSION`, `LC_MAIN` and one `LC_LOAD_DYLIB` per
//! linked dylib (`dynamic_linking`); then the dyld info command, written only
//! when the image has rebases or imports, the `__LINKEDIT` segment, empty
//! `LC_SYMTAB` and `LC_DYSYMTAB` commands and `LC_CODE_SIGNATURE`
//! (`linkedit`).
//!
//! `dynamic_linking` also defines `MachoDylib`, the install name and version
//! fields of one linked dylib, which the import roster (`imports`) and the
//! file layout plan (`file_layout::plan`) use.

mod dynamic_linking;
mod header;
mod linkedit;
mod segments;

pub(crate) use dynamic_linking::{
    MachoDylib, write_macho_executable_build_version_command, write_macho_load_dylib_command,
    write_macho_load_dylinker_command, write_macho_main_command,
};
pub(crate) use header::{write_macho_executable_header, write_macho_uuid_command};
pub(crate) use linkedit::{
    write_empty_macho_dysymtab_command, write_empty_macho_symtab_command,
    write_macho_code_signature_command, write_macho_dyld_info_command,
};
pub(crate) use segments::{
    write_macho_executable_data_segment, write_macho_executable_text_segment,
    write_macho_linkedit_segment, write_macho_pagezero_segment,
};
