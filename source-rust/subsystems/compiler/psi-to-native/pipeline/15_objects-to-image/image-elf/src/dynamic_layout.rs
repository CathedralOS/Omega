/*
Step 1 of the dynamic ELF pipeline: preflight inputs, plan the 14 ELF64 sections
and 5 program headers (`PT_INTERP`, 3 `PT_LOAD` segments, `PT_DYNAMIC`), assign
virtual addresses and file offsets under 64 KiB page alignment (`ELF_MAX_PAGE_SIZE`),
and verify segment congruence, W^X policy, and ISA branch/page envelopes.
*/

use crate::constants::{
    ELF_HEADER_SIZE, ELF_MAX_PAGE_SIZE, ELF64_DYNAMIC_ROW_SIZE, ELF64_RELA_SIZE,
    ELF64_SECTION_HEADER_SIZE, ELF64_SYMBOL_SIZE, IMAGE_BASE, PROGRAM_HEADER_SIZE, SECTION_COUNT,
    SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE,
};
use crate::dynamic_relocation::{LinkageClassification, classify_import_relocations};
use crate::dynamic_tables::{
    EncodedImportTables, build_dynamic_import_tables, dynamic_table_byte_size,
};
use crate::entry_symbol::elf_entry_address;
use crate::imports::{ElfImportLocator, ElfImportRequest, canonical_referenced_imports};
use diagnostics::Diagnostic;
use image::{FinalImage, FinalImageLayout, FinalImageSection};
use object_file::SymbolKind;
use target::{NormalizedElfInterpreterPlan, TargetProfile};

pub(crate) const DYNAMIC_PROGRAM_HEADER_COUNT: usize = 5;
pub(crate) const AARCH64_RELOCATION_PAGE_SIZE: u64 = 0x1000;
pub(crate) const SECTION_NAME_TABLE_INDEX: u16 = 13;

pub(crate) const PF_X: u32 = 1;
pub(crate) const PF_W: u32 = 2;
pub(crate) const PF_R: u32 = 4;

pub(crate) const PT_LOAD: u32 = 1;
pub(crate) const PT_DYNAMIC: u32 = 2;
pub(crate) const PT_INTERP: u32 = 3;

const SHT_NULL: u32 = 0;
const SHT_PROGBITS: u32 = 1;
const SHT_STRTAB: u32 = 3;
const SHT_RELA: u32 = 4;
const SHT_HASH: u32 = 5;
const SHT_DYNAMIC: u32 = 6;
const SHT_DYNSYM: u32 = 11;
const SHT_GNU_HASH: u32 = 0x6fff_fff6;
const SHT_GNU_VERNEED: u32 = 0x6fff_fffe;
const SHT_GNU_VERSYM: u32 = 0x6fff_ffff;

const SHF_INFO_LINK: u64 = 0x40;
const SHF_AARCH64_PURECODE: u64 = 0x2000_0000;

