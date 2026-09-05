//! PE32+ executables for x86-64 Windows, in one function whose whole difficulty
//! is the order its nine steps have to run in.
//!
//! PE keeps two alignments at once, and every section carries both a virtual
//! size and a raw size because of it: `SECTION_ALIGNMENT` is 0x1000 in memory,
//! `FILE_ALIGNMENT` is 0x200 on disk. `IMAGE_BASE` is 0x1_4000_0000 and `.text`
//! always begins at RVA 0x1000. `.text` and `.rdata` are always emitted;
//! `.data`, `.reloc` and `.bss` appear only when non-empty, so `section_count`
//! is `2 + has_data + has_reloc + has_bss` and the header size depends on it.
//!
//! Section flags are written as raw COFF characteristics: `.text` 0x6000_0020,
//! `.rdata` 0x4000_0040, `.data` 0xc000_0040, `.bss` 0xc000_0080, and `.reloc`
//! 0x4200_0040 - initialized data, read, and DISCARDABLE, because the loader
//! consumes the fixups and then throws the section away.
//!
//! An import becomes a six-byte thunk appended to `.text`, `ff 25 00 00 00 00`,
//! which is `jmp [rip+disp32]` with the displacement filled in later. The
//! opcode is chosen for what it does not touch: control flow moves, and no
//! general-purpose register, no flag, no stack slot and no vector lane changes,
//! so inserting one cannot perturb the surrounding code's machine state.
//!
//! The step order in `emit_pe_x86_64_executable` is almost entirely forced:
//!
//! ```text
//!   1  install_import_thunks     appends to .text, so sizes are not known yet
//!   2  plan_pe_sections(.., 0)   a first pass, only to learn rdata_rva
//!   3  build_import_table        needs that rva to place the IAT
//!   4  plan_pe_sections(.., len) the real pass, now the rdata size is known
//!   5  build_base_relocations    BEFORE relocations are applied
//!   6  patch_import_thunks       fills the disp32 now that the IAT has an address
//!   7  apply_x86_64_relocations  mutates text and data
//!   8  validate_import_thunk_footprints   re-checks the ff 25 opcodes after 7
//!   9  place_executable_regions
//! ```
//!
//! Step 5 reads oddly and is right: `.reloc` lists the OFFSETS of relocation
//! sites, never their values, so it is identical before and after step 7 and
//! computing it first keeps the section base-independent. Step 8 exists because
//! step 7 writes into the same `.text` the thunks were appended to; a
//! relocation whose site overlapped a thunk would corrupt it silently, so the
//! opcodes are re-read after patching rather than assumed.

//! `plan_pe_sections` runs twice per emission and is not memoised, and the
//! honest reason is that the second call needs a number only the first call's
//! output can produce. Threading a partially built `PeSections` through
//! `build_import_table` would let it observe fields that are still wrong at that
//! point - every raw offset after `.rdata` shifts once the import table has a
//! size - so the layout is recomputed from scratch instead of patched.
//!
//! It costs more than it looks. `plan_pe_sections` itself calls
//! `build_base_relocations` purely to measure the section's length, so the
//! relocation bytes are built three times per executable: once inside each
//! planning pass, and once for real at step 5. For the image sizes this
//! compiler produces that is not worth a cache; if `.reloc` ever gets large it
//! is the first thing to look at.

//! `image` supplies `FinalImage`, `apply_x86_64_relocations` and
//! `place_executable_regions`. The Windows subsystem value is a parameter rather
//! than a constant here, so the console and GUI variants share every byte of
//! this path except that one `u16`.
//!
//! @Note: this crate has no `.idata` section. The import directory and IAT are
//! written into `.rdata` and named through the PE data directories instead,
//! which is what modern linkers do and what the `import_directory_rva` /
//! `iat_rva` fields of `PeHeaderInput` carry. Looking for `.idata` here and not
//! finding it does not mean imports are unimplemented.

use diagnostics::Diagnostic;
use image::{
    ExecutableImageOutput, FinalImage, apply_x86_64_relocations, place_executable_regions,
};

mod bytes;
mod constants;
mod entry;
mod headers;
mod imports;
mod layout;
mod relocations;
mod sections;

use constants::TEXT_RVA;
use entry::pe_entry_rva;
use headers::{PeHeaderInput, write_dos_header, write_pe_headers, write_section_header};
use imports::{
    build_import_table, install_import_thunks, patch_import_thunks,
    validate_import_thunk_footprints,
};
use sections::plan_pe_sections;

