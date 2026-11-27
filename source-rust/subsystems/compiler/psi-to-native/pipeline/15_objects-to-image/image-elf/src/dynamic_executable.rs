/*
Step 3 of the dynamic ELF pipeline: assemble deterministic ELF64 executable bytes
from the Step 1 layout plan and Step 2 resolved sections/relocations, wrap the
result in `ValidatedElfDynamicExecutable`, and expose `emit_elf_dynamic_executable`.
*/

use crate::bytes::{write_u16, write_u32, write_u64};
use crate::constants::{
    ELF_HEADER_SIZE, ELF64_SECTION_HEADER_SIZE, PROGRAM_HEADER_SIZE, SECTION_COUNT,
};
use crate::dynamic_layout::{
    COMPLETE_SECTION_NAME_TABLE, DYNAMIC_PROGRAM_HEADER_COUNT, DynamicLayoutPlan,
    DynamicSectionIndex, SECTION_NAME_TABLE_INDEX, plan_dynamic_layout, require,
};
use crate::dynamic_relocation::{ResolvedDynamicRelocations, resolve_dynamic_relocations};
use diagnostics::Diagnostic;
use image::{ExecutableImageOutput, FinalImage, place_data_extent, place_executable_extent};
use target::{NormalizedElfInterpreterPlan, TargetProfile};

const ET_EXEC: u16 = 2;
const EM_X86_64: u16 = 62;
const EM_AARCH64: u16 = 183;
const EV_CURRENT: u32 = 1;

#[derive(Debug)]
#[must_use = "admitted dynamic ELF retains exact final-image and byte custody"]
pub struct ValidatedElfDynamicExecutable {
    image: FinalImage,
    output: ExecutableImageOutput,
}

impl ValidatedElfDynamicExecutable {
    pub const fn image(&self) -> &FinalImage {
        &self.image
    }

    pub const fn output(&self) -> &ExecutableImageOutput {
        &self.output
    }

    pub fn into_parts(self) -> (FinalImage, ExecutableImageOutput) {
        (self.image, self.output)
    }
}

#[derive(Debug)]
#[must_use = "dynamic ELF emission failure retains the exact failing owner"]
pub enum ElfDynamicExecutableEmissionError {
    LayoutPlanning {
        image: FinalImage,
        interpreter: NormalizedElfInterpreterPlan,
        diagnostic: Diagnostic,
    },
    RelocationResolution {
        image: FinalImage,
        interpreter: NormalizedElfInterpreterPlan,
        diagnostic: Diagnostic,
    },
    ByteEmission {
        image: FinalImage,
        interpreter: NormalizedElfInterpreterPlan,
        diagnostic: Diagnostic,
    },
}

impl ElfDynamicExecutableEmissionError {
    pub const fn stage(&self) -> &'static str {
        match self {
            Self::LayoutPlanning { .. } => "layout-planning",
            Self::RelocationResolution { .. } => "relocation-resolution",
            Self::ByteEmission { .. } => "byte-emission",
        }
    }

    pub const fn diagnostic(&self) -> &Diagnostic {
        match self {
            Self::LayoutPlanning { diagnostic, .. }
            | Self::RelocationResolution { diagnostic, .. }
            | Self::ByteEmission { diagnostic, .. } => diagnostic,
        }
    }

    pub fn into_parts(self) -> (FinalImage, NormalizedElfInterpreterPlan, Diagnostic) {
        match self {
            Self::LayoutPlanning {
                image,
                interpreter,
                diagnostic,
            }
            | Self::RelocationResolution {
                image,
                interpreter,
                diagnostic,
            }
            | Self::ByteEmission {
                image,
                interpreter,
                diagnostic,
            } => (image, interpreter, diagnostic),
        }
    }
}

impl std::fmt::Display for ElfDynamicExecutableEmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "dynamic ELF {}: {}",
            self.stage(),
            self.diagnostic()
        )
    }
}