pub(crate) const COMPLETE_SECTION_NAME_TABLE: &[u8] = b"\0.interp\0.dynstr\0.dynsym\0.hash\0.gnu.version\0.gnu.version_r\0.shstrtab\0.gnu.hash\0.plt\0.got.plt\0.rela.plt\0.rela.dyn\0.dynamic\0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SectionRegion {
    ReadOnly,
    ReadExecute,
    ReadWrite,
    FileOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum DynamicSectionIndex {
    Null = 0,
    Interpreter = 1,
    DynamicString = 2,
    DynamicSymbol = 3,
    SystemVHash = 4,
    GnuSymbolVersion = 5,
    GnuVersionRequirement = 6,
    GnuHash = 7,
    ProcedureLinkage = 8,
    ProcedureGot = 9,
    ProcedureRelocation = 10,
    GeneralRelocation = 11,
    DynamicTable = 12,
    SectionNameTable = 13,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlannedSection {
    pub(crate) index: u32,
    pub(crate) name_offset: u32,
    pub(crate) section_type: u32,
    pub(crate) flags: u64,
    pub(crate) region: Option<SectionRegion>,
    pub(crate) file_offset: u64,
    pub(crate) virtual_address: u64,
    pub(crate) byte_size: u64,
    pub(crate) link: u32,
    pub(crate) info: u32,
    pub(crate) alignment: u64,
    pub(crate) entry_size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlannedProgramHeader {
    pub(crate) segment_type: u32,
    pub(crate) flags: u32,
    pub(crate) file_offset: u64,
    pub(crate) virtual_address: u64,
    pub(crate) physical_address: u64,
    pub(crate) file_size: u64,
    pub(crate) memory_size: u64,
    pub(crate) alignment: u64,
}

pub(crate) struct DynamicLayoutPlan {
    pub(crate) target: TargetProfile,
    pub(crate) entry_address: u64,
    pub(crate) text_file_offset: u64,
    pub(crate) data_file_offset: u64,
    pub(crate) image_layout: FinalImageLayout,
    pub(crate) sections: Vec<PlannedSection>,
    pub(crate) program_headers: Vec<PlannedProgramHeader>,
    pub(crate) section_header_table_file_offset: u64,
    pub(crate) total_file_size: usize,
    pub(crate) imports: Vec<ElfImportRequest>,
    pub(crate) tables: EncodedImportTables,
    pub(crate) linkage: LinkageClassification,
}

impl DynamicLayoutPlan {
    pub(crate) fn section(&self, index: DynamicSectionIndex) -> &PlannedSection {
        &self.sections[index as usize]
    }
}

/// Step 1: Preflight inputs and plan all sections, program headers, and addresses.
pub(crate) fn plan_dynamic_layout(
    image: &FinalImage,
    interpreter: &NormalizedElfInterpreterPlan,
) -> Result<DynamicLayoutPlan, Diagnostic> {
    let target = interpreter.target();
    if target.native_target() != image.target
        || !matches!(target, TargetProfile::LinuxX64 | TargetProfile::LinuxArm64)
    {
        return Err(Diagnostic::error(
            "ELF interpreter profile does not match the exact final-image target",
        ));
    }

    let imports = canonical_referenced_imports(image)?;
    if imports.is_empty() {
        return Err(Diagnostic::error(
            "ELF interpreter input has no referenced normalized versioned import to join",
        ));
    }
    for import in &imports {
        let import_target = match &import.locator {
            ElfImportLocator::Versioned { target_profile, .. } => *target_profile,
            ElfImportLocator::Component { locator, .. } => locator.target(),
        };
        if import_target != target {
            return Err(Diagnostic::error(
                "versioned ELF import profile does not match the exact interpreter profile",
            ));
        }
    }

    let bss_alignment = u64::try_from(image.memory.bss_alignment)
        .map_err(|_| Diagnostic::error("source BSS alignment exceeds Elf64_Xword"))?;
    require(
        bss_alignment != 0 && bss_alignment.is_power_of_two(),
        "source BSS alignment must be a nonzero power of two",
    )?;
    let entry_offset = validate_entry_symbol(image)?;

    image.validate_import_program()?;
    let resolver = image
        .import_program
        .resolver
        .as_ref()
        .ok_or_else(|| Diagnostic::error("ELF imports have no previously encoded resolver"))?;

    let tables = build_dynamic_import_tables(&imports, interpreter.interpreter_path())?;
    let linkage = classify_import_relocations(target, image, &imports, &tables.contents.bindings)?;

    let mut plt_size = resolver.bytes.len();
    for slot in &linkage.slots {
        let request = imports.get(slot.request_index).ok_or_else(|| {
            Diagnostic::error("ELF import slot exceeds its closed request roster")
        })?;
        let body = image.encoded_import_body(request.symbol_handle)?;
        plt_size = checked_sum(plt_size, body.bytes.len(), "ELF .plt payload size")?;
    }
    let got_plt_size = checked_product(
        checked_sum(3, linkage.slots.len(), "procedure GOT word count")?,
        8,
        "procedure GOT size",
    )?;
    let rela_plt_size =
        checked_product(linkage.slots.len(), ELF64_RELA_SIZE, "procedure RELA size")?;
    let rela_dyn_size = checked_product(
        linkage.general_relocations.len(),
        ELF64_RELA_SIZE,
        "general RELA size",
    )?;
    let dynamic_size = dynamic_table_byte_size(&tables.contents)?;

    let verneed_count = u32::try_from(tables.contents.verneed.len())
        .map_err(|_| Diagnostic::error("ELF version-need section info exceeds Elf64_Word"))?;
    let plt_flags = match target {
        TargetProfile::LinuxX64 => SHF_ALLOC | SHF_EXECINSTR,
        TargetProfile::LinuxArm64 => SHF_ALLOC | SHF_EXECINSTR | SHF_AARCH64_PURECODE,
        _ => unreachable!(),
    };

    let mut sections = vec![
        PlannedSection {
            index: DynamicSectionIndex::Null as u32,
            name_offset: 0,
            section_type: SHT_NULL,
            flags: 0,
            region: None,
            file_offset: 0,
            virtual_address: 0,
            byte_size: 0,
            link: 0,
            info: 0,
            alignment: 0,
            entry_size: 0,
        },
        section_spec(
            DynamicSectionIndex::Interpreter,
            1,
            SHT_PROGBITS,
            SHF_ALLOC,
            SectionRegion::ReadOnly,
            tables.interpreter.len(),
            0,
            0,
            1,
            0,
        )?,
        section_spec(
            DynamicSectionIndex::DynamicString,
            9,
            SHT_STRTAB,
            SHF_ALLOC,
            SectionRegion::ReadOnly,
            tables.dynstr.len(),
            0,
            0,
            1,
            0,
        )?,
        section_spec(
            DynamicSectionIndex::DynamicSymbol,
            17,
            SHT_DYNSYM,
            SHF_ALLOC,
            SectionRegion::ReadOnly,
            tables.dynsym.len(),
            DynamicSectionIndex::DynamicString as u32,
            1,
            8,
            ELF64_SYMBOL_SIZE as u64,
        )?,
        section_spec(
            DynamicSectionIndex::SystemVHash,
            25,
            SHT_HASH,
            SHF_ALLOC,
            SectionRegion::ReadOnly,
            tables.sysv_hash.len(),
            DynamicSectionIndex::DynamicSymbol as u32,
            0,
            4,
            4,
        )?,
        section_spec(
            DynamicSectionIndex::GnuSymbolVersion,
            31,
            SHT_GNU_VERSYM,
            SHF_ALLOC,
            SectionRegion::ReadOnly,
            tables.versym.len(),
            DynamicSectionIndex::DynamicSymbol as u32,
            0,
            2,
            2,
        )?,
        section_spec(
            DynamicSectionIndex::GnuVersionRequirement,
            44,
            SHT_GNU_VERNEED,
            SHF_ALLOC,
            SectionRegion::ReadOnly,
            tables.verneed.len(),
            DynamicSectionIndex::DynamicString as u32,
            verneed_count,
            4,
            0,
        )?,
        section_spec(
            DynamicSectionIndex::GnuHash,
            69,
            SHT_GNU_HASH,
            SHF_ALLOC,
            SectionRegion::ReadOnly,
            tables.gnu_hash.len(),
            DynamicSectionIndex::DynamicSymbol as u32,
            0,
            8,
            0,
        )?,
        section_spec(
            DynamicSectionIndex::ProcedureLinkage,
            79,
            SHT_PROGBITS,
            plt_flags,
            SectionRegion::ReadExecute,
            plt_size,
            0,
            0,
            16,
            0,
        )?,
        section_spec(
            DynamicSectionIndex::ProcedureGot,
            84,
            SHT_PROGBITS,
            SHF_ALLOC | SHF_WRITE,
            SectionRegion::ReadWrite,
            got_plt_size,
            0,
            0,
            8,
            0,
        )?,
        section_spec(
            DynamicSectionIndex::ProcedureRelocation,
            93,
            SHT_RELA,
            SHF_ALLOC | SHF_INFO_LINK,
            SectionRegion::ReadOnly,
            rela_plt_size,
            DynamicSectionIndex::DynamicSymbol as u32,
            DynamicSectionIndex::ProcedureGot as u32,
            8,
            ELF64_RELA_SIZE as u64,
        )?,
        section_spec(
            DynamicSectionIndex::GeneralRelocation,
            103,
            SHT_RELA,
            SHF_ALLOC | SHF_INFO_LINK,
            SectionRegion::ReadOnly,
            rela_dyn_size,
            DynamicSectionIndex::DynamicSymbol as u32,
            0,
            8,
            ELF64_RELA_SIZE as u64,
        )?,
        section_spec(
            DynamicSectionIndex::DynamicTable,
            113,
            SHT_DYNAMIC,
            SHF_ALLOC | SHF_WRITE,
            SectionRegion::ReadWrite,
            dynamic_size,
            DynamicSectionIndex::DynamicString as u32,
            0,
            8,
            ELF64_DYNAMIC_ROW_SIZE as u64,
        )?,
        section_spec(
            DynamicSectionIndex::SectionNameTable,
            59,
            SHT_STRTAB,
            0,
            SectionRegion::FileOnly,
            COMPLETE_SECTION_NAME_TABLE.len(),
            0,
            0,
            1,
            0,
        )?,
    ];

    // Physical section packing order places ReadOnly sections (including .rela.dyn
    // before .rela.plt), then .text + .plt (ReadExecute), then .got.plt + .dynamic
    // + .data + .bss (ReadWrite), then .shstrtab (FileOnly).
    const PHYSICAL_ORDER: [usize; SECTION_COUNT - 1] = [1, 2, 3, 4, 5, 6, 7, 11, 10, 8, 9, 12, 13];
    let header_end = checked_sum_u64(
        ELF_HEADER_SIZE as u64,
        checked_product_u64(
            PROGRAM_HEADER_SIZE as u64,
            DYNAMIC_PROGRAM_HEADER_COUNT as u64,
            "dynamic ELF program-header bytes",
        )?,
        "dynamic ELF header prefix",
    )?;

    let max_ro_align = max_region_align(&sections, SectionRegion::ReadOnly);
    let max_rx_align = max_region_align(&sections, SectionRegion::ReadExecute);
    let max_file_only_align = max_region_align(&sections, SectionRegion::FileOnly);

    let read_only_base = checked_align_u64(header_end, max_ro_align)?;
    let mut ro_cursor = read_only_base;
    for &idx in &PHYSICAL_ORDER {
        if sections[idx].region == Some(SectionRegion::ReadOnly) {
            ro_cursor = place_section(&mut sections[idx], ro_cursor, true)?;
        }
    }
    let read_only_end = ro_cursor;

    let read_execute_base = checked_align_u64(read_only_end, ELF_MAX_PAGE_SIZE)?;
    let text_size = checked_u64(image.memory.text.len(), "source text size")?;
    let text_end = checked_sum_u64(read_execute_base, text_size, "source text end")?;
    let mut rx_cursor = checked_align_u64(text_end, max_rx_align)?;
    for &idx in &PHYSICAL_ORDER {
        if sections[idx].region == Some(SectionRegion::ReadExecute) {
            rx_cursor = place_section(&mut sections[idx], rx_cursor, true)?;
        }
    }
    let read_execute_end = rx_cursor;

    let read_write_base = checked_align_u64(read_execute_end, ELF_MAX_PAGE_SIZE)?;
    let mut rw_cursor = read_write_base;
    for &idx in &PHYSICAL_ORDER {
        if sections[idx].region == Some(SectionRegion::ReadWrite) {
            rw_cursor = place_section(&mut sections[idx], rw_cursor, true)?;
        }
    }
    let data_file_offset = checked_align_u64(rw_cursor, image.target.pointer_alignment as u64)?;
    let data_size = checked_u64(image.memory.data.len(), "source data size")?;
    let data_end = checked_sum_u64(data_file_offset, data_size, "source data end")?;
    let bss_file_cursor = checked_align_u64(data_end, bss_alignment)?;
    let bss_size = checked_u64(image.memory.bss_size, "source BSS size")?;
    let read_write_memory_end = checked_sum_u64(bss_file_cursor, bss_size, "source BSS end")?;

    let mut file_only_cursor = checked_align_u64(data_end, max_file_only_align)?;
    for &idx in &PHYSICAL_ORDER {
        if sections[idx].region == Some(SectionRegion::FileOnly) {
            file_only_cursor = place_section(&mut sections[idx], file_only_cursor, false)?;
        }
    }
    let section_header_table_file_offset = checked_align_u64(file_only_cursor, 8)?;
    let shdr_table_bytes = checked_product_u64(
        SECTION_COUNT as u64,
        ELF64_SECTION_HEADER_SIZE as u64,
        "section-header table size",
    )?;
    let total_file_size = usize::try_from(checked_sum_u64(
        section_header_table_file_offset,
        shdr_table_bytes,
        "dynamic ELF total file size",
    )?)
    .map_err(|_| Diagnostic::error("dynamic ELF file size exceeds usize"))?;

    let text_address = checked_sum_u64(IMAGE_BASE, read_execute_base, "source text address")?;
    let data_address = checked_sum_u64(IMAGE_BASE, data_file_offset, "source data address")?;
    let bss_address = checked_sum_u64(IMAGE_BASE, bss_file_cursor, "source BSS address")?;
    let image_layout = FinalImageLayout {
        text_address,
        data_address,
        bss_address,
    };

    let entry_address = checked_sum_u64(
        text_address,
        u64::try_from(entry_offset).map_err(|_| Diagnostic::error("entry offset overflows u64"))?,
        "dynamic ELF entry address",
    )?;
    // Re-check against the shared entry-symbol helper for exact agreement.
    debug_assert_eq!(entry_address, elf_entry_address(image, text_address)?);

    let interp = &sections[DynamicSectionIndex::Interpreter as usize];
    let dynamic = &sections[DynamicSectionIndex::DynamicTable as usize];
    let program_headers = vec![
        PlannedProgramHeader {
            segment_type: PT_INTERP,
            flags: PF_R,
            file_offset: interp.file_offset,
            virtual_address: interp.virtual_address,
            physical_address: interp.virtual_address,
            file_size: interp.byte_size,
            memory_size: interp.byte_size,
            alignment: 1,
        },
        PlannedProgramHeader {
            segment_type: PT_LOAD,
            flags: PF_R,
            file_offset: 0,
            virtual_address: IMAGE_BASE,
            physical_address: IMAGE_BASE,
            file_size: read_only_end,
            memory_size: read_only_end,
            alignment: ELF_MAX_PAGE_SIZE,
        },
        PlannedProgramHeader {
            segment_type: PT_LOAD,
            flags: PF_R | PF_X,
            file_offset: read_execute_base,
            virtual_address: text_address,
            physical_address: text_address,
            file_size: read_execute_end - read_execute_base,
            memory_size: read_execute_end - read_execute_base,
            alignment: ELF_MAX_PAGE_SIZE,
        },
        PlannedProgramHeader {
            segment_type: PT_LOAD,
            flags: PF_R | PF_W,
            file_offset: read_write_base,
            virtual_address: checked_sum_u64(IMAGE_BASE, read_write_base, "RW load address")?,
            physical_address: checked_sum_u64(IMAGE_BASE, read_write_base, "RW load address")?,
            file_size: data_end - read_write_base,
            memory_size: read_write_memory_end - read_write_base,
            alignment: ELF_MAX_PAGE_SIZE,
        },
        PlannedProgramHeader {
            segment_type: PT_DYNAMIC,
            flags: PF_R | PF_W,
            file_offset: dynamic.file_offset,
            virtual_address: dynamic.virtual_address,
            physical_address: dynamic.virtual_address,
            file_size: dynamic.byte_size,
            memory_size: dynamic.byte_size,
            alignment: dynamic.alignment,
        },
    ];

    validate_abi_and_envelopes(target, &program_headers, &sections, &image_layout, &linkage)?;

    Ok(DynamicLayoutPlan {
        target,
        entry_address,
        text_file_offset: read_execute_base,
        data_file_offset,
        image_layout,
        sections,
        program_headers,
        section_header_table_file_offset,
        total_file_size,
        imports,
        tables,
        linkage,
    })
}

fn validate_entry_symbol(image: &FinalImage) -> Result<usize, Diagnostic> {
    let entry = image
        .symbol_table
        .symbols
        .is_valid(image.symbol_table.entry_symbol)
        .then(|| {
            image
                .symbol_table
                .symbols
                .get(image.symbol_table.entry_symbol)
        })
        .ok_or_else(|| {
            Diagnostic::error("dynamic ELF image has no exact final-image entry symbol")
        })?;
    require(
        entry.section == FinalImageSection::Text,
        "dynamic ELF entry symbol is not in source text",
    )?;
    require(
        entry.kind == SymbolKind::Function,
        "dynamic ELF entry symbol is not a function",
    )?;
    require(
        entry.size != 0,
        "dynamic ELF entry function has an empty text span",
    )?;
    let entry_end = checked_sum(entry.offset, entry.size, "dynamic ELF entry span")?;
    require(
        entry.offset < image.memory.text.len() && entry_end <= image.memory.text.len(),
        "dynamic ELF entry function lies outside source text",
    )?;
    Ok(entry.offset)
}

#[allow(clippy::too_many_arguments)]
fn section_spec(
    index: DynamicSectionIndex,
    name_offset: u32,
    section_type: u32,
    flags: u64,
    region: SectionRegion,
    byte_size: usize,
    link: u32,
    info: u32,
    alignment: u64,
    entry_size: u64,
) -> Result<PlannedSection, Diagnostic> {
    Ok(PlannedSection {
        index: index as u32,
        name_offset,
        section_type,
        flags,
        region: Some(region),
        file_offset: 0,
        virtual_address: 0,
        byte_size: checked_u64(byte_size, "ELF section payload size")?,
        link,
        info,
        alignment,
        entry_size,
    })
}

fn max_region_align(sections: &[PlannedSection], region: SectionRegion) -> u64 {
    sections
        .iter()
        .filter(|section| section.region == Some(region))
        .map(|section| section.alignment)
        .max()
        .unwrap_or(1)
}

fn place_section(
    section: &mut PlannedSection,
    cursor: u64,
    allocated: bool,
) -> Result<u64, Diagnostic> {
    let file_offset = checked_align_u64(cursor, section.alignment)?;
    section.file_offset = file_offset;
    section.virtual_address = if allocated {
        checked_sum_u64(IMAGE_BASE, file_offset, "ELF section virtual address")?
    } else {
        0
    };
    checked_sum_u64(file_offset, section.byte_size, "ELF section end")
}

fn validate_abi_and_envelopes(
    target: TargetProfile,
    program_headers: &[PlannedProgramHeader],
    sections: &[PlannedSection],
    image_layout: &FinalImageLayout,
    linkage: &LinkageClassification,
) -> Result<(), Diagnostic> {
    for header in &program_headers[1..=3] {
        require(
            header.alignment == ELF_MAX_PAGE_SIZE
                && header.alignment.is_power_of_two()
                && header.file_offset % header.alignment
                    == header.virtual_address % header.alignment,
            "dynamic ELF PT_LOAD alignment or congruence drifted",
        )?;
        require(
            header.flags & PF_R != 0 && header.flags & (PF_W | PF_X) != (PF_W | PF_X),
            "dynamic ELF PT_LOAD violates strict readable W^X policy",
        )?;
        require(
            header.file_size <= header.memory_size,
            "dynamic ELF PT_LOAD file size exceeds memory size",
        )?;
    }
    for pair in program_headers[1..=3].windows(2) {
        require(
            checked_sum_u64(pair[0].file_offset, pair[0].file_size, "PT_LOAD file end")?
                <= pair[1].file_offset,
            "dynamic ELF PT_LOAD file extents overlap",
        )?;
        require(
            checked_sum_u64(
                pair[0].virtual_address,
                pair[0].memory_size,
                "PT_LOAD memory end",
            )? <= pair[1].virtual_address,
            "dynamic ELF PT_LOAD memory extents overlap",
        )?;
    }

    let plt = &sections[DynamicSectionIndex::ProcedureLinkage as usize];
    let got_plt = &sections[DynamicSectionIndex::ProcedureGot as usize];
    require(
        got_plt.alignment >= 8 && got_plt.virtual_address.is_multiple_of(8),
        "ELF .got.plt is not eight-byte aligned",
    )?;
    require(
        plt.byte_size > 0 && got_plt.byte_size > 0,
        "ELF procedure linkage sections cannot be empty",
    )?;

    let plt_last = plt.virtual_address + plt.byte_size - 1;
    let got_last = got_plt.virtual_address + got_plt.byte_size - 1;
    match target {
        TargetProfile::LinuxX64 => {
            let span = plt
                .virtual_address
                .abs_diff(got_plt.virtual_address)
                .max(plt_last.abs_diff(got_last));
            require(
                span <= i32::MAX as u64,
                "x86-64 procedure linkage exceeds signed-32 placement envelope",
            )?;
            for slot in &linkage.slots {
                for site in &slot.call_sites {
                    let source = image_layout.text_address + site.relocation_offset as u64;
                    let call_span = source
                        .abs_diff(plt.virtual_address)
                        .max(source.abs_diff(plt_last));
                    require(
                        call_span <= i32::MAX as u64,
                        "x86-64 call site exceeds signed-32 placement envelope",
                    )?;
                }
            }
        }
        TargetProfile::LinuxArm64 => {
            // AArch64 ADRP uses 4 KiB page arithmetic regardless of kernel page size.
            let adrp_ok = aarch64_page_delta_covers_extent(
                plt.virtual_address,
                got_plt.virtual_address,
                got_plt.byte_size,
            )? && aarch64_page_delta_covers_extent(
                plt_last,
                got_plt.virtual_address,
                got_plt.byte_size,
            )?;
            require(
                adrp_ok,
                "AArch64 ADRP fixup exceeds its 4-KiB-page delta envelope",
            )?;
            for slot in &linkage.slots {
                for site in &slot.call_sites {
                    let source = image_layout.text_address + site.relocation_offset as u64;
                    let span = source
                        .abs_diff(plt.virtual_address)
                        .max(source.abs_diff(plt_last));
                    require(
                        source.is_multiple_of(4) && span < (1 << 27),
                        "AArch64 branch fixup exceeds aligned branch-26 placement envelope",
                    )?;
                }
            }
        }
        _ => unreachable!(),
    }

    Ok(())
}

pub(crate) fn aarch64_page_delta_covers_extent(
    source: u64,
    target_start: u64,
    target_size: u64,
) -> Result<bool, Diagnostic> {
    if target_size == 0 {
        return Ok(false);
    }
    let target_last = checked_sum_u64(target_start, target_size - 1, "AArch64 ADRP target end")?;
    let page_number = |address: u64| address / AARCH64_RELOCATION_PAGE_SIZE;
    let source_page = i128::from(page_number(source));
    let minimum_delta = -(1_i128 << 20);
    let maximum_delta = (1_i128 << 20) - 1;
    Ok([target_start, target_last].into_iter().all(|target| {
        let delta = i128::from(page_number(target)) - source_page;
        (minimum_delta..=maximum_delta).contains(&delta)
    }))
}

pub(crate) fn require(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| Diagnostic::error(message))
}

pub(crate) fn checked_sum(
    left: usize,
    right: usize,
    context: &'static str,
) -> Result<usize, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

pub(crate) fn checked_product(
    left: usize,
    right: usize,
    context: &'static str,
) -> Result<usize, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

pub(crate) fn checked_u32(value: usize, context: &'static str) -> Result<u32, Diagnostic> {
    u32::try_from(value).map_err(|_| Diagnostic::error(format!("{context} exceeds Elf64_Word")))
}

pub(crate) fn checked_u64(value: usize, context: &str) -> Result<u64, Diagnostic> {
    u64::try_from(value).map_err(|_| Diagnostic::error(format!("{context} exceeds Elf64_Xword")))
}

pub(crate) fn checked_sum_u64(left: u64, right: u64, context: &str) -> Result<u64, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows Elf64_Off")))
}

