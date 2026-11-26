/*
We emit 64-bit ELF static executables (`emit_elf_executable`), three-step
dynamically linked executables (`emit_elf_dynamic_executable`), and `ET_DYN`
position-independent shared components (`emit_elf_shared_component`) for Linux
x86-64 (`EM_X86_64 = 62`) and AArch64 (`EM_AARCH64 = 183`). Static executables
emit a compact 64-byte ELF header and two program headers (`PT_LOAD` for
`.text` and `.data`/`.bss` at `IMAGE_BASE = 0x400000`) with zero section
headers, refusing any image with referenced imports before mutating it.

Dynamically linked executables run a direct three-step pipeline:
1. `dynamic_layout`: preflight inputs, plan the 14 ELF64 sections and 5 program
   headers (`PT_INTERP`, 3 `PT_LOAD` segments, `PT_DYNAMIC`), and assign
   virtual addresses and file offsets under `ELF_MAX_PAGE_SIZE = 0x1_0000` so
   every binary loads on 4 KiB, 16 KiB, and 64 KiB page kernels.
   `AARCH64_RELOCATION_PAGE_SIZE` (`0x1000`) stays separate because ADRP page
   arithmetic always uses 4 KiB pages regardless of kernel page size.
2. `dynamic_relocation`: build dynamic symbol/string/hash/version tables and
   resolve image-local, `.plt`/`.got.plt`, `.rela.plt`/`.rela.dyn`, and
   `.dynamic` relocations against the planned addresses.
3. `dynamic_executable`: emit deterministic ELF64 bytes and wrap the admitted
   `FinalImage` and `ExecutableImageOutput` in `ValidatedElfDynamicExecutable`.
*/

/*
`image` supplies `FinalImage`, `apply_relocations`, and region placement;
`image-emission` invokes `emit_elf_executable`, `emit_elf_dynamic_executable`,
and `emit_elf_shared_component`. Keeping dynamic table construction in
`dynamic_tables` lets both dynamic executables and `ET_DYN` shared components
share one canonical encoder for `.dynstr`, `.dynsym`, `.hash`, and version
requirements.
*/

mod shared_component;
pub use shared_component::{
    ElfComponentExport, ElfSharedComponentEmissionError, ElfSharedComponentImage,
    emit_elf_shared_component,
};

mod bytes {
    use diagnostics::Diagnostic;
    pub(crate) use image::codec_bytes::{write_u16, write_u32, write_u64};

    pub(crate) fn name_at<'a>(
        bytes: &'a [u8],
        offset: u32,
        context: &'static str,
    ) -> Result<&'a [u8], Diagnostic> {
        let tail = bytes
            .get(offset as usize..)
            .ok_or_else(|| Diagnostic::error(format!("{context} is outside its name table")))?;
        let terminator = tail
            .iter()
            .position(|byte| *byte == 0)
            .ok_or_else(|| Diagnostic::error(format!("{context} is not NUL-terminated")))?;
        Ok(&tail[..terminator])
    }
}

mod constants {
    pub(crate) const ELF_HEADER_SIZE: usize = 64;
    pub(crate) const PROGRAM_HEADER_SIZE: usize = 56;
    pub(crate) const PROGRAM_HEADER_COUNT: usize = 2;
    pub(crate) const IMAGE_BASE: u64 = 0x400000;
    /// Linux arm64 kernels run with 4 KiB, 16 KiB or 64 KiB pages, and the
    /// x86-64 psABI allows pages up to 64 KiB, so a `PT_LOAD` segment that
    /// starts on a smaller boundary can share a page with its neighbour and
    /// the kernel refuses or overlaps the mapping.
    pub(crate) const ELF_MAX_PAGE_SIZE: u64 = 0x1_0000;

    pub(crate) const SECTION_COUNT: usize = 14;
    pub(crate) const ELF64_SECTION_HEADER_SIZE: usize = 64;
    pub(crate) const ELF64_DYNAMIC_ROW_SIZE: usize = 16;
    pub(crate) const ELF64_RELA_SIZE: usize = 24;
    pub(crate) const ELF64_SYMBOL_SIZE: usize = 24;
    pub(crate) const ELF64_GOT_WORD_SIZE: usize = 8;
    pub(crate) const GOT_PLT_HEADER_WORDS: usize = 3;
    pub(crate) const SHF_WRITE: u64 = 0x1;
    pub(crate) const SHF_ALLOC: u64 = 0x2;
    pub(crate) const SHF_EXECINSTR: u64 = 0x4;
}