impl std::error::Error for ElfDynamicExecutableEmissionError {}

/// Emit a dynamically linked ELF64 executable using the three-step direct builder:
/// 1. Plan sections, program headers, and virtual/file layout (`plan_dynamic_layout`).
/// 2. Build dynamic tables and resolve image-local, GOT/PLT, and RELA relocations
///    (`resolve_dynamic_relocations`).
/// 3. Emit deterministic ELF64 bytes (`emit_dynamic_bytes`).
pub fn emit_elf_dynamic_executable(
    mut image: FinalImage,
    interpreter: NormalizedElfInterpreterPlan,
) -> Result<ValidatedElfDynamicExecutable, Box<ElfDynamicExecutableEmissionError>> {
    let plan = match plan_dynamic_layout(&image, &interpreter) {
        Ok(plan) => plan,
        Err(diagnostic) => {
            return Err(Box::new(
                ElfDynamicExecutableEmissionError::LayoutPlanning {
                    image,
                    interpreter,
                    diagnostic,
                },
            ));
        }
    };

    let resolved = match resolve_dynamic_relocations(&image, &plan) {
        Ok(resolved) => resolved,
        Err(diagnostic) => {
            return Err(Box::new(
                ElfDynamicExecutableEmissionError::RelocationResolution {
                    image,
                    interpreter,
                    diagnostic,
                },
            ));
        }
    };

    let output = match emit_dynamic_bytes(&image, &plan, resolved) {
        Ok(output) => output,
        Err(diagnostic) => {
            return Err(Box::new(ElfDynamicExecutableEmissionError::ByteEmission {
                image,
                interpreter,
                diagnostic,
            }));
        }
    };

    image.memory.text = output.final_text_bytes.clone();
    image.memory.data = output.final_data_bytes.clone();
    Ok(ValidatedElfDynamicExecutable { image, output })
}

