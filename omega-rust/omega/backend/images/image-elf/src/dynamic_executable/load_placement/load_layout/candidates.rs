//! Load layout candidates, derived contents and candidate validation.

use crate::constants::{ELF_HEADER_SIZE, IMAGE_BASE, PROGRAM_HEADER_SIZE};
use crate::dynamic_executable::load_placement::load_layout::abi_validation::{
    align_for_region, placed_coordinates, public_section_kind, retained_image, retained_target,
    section, validate_abi,
};
use crate::dynamic_executable::load_placement::load_layout::compatibility_fingerprint::{
    checked_align, checked_product, checked_sum,
    non_authoritative_layout_compatibility_fingerprint, require,
};
use crate::dynamic_executable::load_placement::load_layout::{
    DYNAMIC_MAX_PAGE_SIZE, DYNAMIC_PROGRAM_HEADER_COUNT, ElfLoadImageMemoryPlacement,
    ElfLoadProgramHeader, ElfLoadProgramHeaderKind, ElfPlacedDynamicSection,
    ElfPlacedDynamicSectionKind, ElfResolvedSectionHeaderPlacement,
    ElfSectionPlacementResolutionKind, PF_R, PF_W, PF_X, PLACEMENT_FIXUP_COUNT, SECTION_COUNT,
    ValidatedElfDynamicLoadLayout,
};
use crate::dynamic_executable::section_headers::relative_section_layout::{
    ElfRelativeSectionPayloadRegion, ValidatedElfRelativeSectionPayloadLayout,
};
use crate::dynamic_executable::section_headers::section_header_bytes::ElfSectionHeaderPlacementFixupKind;
use diagnostics::Diagnostic;
use target::TargetProfile;

pub(crate) struct Candidate {
    pub(super) relative: ValidatedElfRelativeSectionPayloadLayout,
    pub(super) target: TargetProfile,
    pub(super) image_base: u64,
    pub(super) max_page_alignment: u64,
    pub(super) program_headers: Vec<ElfLoadProgramHeader>,
    pub(super) image_memory: ElfLoadImageMemoryPlacement,
    pub(super) sections: Vec<ElfPlacedDynamicSection>,
    pub(super) section_header_table_file_offset: u64,
    pub(super) section_header_resolutions: Vec<ElfResolvedSectionHeaderPlacement>,
    pub(super) non_authoritative_layout_compatibility_fingerprint: u64,
}

pub(super) struct CandidateValidationError {
    pub(super) candidate: Candidate,
    pub(super) diagnostic: Diagnostic,
}

type DerivedContents = (
    Vec<ElfLoadProgramHeader>,
    ElfLoadImageMemoryPlacement,
    Vec<ElfPlacedDynamicSection>,
    u64,
    Vec<ElfResolvedSectionHeaderPlacement>,
);