mod dynamic_executable;
mod dynamic_layout;
mod dynamic_relocation;
mod dynamic_tables;
mod entry_symbol {
    use diagnostics::Diagnostic;
    use image::FinalImage;

    pub(crate) fn elf_entry_address(
        image: &FinalImage,
        text_address: u64,
    ) -> Result<u64, Diagnostic> {
        Ok(text_address + image::final_image_entry_symbol(image, "ELF")?.offset as u64)
    }
}

mod imports;
mod static_executable;

pub use dynamic_executable::{
    ElfDynamicExecutableEmissionError, ValidatedElfDynamicExecutable, emit_elf_dynamic_executable,
};
pub use static_executable::emit_elf_executable;

#[cfg(test)]
pub(crate) mod test_support {
    use crate::imports::{ElfImportRequest, canonical_referenced_imports};
    use image::FinalImage;
    use target::{NormalizedElfInterpreterPlan, TargetProfile, normalize_elf_interpreter_plan};

    pub(crate) fn interpreter(target: TargetProfile) -> NormalizedElfInterpreterPlan {
        let interpreter_path = match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-x86-64.so.2".as_slice(),
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-aarch64.so.1".as_slice(),
            _ => unreachable!(),
        };
        normalize_elf_interpreter_plan(interpreter_path.to_vec(), target).unwrap()
    }

    pub(crate) fn import_image(
        target: TargetProfile,
        bss_alignment: usize,
        symbol_capacity: usize,
    ) -> FinalImage {
        FinalImage::with_capacity(
            target.native_target(),
            image::FinalImageMemory {
                text: vec![0; 32],
                data: vec![0x5a; 13],
                bss_size: 23,
                bss_alignment,
            },
            arena::Handle::invalid(),
            symbol_capacity,
            1,
            1,
        )
    }

    pub(crate) fn text_entry(image: &mut FinalImage, offset: usize) {
        use image::{FinalImageSection, FinalImageSymbol};
        use object_file::SymbolKind;

        let entry = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "_start".to_owned(),
            section: FinalImageSection::Text,
            offset,
            size: 4,
            kind: SymbolKind::Function,
        });
        image.symbol_table.entry_symbol = entry;
    }

    pub(crate) fn relocation_image(target: target::NativeTarget, text_size: usize) -> FinalImage {
        FinalImage::with_capacity(
            target,
            image::FinalImageMemory {
                text: vec![0; text_size],
                ..image::FinalImageMemory::default()
            },
            arena::Handle::invalid(),
            1,
            1,
            2,
        )
    }

    pub(crate) fn import_with_text_relocations(
        image: &mut FinalImage,
        name: &str,
        import: image::FinalImageImportPlan,
        offsets: &[usize],
    ) -> image::FinalImageSymbolHandle {
        use image::{FinalImageImport, FinalImageRelocation, FinalImageSection, FinalImageSymbol};
        use object_file::{RelocationKind, SymbolKind};

        let imported = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: name.into(),
            section: FinalImageSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: imported,
            import,
        });
        image::test_support::install_imports(image);
        for &offset in offsets {
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Text,
                    offset,
                    byte_width: 4,
                    symbol_handle: imported,
                    addend: 0,
                    kind: RelocationKind::X86_64Relative32,
                });
        }
        imported
    }

    #[derive(Clone, Copy)]
    pub(crate) struct ImportFixture {
        pub(crate) object: &'static [u8],
        pub(crate) symbol: &'static [u8],
        pub(crate) version: &'static [u8],
        pub(crate) instruction_offsets: &'static [usize],
    }

    pub(crate) const IMPORTS: [ImportFixture; 3] = [
        ImportFixture {
            object: b"liba\xff.so",
            symbol: b"alpha\xfe",
            version: b"V1\xfd",
            instruction_offsets: &[0, 44],
        },
        ImportFixture {
            object: b"liba\xff.so",
            symbol: b"beta",
            version: b"V2",
            instruction_offsets: &[16],
        },
        ImportFixture {
            object: b"libb.so",
            symbol: b"gamma",
            version: b"V1\xfd",
            instruction_offsets: &[28],
        },
    ];

    pub(crate) const UTF8_IMPORTS: [ImportFixture; 3] = [
        ImportFixture {
            object: b"liba.so.1",
            symbol: b"alpha",
            version: b"V1",
            instruction_offsets: &[0, 44],
        },
        ImportFixture {
            object: b"liba.so.1",
            symbol: b"beta",
            version: b"V2",
            instruction_offsets: &[16],
        },
        ImportFixture {
            object: b"libb.so.2",
            symbol: b"gamma",
            version: b"V1",
            instruction_offsets: &[28],
        },
    ];

    pub(crate) fn dynamic_import_inputs(
        target: TargetProfile,
        imports: &[ImportFixture],
        symbol_prefix: &'static str,
        locator_expect: &'static str,
        interpreter_expect: &'static str,
    ) -> (
        FinalImage,
        NormalizedElfInterpreterPlan,
        Vec<ElfImportRequest>,
    ) {
        use arena::Handle;
        use image::FinalImageMemory;

        let relocation_count = imports
            .iter()
            .map(|fixture| fixture.instruction_offsets.len())
            .sum();
        let mut image = FinalImage::with_capacity(
            target.native_target(),
            FinalImageMemory {
                text: vec![0; 64],
                ..FinalImageMemory::default()
            },
            Handle::invalid(),
            imports.len(),
            imports.len(),
            relocation_count,
        );
        for (index, fixture) in imports.iter().enumerate() {
            let symbol_handle = versioned_import(
                &mut image,
                &format!("{symbol_prefix}{index}"),
                fixture.object,
                fixture.symbol,
                fixture.version,
                target,
                locator_expect,
            );
            insert_text_call_relocations(
                &mut image,
                target,
                fixture
                    .instruction_offsets
                    .iter()
                    .map(|offset| (*offset, symbol_handle)),
            );
        }
        let interpreter = normalize_elf_interpreter_plan(
            import_fixture_interpreter_path(target).to_vec(),
            target,
        )
        .expect(interpreter_expect);
        let requests = canonical_referenced_imports(&image).expect("valid dynamic-link inputs");
        (image, interpreter, requests)
    }

    pub(crate) fn insert_text_call_relocations(
        image: &mut image::FinalImage,
        target: TargetProfile,
        sites: impl IntoIterator<Item = (usize, image::FinalImageSymbolHandle)>,
    ) {
        use image::{FinalImageRelocation, FinalImageSection};
        use object_file::RelocationKind;
        for (instruction_offset, symbol_handle) in sites {
            let (relocation_offset, kind) = match target {
                TargetProfile::LinuxX64 => {
                    image.memory.text[instruction_offset] = 0xe8;
                    (instruction_offset + 1, RelocationKind::X86_64Relative32)
                }
                TargetProfile::LinuxArm64 => {
                    image.memory.text[instruction_offset..instruction_offset + 4]
                        .copy_from_slice(&[0, 0, 0, 0x94]);
                    (instruction_offset, RelocationKind::Aarch64Branch26)
                }
                _ => unreachable!(),
            };
            image
                .relocation_table
                .relocations
                .insert(FinalImageRelocation {
                    section: FinalImageSection::Text,
                    offset: relocation_offset,
                    byte_width: 4,
                    symbol_handle,
                    addend: 0,
                    kind,
                });
        }
    }

    pub(crate) fn versioned_import(
        image: &mut FinalImage,
        name: &str,
        object: &[u8],
        symbol: &[u8],
        version: &[u8],
        target: TargetProfile,
        locator_expect: &str,
    ) -> image::FinalImageSymbolHandle {
        use image::{FinalImageImport, FinalImageImportPlan, FinalImageSection, FinalImageSymbol};
        use object_file::SymbolKind;
        use target::{ForeignLocatorCandidate, normalize_foreign_locator};
        let imported = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: name.into(),
            section: FinalImageSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle: imported,
            import: FinalImageImportPlan::Normalized(
                normalize_foreign_locator(
                    ForeignLocatorCandidate::ElfVersioned {
                        object: object.to_vec(),
                        symbol: symbol.to_vec(),
                        version: version.to_vec(),
                    },
                    target,
                )
                .expect(locator_expect)
                .into(),
            ),
        });
        image::test_support::install_imports(image);
        imported
    }

    pub(crate) fn import_fixture_interpreter_path(target: TargetProfile) -> &'static [u8] {
        match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2",
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1",
            _ => unreachable!("import fixture uses a Linux target"),
        }
    }
}