fn emit_dynamic_bytes(
    image: &FinalImage,
    plan: &DynamicLayoutPlan,
    resolved: ResolvedDynamicRelocations,
) -> Result<ExecutableImageOutput, Diagnostic> {
    let (machine, format) = match plan.target {
        TargetProfile::LinuxX64 => (EM_X86_64, "elf64-x86-64-dynamic-executable"),
        TargetProfile::LinuxArm64 => (EM_AARCH64, "elf64-aarch64-dynamic-executable"),
        _ => {
            return Err(Diagnostic::error(
                "dynamic ELF emission requires an exact Linux x86-64 or AArch64 target",
            ));
        }
    };

    let executable_regions = place_executable_extent(
        &resolved.text_bytes,
        image.executable_regions.clone(),
        plan.image_layout.text_address,
    )?;
    let data_regions = place_data_extent(
        &resolved.data_bytes,
        image.data_regions.clone(),
        plan.image_layout.data_address,
        "initialized-data",
    )?;

    let mut bytes = vec![0u8; plan.total_file_size];
    // Header, text, data, each section and the section header table.
    let mut fragments = Vec::with_capacity(plan.sections.len() + 4);

    let mut header_prefix =
        Vec::with_capacity(ELF_HEADER_SIZE + PROGRAM_HEADER_SIZE * DYNAMIC_PROGRAM_HEADER_COUNT);
    header_prefix.extend([0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0]);
    header_prefix.extend([0; 7]);
    write_u16(&mut header_prefix, ET_EXEC);
    write_u16(&mut header_prefix, machine);
    write_u32(&mut header_prefix, EV_CURRENT);
    write_u64(&mut header_prefix, plan.entry_address);
    write_u64(&mut header_prefix, ELF_HEADER_SIZE as u64);
    write_u64(&mut header_prefix, plan.section_header_table_file_offset);
    write_u32(&mut header_prefix, 0);
    write_u16(&mut header_prefix, ELF_HEADER_SIZE as u16);
    write_u16(&mut header_prefix, PROGRAM_HEADER_SIZE as u16);
    write_u16(&mut header_prefix, DYNAMIC_PROGRAM_HEADER_COUNT as u16);
    write_u16(&mut header_prefix, ELF64_SECTION_HEADER_SIZE as u16);
    write_u16(&mut header_prefix, SECTION_COUNT as u16);
    write_u16(&mut header_prefix, SECTION_NAME_TABLE_INDEX);
    for phdr in &plan.program_headers {
        write_u32(&mut header_prefix, phdr.segment_type);
        write_u32(&mut header_prefix, phdr.flags);
        write_u64(&mut header_prefix, phdr.file_offset);
        write_u64(&mut header_prefix, phdr.virtual_address);
        write_u64(&mut header_prefix, phdr.physical_address);
        write_u64(&mut header_prefix, phdr.file_size);
        write_u64(&mut header_prefix, phdr.memory_size);
        write_u64(&mut header_prefix, phdr.alignment);
    }
    write_fragment(&mut bytes, &mut fragments, 0, &header_prefix)?;

    write_fragment(
        &mut bytes,
        &mut fragments,
        plan.text_file_offset,
        &resolved.text_bytes,
    )?;
    write_fragment(
        &mut bytes,
        &mut fragments,
        plan.data_file_offset,
        &resolved.data_bytes,
    )?;

    for section in &plan.sections {
        if section.index == 0 {
            continue;
        }
        let payload: &[u8] = match section.index {
            x if x == DynamicSectionIndex::Interpreter as u32 => &plan.tables.interpreter,
            x if x == DynamicSectionIndex::DynamicString as u32 => &plan.tables.dynstr,
            x if x == DynamicSectionIndex::DynamicSymbol as u32 => &plan.tables.dynsym,
            x if x == DynamicSectionIndex::SystemVHash as u32 => &plan.tables.sysv_hash,
            x if x == DynamicSectionIndex::GnuSymbolVersion as u32 => &plan.tables.versym,
            x if x == DynamicSectionIndex::GnuVersionRequirement as u32 => &plan.tables.verneed,
            x if x == DynamicSectionIndex::GnuHash as u32 => &plan.tables.gnu_hash,
            x if x == DynamicSectionIndex::ProcedureLinkage as u32 => &resolved.plt_bytes,
            x if x == DynamicSectionIndex::ProcedureGot as u32 => &resolved.got_plt_bytes,
            x if x == DynamicSectionIndex::ProcedureRelocation as u32 => &resolved.rela_plt_bytes,
            x if x == DynamicSectionIndex::GeneralRelocation as u32 => &resolved.rela_dyn_bytes,
            x if x == DynamicSectionIndex::DynamicTable as u32 => &resolved.dynamic_bytes,
            x if x == DynamicSectionIndex::SectionNameTable as u32 => COMPLETE_SECTION_NAME_TABLE,
            _ => unreachable!("closed 14-section ELF roster"),
        };
        require(
            payload.len() as u64 == section.byte_size,
            "dynamic ELF section payload size drifted from layout plan",
        )?;
        write_fragment(&mut bytes, &mut fragments, section.file_offset, payload)?;
    }

    let mut shdr_table = Vec::with_capacity(SECTION_COUNT * ELF64_SECTION_HEADER_SIZE);
    for section in &plan.sections {
        write_u32(&mut shdr_table, section.name_offset);
        write_u32(&mut shdr_table, section.section_type);
        write_u64(&mut shdr_table, section.flags);
        write_u64(&mut shdr_table, section.virtual_address);
        write_u64(&mut shdr_table, section.file_offset);
        write_u64(&mut shdr_table, section.byte_size);
        write_u32(&mut shdr_table, section.link);
        write_u32(&mut shdr_table, section.info);
        write_u64(&mut shdr_table, section.alignment);
        write_u64(&mut shdr_table, section.entry_size);
    }
    write_fragment(
        &mut bytes,
        &mut fragments,
        plan.section_header_table_file_offset,
        &shdr_table,
    )?;
    require_disjoint_fragments(&mut fragments)?;

    Ok(ExecutableImageOutput {
        final_image_layout: plan.image_layout,
        final_text_bytes: resolved.text_bytes,
        final_data_bytes: resolved.data_bytes,
        final_import_data_bytes: resolved.got_plt_bytes,
        bytes,
        file_name: "omega-program".to_owned(),
        format: format.to_owned(),
        text_bytes: image.memory.text.len(),
        data_bytes: image.memory.data.len(),
        bss_bytes: image.memory.bss_size,
        symbols: image.symbol_table.symbols.len(),
        imports: image.symbol_table.imports.len(),
        relocations: image.relocation_table.relocations.len(),
        executable_regions,
        data_regions,
        import_data_regions: resolved.import_data_regions,
    })
}