pub(crate) fn checked_product_u64(left: u64, right: u64, context: &str) -> Result<u64, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows Elf64_Off")))
}

pub(crate) fn checked_align_u64(value: u64, alignment: u64) -> Result<u64, Diagnostic> {
    if alignment <= 1 {
        return Ok(value);
    }
    require(
        alignment.is_power_of_two(),
        "ELF placement alignment is not a power of two",
    )?;
    let mask = alignment - 1;
    checked_sum_u64(value, mask, "ELF alignment").map(|sum| sum & !mask)
}

#[cfg(test)]
mod tests {
    use super::{
        COMPLETE_SECTION_NAME_TABLE, DynamicSectionIndex, PF_R, PF_W, PF_X, PT_DYNAMIC, PT_INTERP,
        PT_LOAD, aarch64_page_delta_covers_extent, plan_dynamic_layout,
    };
    use crate::constants::ELF_MAX_PAGE_SIZE;
    use crate::test_support::{IMPORTS, dynamic_import_inputs, import_image, text_entry};
    use target::TargetProfile;

    #[test]
    fn both_targets_plan_ordered_congruent_wx_segments_and_fourteen_sections() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let (mut image, interpreter, _) =
                dynamic_import_inputs(target, &IMPORTS, "__omega_layout_", "valid", "valid");
            text_entry(&mut image, 0);
            let plan = plan_dynamic_layout(&image, &interpreter).expect("layout plan");
            assert_eq!(plan.sections.len(), 14);
            assert_eq!(plan.program_headers.len(), 5);
            assert_eq!(plan.program_headers[0].segment_type, PT_INTERP);
            assert_eq!(plan.program_headers[1].segment_type, PT_LOAD);
            assert_eq!(plan.program_headers[1].flags, PF_R);
            assert_eq!(plan.program_headers[2].segment_type, PT_LOAD);
            assert_eq!(plan.program_headers[2].flags, PF_R | PF_X);
            assert_eq!(plan.program_headers[3].segment_type, PT_LOAD);
            assert_eq!(plan.program_headers[3].flags, PF_R | PF_W);
            assert_eq!(plan.program_headers[4].segment_type, PT_DYNAMIC);
            for load in &plan.program_headers[1..=3] {
                assert_eq!(load.alignment, ELF_MAX_PAGE_SIZE);
                assert_eq!(
                    load.file_offset % ELF_MAX_PAGE_SIZE,
                    load.virtual_address % ELF_MAX_PAGE_SIZE
                );
            }
            let shstrtab = plan.section(DynamicSectionIndex::SectionNameTable);
            assert_eq!(shstrtab.virtual_address, 0);
            assert_eq!(
                shstrtab.byte_size as usize,
                COMPLETE_SECTION_NAME_TABLE.len()
            );
        }
    }

    #[test]
    fn aarch64_adrp_envelope_uses_four_kibibyte_page_boundaries() {
        assert!(aarch64_page_delta_covers_extent(0x41_0000, 0x42_0000, 32).unwrap());
        assert!(
            !aarch64_page_delta_covers_extent(0x40_0000, 0x1_0040_0000, 32).unwrap(),
            "addresses beyond +-4 GiB must fail the 4 KiB ADRP envelope"
        );
    }

    #[test]
    fn invalid_bss_alignment_and_missing_entry_reject() {
        let target = TargetProfile::LinuxX64;
        let (mut image, interpreter, _) =
            dynamic_import_inputs(target, &IMPORTS, "__omega_bss_", "valid", "valid");
        assert!(
            plan_dynamic_layout(&image, &interpreter).is_err(),
            "missing entry symbol must reject"
        );
        text_entry(&mut image, 0);
        image.memory.bss_alignment = 6;
        assert!(
            plan_dynamic_layout(&image, &interpreter).is_err(),
            "non-power-of-two BSS alignment must reject"
        );
        let empty_image = import_image(target, 8, 1);
        assert!(plan_dynamic_layout(&empty_image, &interpreter).is_err());
    }
}