pub fn emit_pe_x86_64_executable(
    mut image: FinalImage,
    subsystem: u16,
) -> Result<ExecutableImageOutput, Diagnostic> {
    let import_thunks = install_import_thunks(&mut image, subsystem)?;
    let initial_sections = plan_pe_sections(&image, 0);
    let import_table = build_import_table(&import_thunks, initial_sections.rdata_rva);
    let sections = plan_pe_sections(&image, import_table.bytes.len());
    let layout = sections.final_image_layout();

    // The `.reloc` bytes are built from the relocation table BEFORE
    // relocations are applied (the sites' offsets, not their values, are what
    // it lists), so the section is base-independent.
    let base_relocations = relocations::build_base_relocations(&image, TEXT_RVA, sections.data_rva);

    patch_import_thunks(&mut image, &layout, &import_thunks, &import_table.iat_rvas)?;
    apply_x86_64_relocations(&mut image, &layout, "PE direct executable")?;
    validate_import_thunk_footprints(&mut image, &import_thunks)?;
    let executable_regions = place_executable_regions(&image, layout)?;

    let entry_rva = pe_entry_rva(&image)?;

    let (reloc_directory_rva, reloc_directory_size) = if sections.has_reloc {
        (sections.reloc_rva, sections.reloc_virtual_size)
    } else {
        (0, 0)
    };

    let mut bytes = Vec::new();
    write_dos_header(&mut bytes);
    write_pe_headers(
        &mut bytes,
        PeHeaderInput {
            section_count: sections.section_count,
            entry_rva,
            size_of_code: sections.text_raw_size,
            size_of_initialized_data: sections.rdata_raw_size + sections.data_raw_size,
            size_of_image: sections.size_of_image,
            size_of_headers: sections.headers_size,
            import_directory_rva: import_table.import_directory_rva,
            import_directory_size: import_table.import_directory_size,
            iat_rva: import_table.iat_rva,
            iat_size: import_table.iat_size,
            reloc_directory_rva,
            reloc_directory_size,
            has_reloc: sections.has_reloc,
            subsystem,
        },
    );
    write_section_header(
        &mut bytes,
        ".text",
        sections.text_virtual_size,
        TEXT_RVA,
        sections.text_raw_size,
        sections.text_raw,
        0x6000_0020,
    );
    write_section_header(
        &mut bytes,
        ".rdata",
        sections.rdata_virtual_size,
        sections.rdata_rva,
        sections.rdata_raw_size,
        sections.rdata_raw,
        0x4000_0040,
    );
    if sections.has_data {
        write_section_header(
            &mut bytes,
            ".data",
            image.memory.data.len(),
            sections.data_rva,
            sections.data_raw_size,
            sections.data_raw,
            0xc000_0040,
        );
    }
    if sections.has_reloc {
        // INITIALIZED_DATA | DISCARDABLE | READ (the loader consumes `.reloc`
        // then discards it).
        write_section_header(
            &mut bytes,
            ".reloc",
            sections.reloc_virtual_size,
            sections.reloc_rva,
            sections.reloc_raw_size,
            sections.reloc_raw,
            0x4200_0040,
        );
    }
    if sections.has_bss {
        write_section_header(
            &mut bytes,
            ".bss",
            image.memory.bss_size,
            sections.bss_rva,
            0,
            0,
            0xc000_0080,
        );
    }

    bytes.resize(sections.text_raw, 0);
    bytes.extend(&image.memory.text);
    bytes.resize(sections.text_raw + sections.text_raw_size, 0);
    bytes.resize(sections.rdata_raw, 0);
    bytes.extend(&import_table.bytes);
    bytes.resize(sections.rdata_raw + sections.rdata_raw_size, 0);
    if sections.has_data {
        bytes.resize(sections.data_raw, 0);
        bytes.extend(&image.memory.data);
        bytes.resize(sections.data_raw + sections.data_raw_size, 0);
    }
    if sections.has_reloc {
        bytes.resize(sections.reloc_raw, 0);
        bytes.extend(&base_relocations.bytes);
        bytes.resize(sections.reloc_raw + sections.reloc_raw_size, 0);
    }

    Ok(ExecutableImageOutput {
        final_image_layout: layout,
        final_text_bytes: image.memory.text.clone(),
        final_data_bytes: image.memory.data.clone(),
        bytes,
        file_name: "omega-program.exe".to_owned(),
        format: "pe64-x86_64-executable".to_owned(),
        text_bytes: image.memory.text.len(),
        data_bytes: image.memory.data.len(),
        bss_bytes: image.memory.bss_size,
        symbols: image.symbol_table.symbols.len(),
        imports: image.symbol_table.imports.len(),
        relocations: image.relocation_table.relocations.len(),
        executable_regions,
    })
}