/// Copy one fragment into the file and record its file range for the overlap
/// check. An empty fragment occupies no bytes, so it records nothing.
fn write_fragment(
    file: &mut [u8],
    fragments: &mut Vec<std::ops::Range<usize>>,
    file_offset: u64,
    bytes: &[u8],
) -> Result<(), Diagnostic> {
    let start = usize::try_from(file_offset)
        .map_err(|_| Diagnostic::error("dynamic ELF file offset exceeds usize"))?;
    let end = start
        .checked_add(bytes.len())
        .ok_or_else(|| Diagnostic::error("dynamic ELF fragment end overflows usize"))?;
    file.get_mut(start..end)
        .ok_or_else(|| Diagnostic::error("dynamic ELF fragment exceeds allocated file"))?
        .copy_from_slice(bytes);
    if start < end {
        fragments.push(start..end);
    }
    Ok(())
}

/// Refuse the file when two written fragments share a byte. After sorting by
/// start, any overlap also shows between two neighbors, so the check costs one
/// sort over the fragments rather than a flag per file byte.
fn require_disjoint_fragments(fragments: &mut [std::ops::Range<usize>]) -> Result<(), Diagnostic> {
    fragments.sort_unstable_by_key(|fragment| fragment.start);
    require(
        fragments
            .windows(2)
            .all(|pair| pair[0].end <= pair[1].start),
        "dynamic ELF file fragments overlap",
    )
}

#[cfg(test)]
mod tests {
    use super::emit_elf_dynamic_executable;
    use crate::constants::ELF_MAX_PAGE_SIZE;
    use crate::test_support::{
        IMPORTS, UTF8_IMPORTS, dynamic_import_inputs, import_fixture_interpreter_path,
        import_with_text_relocations, interpreter, text_entry,
    };
    use arena::Handle;
    use image::{
        FinalImage, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
        FinalImageSection, FinalImageSymbol,
    };
    use object_file::{RelocationKind, SymbolKind};
    use target::{TargetProfile, normalize_elf_interpreter_plan};

    const PRIVATE_CALLBACK_OFFSET: usize = 40;
    const CALLBACK_MATERIALIZATION_OFFSET: usize = 8;
    const CALLBACK_POINTER_DATA_OFFSET: usize = 8;

    fn read_u16(bytes: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
    }

    fn read_u32(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
    }