pub(super) fn derive_contents(
    relative: &ValidatedElfRelativeSectionPayloadLayout,
    target: TargetProfile,
) -> Result<DerivedContents, Diagnostic> {
    require(
        matches!(target, TargetProfile::LinuxX64 | TargetProfile::LinuxArm64),
        "dynamic ELF load placement requires an exact Linux x86-64 or AArch64 target",
    )?;
    let relative_contents = relative.contents();
    require(
        relative_contents.rows.len() == SECTION_COUNT,
        "dynamic ELF load placement requires the exact fourteen-row relative roster",
    )?;
    for region in [
        ElfRelativeSectionPayloadRegion::ReadOnly,
        ElfRelativeSectionPayloadRegion::ReadExecute,
        ElfRelativeSectionPayloadRegion::ReadWrite,
        ElfRelativeSectionPayloadRegion::FileOnly,
    ] {
        require(
            relative.region_span(region) != 0,
            "dynamic ELF load placement requires every closed relative region",
        )?;
    }

    let header_end = checked_sum(
        ELF_HEADER_SIZE as u64,
        checked_product(
            PROGRAM_HEADER_SIZE as u64,
            DYNAMIC_PROGRAM_HEADER_COUNT,
            "dynamic ELF program-header bytes",
        )?,
        "dynamic ELF header prefix",
    )?;
    let read_only_base = align_for_region(
        relative,
        ElfRelativeSectionPayloadRegion::ReadOnly,
        header_end,
    )?;
    let read_only_end = checked_sum(
        read_only_base,
        relative.region_span(ElfRelativeSectionPayloadRegion::ReadOnly),
        "read-only ELF load end",
    )?;
    let read_execute_base = checked_align(
        read_only_end,
        DYNAMIC_MAX_PAGE_SIZE,
        "read-execute ELF load base",
    )?;
    let image = retained_image(relative);
    let text_size = u64::try_from(image.memory.text.len())
        .map_err(|_| Diagnostic::error("source text size exceeds Elf64_Xword"))?;
    let data_size = u64::try_from(image.memory.data.len())
        .map_err(|_| Diagnostic::error("source data size exceeds Elf64_Xword"))?;
    let bss_size = u64::try_from(image.memory.bss_size)
        .map_err(|_| Diagnostic::error("source BSS size exceeds Elf64_Xword"))?;
    let bss_alignment = u64::try_from(image.memory.bss_alignment)
        .map_err(|_| Diagnostic::error("source BSS alignment exceeds Elf64_Xword"))?;
    require(
        bss_alignment != 0 && bss_alignment.is_power_of_two(),
        "source BSS alignment must be a nonzero power of two",
    )?;
    let read_execute_section_base = align_for_region(
        relative,
        ElfRelativeSectionPayloadRegion::ReadExecute,
        checked_sum(read_execute_base, text_size, "source text end")?,
    )?;
    let read_execute_end = checked_sum(
        read_execute_section_base,
        relative.region_span(ElfRelativeSectionPayloadRegion::ReadExecute),
        "read-execute ELF load end",
    )?;
    let read_write_base = checked_align(
        read_execute_end,
        DYNAMIC_MAX_PAGE_SIZE,
        "read-write ELF load base",
    )?;
    let read_write_section_end = checked_sum(
        read_write_base,
        relative.region_span(ElfRelativeSectionPayloadRegion::ReadWrite),
        "read-write ELF load end",
    )?;
    let data_file_offset = checked_align(
        read_write_section_end,
        image.target.pointer_alignment as u64,
        "source data placement",
    )?;
    let data_end = checked_sum(data_file_offset, data_size, "source data end")?;
    let bss_file_cursor = checked_align(data_end, bss_alignment, "source BSS placement")?;
    let read_write_memory_end = checked_sum(bss_file_cursor, bss_size, "source BSS end")?;
    let file_only_base = align_for_region(
        relative,
        ElfRelativeSectionPayloadRegion::FileOnly,
        data_end,
    )?;

    let mut sections = Vec::with_capacity(SECTION_COUNT);
    for row in &relative_contents.rows {
        let (file_offset, virtual_address) = match row.region {
            None => (0, None),
            Some(ElfRelativeSectionPayloadRegion::ReadOnly) => {
                placed_coordinates(read_only_base, row.relative_file_offset, true)?
            }
            Some(ElfRelativeSectionPayloadRegion::ReadExecute) => {
                placed_coordinates(read_execute_section_base, row.relative_file_offset, true)?
            }
            Some(ElfRelativeSectionPayloadRegion::ReadWrite) => {
                placed_coordinates(read_write_base, row.relative_file_offset, true)?
            }
            Some(ElfRelativeSectionPayloadRegion::FileOnly) => {
                placed_coordinates(file_only_base, row.relative_file_offset, false)?
            }
        };
        sections.push(ElfPlacedDynamicSection {
            index: row.index,
            kind: public_section_kind(row.kind),
            region: row.region,
            file_offset,
            virtual_address,
            byte_size: row.byte_size,
            alignment: row.alignment,
        });
    }

    let section_name_table = section(&sections, ElfPlacedDynamicSectionKind::SectionNameTable)?;
    let section_name_table_end = checked_sum(
        section_name_table.file_offset,
        section_name_table.byte_size,
        "section-name table end",
    )?;
    let section_header_table_file_offset =
        checked_align(section_name_table_end, 8, "section-header table placement")?;

    let image_memory = ElfLoadImageMemoryPlacement {
        text_file_offset: read_execute_base,
        text_virtual_address: checked_sum(IMAGE_BASE, read_execute_base, "source text address")?,
        text_size,
        data_file_offset,
        data_virtual_address: checked_sum(IMAGE_BASE, data_file_offset, "source data address")?,
        data_size,
        bss_virtual_address: checked_sum(IMAGE_BASE, bss_file_cursor, "source BSS address")?,
        bss_size,
        bss_alignment,
    };

    let interpreter = section(&sections, ElfPlacedDynamicSectionKind::Interpreter)?;
    let dynamic = section(&sections, ElfPlacedDynamicSectionKind::DynamicTable)?;
    let dynamic_address = dynamic.virtual_address.ok_or_else(|| {
        Diagnostic::error("dynamic ELF load placement left `.dynamic` outside allocated memory")
    })?;
    let program_headers = vec![
        ElfLoadProgramHeader {
            kind: ElfLoadProgramHeaderKind::Interpreter,
            flags: PF_R,
            file_offset: interpreter.file_offset,
            virtual_address: interpreter.virtual_address.ok_or_else(|| {
                Diagnostic::error(
                    "dynamic ELF load placement left `.interp` outside allocated memory",
                )
            })?,
            physical_address: interpreter.virtual_address.unwrap_or(0),
            file_size: interpreter.byte_size,
            memory_size: interpreter.byte_size,
            alignment: 1,
        },
        ElfLoadProgramHeader {
            kind: ElfLoadProgramHeaderKind::LoadReadOnly,
            flags: PF_R,
            file_offset: 0,
            virtual_address: IMAGE_BASE,
            physical_address: IMAGE_BASE,
            file_size: read_only_end,
            memory_size: read_only_end,
            alignment: DYNAMIC_MAX_PAGE_SIZE,
        },
        ElfLoadProgramHeader {
            kind: ElfLoadProgramHeaderKind::LoadReadExecute,
            flags: PF_R | PF_X,
            file_offset: read_execute_base,
            virtual_address: checked_sum(IMAGE_BASE, read_execute_base, "RX load address")?,
            physical_address: checked_sum(IMAGE_BASE, read_execute_base, "RX load address")?,
            file_size: read_execute_end - read_execute_base,
            memory_size: read_execute_end - read_execute_base,
            alignment: DYNAMIC_MAX_PAGE_SIZE,
        },
        ElfLoadProgramHeader {
            kind: ElfLoadProgramHeaderKind::LoadReadWrite,
            flags: PF_R | PF_W,
            file_offset: read_write_base,
            virtual_address: checked_sum(IMAGE_BASE, read_write_base, "RW load address")?,
            physical_address: checked_sum(IMAGE_BASE, read_write_base, "RW load address")?,
            file_size: data_end - read_write_base,
            memory_size: read_write_memory_end - read_write_base,
            alignment: DYNAMIC_MAX_PAGE_SIZE,
        },
        ElfLoadProgramHeader {
            kind: ElfLoadProgramHeaderKind::Dynamic,
            flags: PF_R | PF_W,
            file_offset: dynamic.file_offset,
            virtual_address: dynamic_address,
            physical_address: dynamic_address,
            file_size: dynamic.byte_size,
            memory_size: dynamic.byte_size,
            alignment: dynamic.alignment,
        },
    ];

    let header_contents = relative.payloads().section_headers().contents();
    require(
        header_contents.placement_fixups.len() == PLACEMENT_FIXUP_COUNT,
        "dynamic ELF load placement requires exactly twenty-five section-header fixups",
    )?;
    let mut resolutions = Vec::with_capacity(PLACEMENT_FIXUP_COUNT);
    for fixup in &header_contents.placement_fixups {
        let placed = sections
            .get(fixup.row_index as usize)
            .filter(|placed| {
                placed.index == fixup.row_index
                    && placed.kind == public_section_kind(fixup.section_kind)
            })
            .ok_or_else(|| {
                Diagnostic::error("section-header placement fixup names no exact placed row")
            })?;
        let (kind, value) = match fixup.kind {
            ElfSectionHeaderPlacementFixupKind::VirtualAddress => (
                ElfSectionPlacementResolutionKind::VirtualAddress,
                placed.virtual_address.ok_or_else(|| {
                    Diagnostic::error("virtual-address fixup names a file-only section")
                })?,
            ),
            ElfSectionHeaderPlacementFixupKind::FileOffset => (
                ElfSectionPlacementResolutionKind::FileOffset,
                placed.file_offset,
            ),
        };
        resolutions.push(ElfResolvedSectionHeaderPlacement {
            row_index: fixup.row_index,
            section_kind: public_section_kind(fixup.section_kind),
            byte_offset: fixup.byte_offset,
            byte_width: fixup.byte_width,
            kind,
            value,
        });
    }
    Ok((
        program_headers,
        image_memory,
        sections,
        section_header_table_file_offset,
        resolutions,
    ))
}