    fn read_u64(bytes: &[u8], offset: usize) -> u64 {
        u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
    }

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
        text_entry(&mut image, 24);
        let private_callback = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "__omega_private_callback".to_owned(),
            section: FinalImageSection::Text,
            offset: PRIVATE_CALLBACK_OFFSET,
            size: 8,
            kind: SymbolKind::Function,
        });
        let import = crate::test_support::versioned_import(
            &mut image,
            "__omega_callback_registrar",
            b"libcallback-registrar.so",
            b"register_callback",
            b"CALLBACK_1",
            target,
            "versioned import locator",
        );

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
                image.memory.text[0] = 0xe8;
                image.memory.text
                    [CALLBACK_MATERIALIZATION_OFFSET..CALLBACK_MATERIALIZATION_OFFSET + 3]
                    .copy_from_slice(&[0x48, 0x8d, 0x05]);
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
                image.memory.text[0..4].copy_from_slice(&[0, 0, 0, 0x94]);
                image.memory.text
                    [CALLBACK_MATERIALIZATION_OFFSET..CALLBACK_MATERIALIZATION_OFFSET + 8]
                    .copy_from_slice(&[0, 0, 0, 0x90, 0, 0, 0, 0x91]);
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
        image
    }

    fn file_offset_of(file: &[u8], address: u64) -> usize {
        const PT_LOAD: u32 = 1;
        let phoff = read_u64(file, 32) as usize;
        let phentsize = usize::from(read_u16(file, 54));
        let phnum = usize::from(read_u16(file, 56));
        for index in 0..phnum {
            let header = phoff + index * phentsize;
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

    fn materialized_callback_address(target: TargetProfile, text: &[u8], text_address: u64) -> u64 {
        let instruction_address = text_address + CALLBACK_MATERIALIZATION_OFFSET as u64;
        match target {
            TargetProfile::LinuxX64 => {
                let displacement =
                    read_u32(text, CALLBACK_MATERIALIZATION_OFFSET + 3).cast_signed();
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
                private_callback
            );
            assert_eq!(
                read_u64(&output.final_data_bytes, CALLBACK_POINTER_DATA_OFFSET),
                private_callback
            );
            let text_file_offset = file_offset_of(&output.bytes, text_address);
            assert_eq!(
                &output.bytes[text_file_offset..text_file_offset + 64],
                output.final_text_bytes.as_slice()
            );
            let data_file_offset = file_offset_of(&output.bytes, data_address);
            assert_eq!(
                read_u64(
                    &output.bytes,
                    data_file_offset + CALLBACK_POINTER_DATA_OFFSET
                ),
                private_callback
            );
            assert_eq!(admitted.image().memory.text, output.final_text_bytes);
            assert_eq!(admitted.image().memory.data, output.final_data_bytes);
        }
    }

    #[test]
    fn both_linux_targets_emit_deterministic_elf64_headers_sections_and_dynamic_tags() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let (mut image, _, _) =
                dynamic_import_inputs(target, &UTF8_IMPORTS, "__omega_e2e_", "valid", "valid");
            text_entry(&mut image, 0);
            let interp = normalize_elf_interpreter_plan(
                import_fixture_interpreter_path(target).to_vec(),
                target,
            )
            .unwrap();
            let first = emit_elf_dynamic_executable(image.clone(), interp.clone()).unwrap();
            let second = emit_elf_dynamic_executable(image, interp).unwrap();
            assert_eq!(first.output().bytes, second.output().bytes);

            let bytes = &first.output().bytes;
            assert!(bytes.starts_with(b"\x7fELF\x02\x01\x01"));
            assert_eq!(read_u16(bytes, 16), 2);
            assert_eq!(
                read_u16(bytes, 18),
                match target {
                    TargetProfile::LinuxX64 => 62,
                    TargetProfile::LinuxArm64 => 183,
                    _ => unreachable!(),
                }
            );
            assert_eq!(read_u16(bytes, 56), 5);
            assert_eq!(read_u16(bytes, 60), 14);
            assert_eq!(read_u16(bytes, 62), 13);

            // Check that every PT_LOAD segment honours 64 KiB page alignment.
            for index in 1..=3 {
                let ph = 64 + index * 56;
                assert_eq!(read_u32(bytes, ph), 1);
                assert_eq!(read_u64(bytes, ph + 48), ELF_MAX_PAGE_SIZE);
            }
            assert_eq!(
                first.output().import_data_regions.regions.len(),
                UTF8_IMPORTS.len()
            );
        }
    }

    #[test]
    fn non_utf8_import_symbol_rejects_with_custody_in_relocation_resolution() {
        let target = TargetProfile::LinuxX64;
        let (mut image, interp, _) =
            dynamic_import_inputs(target, &IMPORTS, "__omega_raw_", "valid", "valid");
        text_entry(&mut image, 0);
        let err = emit_elf_dynamic_executable(image, interp)
            .expect_err("non-UTF-8 symbol in GOT region custody must reject");
        assert_eq!(err.stage(), "relocation-resolution");
        assert!(err.diagnostic().message.contains("UTF-8"));
    }

    #[test]
    fn mixed_versioned_and_component_only_dynamic_executables_emit_cleanly() {
        let target = TargetProfile::LinuxX64;
        let mut image = crate::test_support::import_image(target, 8, 2);
        text_entry(&mut image, 0);
        let locator =
            target::NormalizedComponentLocator::for_image(target, b"component bytes", [9; 32])
                .unwrap();
        image.memory.text[16] = 0xe8;
        import_with_text_relocations(
            &mut image,
            "__omega_comp_only",
            FinalImageImportPlan::Normalized(locator.into()),
            &[17],
        );
        let interp = interpreter(target);
        let emitted = emit_elf_dynamic_executable(image, interp)
            .expect("component-only dynamic ELF executable emits");
        let bytes = &emitted.output().bytes;
        // Locate PT_DYNAMIC and verify DT_VERSYM / DT_VERNEED are omitted and DT_FLAGS=DF_BIND_NOW is present.
        let dyn_ph = 64 + 4 * 56;
        assert_eq!(read_u32(bytes, dyn_ph), 2);
        let dyn_off = read_u64(bytes, dyn_ph + 8) as usize;
        let dyn_size = read_u64(bytes, dyn_ph + 32) as usize;
        let mut tags = Vec::new();
        for offset in (dyn_off..dyn_off + dyn_size).step_by(16) {
            tags.push((
                i64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap()),
                read_u64(bytes, offset + 8),
            ));
        }
        assert!(
            tags.contains(&(30, 8)),
            "DT_FLAGS = DF_BIND_NOW must be present"
        );
        assert!(
            !tags
                .iter()
                .any(|(tag, _)| matches!(tag, 0x6fff_fff0 | 0x6fff_fffe | 0x6fff_ffff))
        );
    }

    #[test]
    fn rejection_retains_unmodified_image_and_interpreter_custody() {
        let target = TargetProfile::LinuxX64;
        let empty = FinalImage::with_capacity(
            target.native_target(),
            FinalImageMemory::default(),
            Handle::invalid(),
            0,
            0,
            0,
        );
        let interp = interpreter(target);
        let err = emit_elf_dynamic_executable(empty.clone(), interp.clone())
            .expect_err("no-import image must reject");
        assert_eq!(err.stage(), "layout-planning");
        let (returned_image, returned_interp, _) = err.into_parts();
        assert_eq!(returned_image, empty);
        assert_eq!(returned_interp, interp);

        let wrong_interp = interpreter(TargetProfile::LinuxArm64);
        let err = emit_elf_dynamic_executable(callback_image(target), wrong_interp.clone())
            .expect_err("target mismatch must reject");
        assert_eq!(err.stage(), "layout-planning");
        let (_, returned_interp, _) = err.into_parts();
        assert_eq!(returned_interp, wrong_interp);
    }

    #[test]
    fn file_fragments_may_abut_but_never_share_a_byte() {
        use super::require_disjoint_fragments;
        assert!(require_disjoint_fragments(&mut [64..128, 0..64, 128..130]).is_ok());
        let overlap = require_disjoint_fragments(&mut [64..128, 0..65]).unwrap_err();
        assert!(overlap.message.contains("fragments overlap"));
        // A fragment inside a wider one overlaps even with nothing between them.
        assert!(require_disjoint_fragments(&mut [0..100, 10..20, 100..101]).is_err());
    }
}