pub(super) fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfDynamicLoadLayout, CandidateValidationError> {
    let expected = match derive_contents(&candidate.relative, candidate.target) {
        Ok(expected) => expected,
        Err(diagnostic) => {
            return Err(CandidateValidationError {
                candidate,
                diagnostic,
            });
        }
    };
    if candidate.target != retained_target(&candidate.relative)
        || candidate.image_base != IMAGE_BASE
        || candidate.max_page_alignment != DYNAMIC_MAX_PAGE_SIZE
        || candidate.program_headers != expected.0
        || candidate.image_memory != expected.1
        || candidate.sections != expected.2
        || candidate.section_header_table_file_offset != expected.3
        || candidate.section_header_resolutions != expected.4
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error("dynamic ELF absolute load layout does not replay"),
        });
    }
    let identity = non_authoritative_layout_compatibility_fingerprint(
        &candidate.relative,
        candidate.target,
        candidate.image_base,
        candidate.max_page_alignment,
        &candidate.program_headers,
        &candidate.image_memory,
        &candidate.sections,
        candidate.section_header_table_file_offset,
        &candidate.section_header_resolutions,
    );
    if candidate.non_authoritative_layout_compatibility_fingerprint == 0
        || candidate.non_authoritative_layout_compatibility_fingerprint != identity
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(
                "dynamic ELF absolute load-layout compatibility fingerprint does not replay",
            ),
        });
    }
    if let Err(diagnostic) = validate_abi(&candidate) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    Ok(ValidatedElfDynamicLoadLayout {
        relative: candidate.relative,
        target: candidate.target,
        image_base: candidate.image_base,
        max_page_alignment: candidate.max_page_alignment,
        program_headers: candidate.program_headers,
        image_memory: candidate.image_memory,
        sections: candidate.sections,
        section_header_table_file_offset: candidate.section_header_table_file_offset,
        section_header_resolutions: candidate.section_header_resolutions,
        non_authoritative_layout_compatibility_fingerprint: candidate
            .non_authoritative_layout_compatibility_fingerprint,
    })
}
