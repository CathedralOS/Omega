//! ABI validation, deferred constraint envelopes and placed coordinates.

use crate::constants::{ELF_HEADER_SIZE, IMAGE_BASE, PROGRAM_HEADER_SIZE};
use crate::dynamic_executable::checked::checked_sum_u64;
use crate::dynamic_executable::load_placement::load_layout::candidates::Candidate;
use crate::dynamic_executable::load_placement::load_layout::compatibility_fingerprint::checked_align;
use crate::dynamic_executable::load_placement::load_layout::{
    AARCH64_RELOCATION_PAGE_SIZE, DYNAMIC_PROGRAM_HEADER_COUNT, ElfLoadProgramHeaderKind,
    ElfPlacedDynamicSection, ElfPlacedDynamicSectionKind, ElfSectionPlacementResolutionKind, PF_R,
    PF_W, PF_X, PLACEMENT_FIXUP_COUNT, public_section_kind,
};
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::ElfProcedureLinkagePlacementConstraintKind;
use crate::dynamic_executable::section_headers::relative_section_layout::{
    ElfRelativeSectionPayloadRegion, ValidatedElfRelativeSectionPayloadLayout,
};
use crate::dynamic_executable::section_headers::section_header_bytes::ElfSectionHeaderPlacementFixupKind;
use crate::dynamic_executable::section_headers::section_payload_roster::ElfIndexedProcedureFixupStorage;
use diagnostics::Diagnostic;
use target::TargetProfile;

macro_rules! s {
    ($a:expr => $b:expr => $c:expr => $d:expr) => {
        checked_sum_u64($a, $b, $c, $d)
    };
}

pub(crate) fn validate_abi(candidate: &Candidate) -> Result<(), Diagnostic> {
    let expected_kinds = [
        ElfLoadProgramHeaderKind::Interpreter,
        ElfLoadProgramHeaderKind::LoadReadOnly,
        ElfLoadProgramHeaderKind::LoadReadExecute,
        ElfLoadProgramHeaderKind::LoadReadWrite,
        ElfLoadProgramHeaderKind::Dynamic,
    ];
    crate::dynamic_executable::checked::require(
        candidate.program_headers.len() == expected_kinds.len()
            && candidate
                .program_headers
                .iter()
                .zip(expected_kinds)
                .all(|(header, kind)| header.kind == kind),
        "dynamic ELF program headers are not in canonical order",
    )?;
    for header in &candidate.program_headers[1..=3] {
        crate::dynamic_executable::checked::require(
            header.alignment == candidate.max_page_alignment
                && header.alignment.is_power_of_two()
                && header.file_offset % header.alignment
                    == header.virtual_address % header.alignment,
            "dynamic ELF PT_LOAD alignment or congruence drifted",
        )?;
        crate::dynamic_executable::checked::require(
            header.flags & PF_R != 0 && header.flags & (PF_W | PF_X) != (PF_W | PF_X),
            "dynamic ELF PT_LOAD violates strict readable W^X policy",
        )?;
        crate::dynamic_executable::checked::require(
            header.file_size <= header.memory_size,
            "dynamic ELF PT_LOAD file size exceeds memory size",
        )?;
    }
    crate::dynamic_executable::checked::require(
        candidate.program_headers[1].flags == PF_R
            && candidate.program_headers[2].flags == PF_R | PF_X
            && candidate.program_headers[3].flags == PF_R | PF_W,
        "dynamic ELF PT_LOAD permissions drifted",
    )?;
    crate::dynamic_executable::checked::require(
        candidate
            .program_headers
            .iter()
            .all(|header| header.physical_address == header.virtual_address),
        "dynamic ELF physical-address replay drifted from virtual placement",
    )?;
    crate::dynamic_executable::checked::require(
        candidate.program_headers[1].file_offset == 0
            && candidate.program_headers[1].virtual_address == candidate.image_base
            && candidate.program_headers[1].file_size
                >= ELF_HEADER_SIZE as u64
                    + PROGRAM_HEADER_SIZE as u64 * DYNAMIC_PROGRAM_HEADER_COUNT,
        "read-only load no longer owns the ELF/program-header prefix",
    )?;
    for pair in candidate.program_headers[1..=3].windows(2) {
        crate::dynamic_executable::checked::require(
            crate::dynamic_executable::checked::checked_sum_u64(
                pair[0].file_offset,
                pair[0].file_size,
                "PT_LOAD extent",
                "Elf64_Off",
            )? <= pair[1].file_offset,
            "dynamic ELF PT_LOAD file extents overlap",
        )?;
        crate::dynamic_executable::checked::require(
            crate::dynamic_executable::checked::checked_sum_u64(
                pair[0].virtual_address,
                pair[0].memory_size,
                "PT_LOAD memory extent",
                "Elf64_Off",
            )? <= pair[1].virtual_address,
            "dynamic ELF PT_LOAD memory extents overlap or are out of order",
        )?;
    }
    for placed in &candidate.sections {
        if placed.kind == ElfPlacedDynamicSectionKind::Null {
            crate::dynamic_executable::checked::require(
                placed.index == 0
                    && placed.region.is_none()
                    && placed.file_offset == 0
                    && placed.virtual_address.is_none()
                    && placed.byte_size == 0,
                "dynamic ELF null-section placement is not canonical",
            )?;
            continue;
        }
        let Some(region) = placed.region else {
            return Err(Diagnostic::error(
                "non-null dynamic ELF section has no placement region",
            ));
        };
        crate::dynamic_executable::checked::require(
            placed.alignment != 0
                && placed.alignment.is_power_of_two()
                && placed.file_offset % placed.alignment == 0
                && placed
                    .virtual_address
                    .is_none_or(|address| address % placed.alignment == 0),
            "dynamic ELF section placement violates retained sh_addralign",
        )?;
        if region == ElfRelativeSectionPayloadRegion::FileOnly {
            crate::dynamic_executable::checked::require(
                placed.virtual_address.is_none(),
                "file-only dynamic ELF section acquired a virtual address",
            )?;
            continue;
        }
        let load_index = match region {
            ElfRelativeSectionPayloadRegion::ReadOnly => 1,
            ElfRelativeSectionPayloadRegion::ReadExecute => 2,
            ElfRelativeSectionPayloadRegion::ReadWrite => 3,
            ElfRelativeSectionPayloadRegion::FileOnly => unreachable!(),
        };
        let load = candidate.program_headers[load_index];
        let placed_address = placed.virtual_address.ok_or_else(|| {
            Diagnostic::error("allocated dynamic ELF section has no virtual address")
        })?;
        crate::dynamic_executable::checked::require(
            placed_address
                == s!(candidate.image_base => placed.file_offset => "section map" => "Elf64_Off")?
                && placed.file_offset >= load.file_offset
                && crate::dynamic_executable::checked::checked_sum_u64(
                    placed.file_offset,
                    placed.byte_size,
                    "section file extent",
                    "Elf64_Off",
                )? <= crate::dynamic_executable::checked::checked_sum_u64(
                    load.file_offset,
                    load.file_size,
                    "load file extent",
                    "Elf64_Off",
                )?
                && placed_address >= load.virtual_address
                && crate::dynamic_executable::checked::checked_sum_u64(
                    placed_address,
                    placed.byte_size,
                    "section memory extent",
                    "Elf64_Off",
                )? <= crate::dynamic_executable::checked::checked_sum_u64(
                    load.virtual_address,
                    load.memory_size,
                    "load memory extent",
                    "Elf64_Off",
                )?,
            "allocated dynamic ELF section is not mapped exactly by its permission load",
        )?;
    }
    let shstrtab = section(
        &candidate.sections,
        ElfPlacedDynamicSectionKind::SectionNameTable,
    )?;
    crate::dynamic_executable::checked::require(
        shstrtab.region == Some(ElfRelativeSectionPayloadRegion::FileOnly)
            && shstrtab.virtual_address.is_none(),
        "`.shstrtab` must remain file-only",
    )?;
    let rw = candidate.program_headers[3];
    let rx = candidate.program_headers[2];
    let text_file_end = crate::dynamic_executable::checked::checked_sum_u64(
        candidate.image_memory.text_file_offset,
        candidate.image_memory.text_size,
        "source text extent",
        "Elf64_Off",
    )?;
    let text_memory_end = crate::dynamic_executable::checked::checked_sum_u64(
        candidate.image_memory.text_virtual_address,
        candidate.image_memory.text_size,
        "source text memory extent",
        "Elf64_Off",
    )?;
    crate::dynamic_executable::checked::require(
        candidate.image_memory.text_virtual_address
            == crate::dynamic_executable::checked::checked_sum_u64(
                candidate.image_base,
                candidate.image_memory.text_file_offset,
                "source text mapping",
                "Elf64_Off",
            )?
            && candidate.image_memory.text_file_offset >= rx.file_offset
            && text_file_end
                <= s!(rx.file_offset => rx.file_size => "RX file extent" => "Elf64_Off")?
            && candidate.image_memory.text_virtual_address >= rx.virtual_address
            && text_memory_end
                <= s!(rx.virtual_address => rx.memory_size => "RX memory extent" => "Elf64_Off")?,
        "source text is not mapped exactly inside the executable load",
    )?;
    let rw_file_end = s!(rw.file_offset => rw.file_size => "RW file extent" => "Elf64_Off")?;
    let rw_memory_end = crate::dynamic_executable::checked::checked_sum_u64(
        rw.virtual_address,
        rw.memory_size,
        "RW memory extent",
        "Elf64_Off",
    )?;
    let data_file_end = crate::dynamic_executable::checked::checked_sum_u64(
        candidate.image_memory.data_file_offset,
        candidate.image_memory.data_size,
        "source data extent",
        "Elf64_Off",
    )?;
    let data_memory_end = crate::dynamic_executable::checked::checked_sum_u64(
        candidate.image_memory.data_virtual_address,
        candidate.image_memory.data_size,
        "source data memory extent",
        "Elf64_Off",
    )?;
    crate::dynamic_executable::checked::require(
        candidate.image_memory.data_file_offset >= rw.file_offset
            && data_file_end <= rw_file_end
            && candidate.image_memory.data_virtual_address
                == crate::dynamic_executable::checked::checked_sum_u64(
                    candidate.image_base,
                    candidate.image_memory.data_file_offset,
                    "source data mapping",
                    "Elf64_Off",
                )?
            && candidate.image_memory.data_virtual_address >= rw.virtual_address
            && data_memory_end <= rw_memory_end
            && candidate.image_memory.bss_virtual_address >= rw.virtual_address
            && candidate.image_memory.bss_virtual_address >= data_memory_end
            && crate::dynamic_executable::checked::checked_sum_u64(
                candidate.image_memory.bss_virtual_address,
                candidate.image_memory.bss_size,
                "source BSS extent",
                "Elf64_Off",
            )? <= rw_memory_end,
        "source data or BSS is not contained by the writable load",
    )?;
    let target_alignment = retained_image(&candidate.relative).target.pointer_alignment as u64;
    crate::dynamic_executable::checked::require(
        target_alignment != 0
            && target_alignment.is_power_of_two()
            && candidate.image_memory.bss_alignment != 0
            && candidate.image_memory.bss_alignment.is_power_of_two()
            && candidate
                .image_memory
                .data_file_offset
                .is_multiple_of(target_alignment)
            && candidate
                .image_memory
                .data_virtual_address
                .is_multiple_of(target_alignment)
            && candidate
                .image_memory
                .bss_virtual_address
                .is_multiple_of(candidate.image_memory.bss_alignment),
        "source data/BSS placement no longer replays retained target alignment",
    )?;
    let first_rx_section = candidate
        .sections
        .iter()
        .filter(|section| section.region == Some(ElfRelativeSectionPayloadRegion::ReadExecute))
        .map(|section| section.file_offset)
        .min()
        .ok_or_else(|| Diagnostic::error("executable section roster is empty"))?;
    let last_rw_section_end = candidate
        .sections
        .iter()
        .filter(|section| section.region == Some(ElfRelativeSectionPayloadRegion::ReadWrite))
        .map(|section| {
            s!(section.file_offset => section.byte_size => "RW section extent" => "Elf64_Off")
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .max()
        .ok_or_else(|| Diagnostic::error("writable section roster is empty"))?;
    crate::dynamic_executable::checked::require(
        text_file_end <= first_rx_section
            && last_rw_section_end <= candidate.image_memory.data_file_offset,
        "source image storage overlaps dynamic-section storage",
    )?;
    crate::dynamic_executable::checked::require(
        shstrtab.file_offset >= rw_file_end
            && candidate.section_header_table_file_offset
                >= crate::dynamic_executable::checked::checked_sum_u64(
                    shstrtab.file_offset,
                    shstrtab.byte_size,
                    "`.shstrtab` extent",
                    "Elf64_Off",
                )?,
        "file-only metadata overlaps an allocated load",
    )?;
    crate::dynamic_executable::checked::require(
        candidate.section_header_resolutions.len() == PLACEMENT_FIXUP_COUNT,
        "dynamic ELF section-header resolution coverage is not exact",
    )?;
    let upstream = &candidate
        .relative
        .payloads()
        .section_headers()
        .contents()
        .placement_fixups;
    crate::dynamic_executable::checked::require(
        upstream.len() == candidate.section_header_resolutions.len(),
        "section-header placement obligation count drifted",
    )?;
    for (fixup, resolution) in upstream.iter().zip(&candidate.section_header_resolutions) {
        let placed = candidate
            .sections
            .get(fixup.row_index as usize)
            .ok_or_else(|| Diagnostic::error("placement resolution row is absent"))?;
        let (kind, value) = match fixup.kind {
            ElfSectionHeaderPlacementFixupKind::VirtualAddress => (
                ElfSectionPlacementResolutionKind::VirtualAddress,
                placed.virtual_address.ok_or_else(|| {
                    Diagnostic::error("virtual-address resolution names file-only storage")
                })?,
            ),
            ElfSectionHeaderPlacementFixupKind::FileOffset => (
                ElfSectionPlacementResolutionKind::FileOffset,
                placed.file_offset,
            ),
        };
        crate::dynamic_executable::checked::require(
            resolution.row_index == fixup.row_index
                && resolution.section_kind == public_section_kind(fixup.section_kind)
                && resolution.byte_offset == fixup.byte_offset
                && resolution.byte_width == 8
                && resolution.byte_width == fixup.byte_width
                && resolution.kind == kind
                && resolution.value == value,
            "section-header placement resolution does not exactly cover its upstream fixup",
        )?;
    }
    let interpreter = section(
        &candidate.sections,
        ElfPlacedDynamicSectionKind::Interpreter,
    )?;
    let dynamic = section(
        &candidate.sections,
        ElfPlacedDynamicSectionKind::DynamicTable,
    )?;
    for (header, placed) in [
        (&candidate.program_headers[0], interpreter),
        (&candidate.program_headers[4], dynamic),
    ] {
        crate::dynamic_executable::checked::require(
            header.file_offset == placed.file_offset
                && header.virtual_address == placed.virtual_address.unwrap_or(0)
                && header.file_size == placed.byte_size
                && header.memory_size == placed.byte_size,
            "special ELF program header no longer aliases its exact section",
        )?;
    }
    let interpreter_header = candidate.program_headers[0];
    let dynamic_header = candidate.program_headers[4];
    crate::dynamic_executable::checked::require(
        interpreter_header.flags == PF_R
            && interpreter_header.alignment == 1
            && interpreter_header.file_size <= interpreter_header.memory_size
            && range_contains(
                candidate.program_headers[1].file_offset,
                candidate.program_headers[1].file_size,
                interpreter_header.file_offset,
                interpreter_header.file_size,
                "PT_INTERP file containment",
            )?
            && range_contains(
                candidate.program_headers[1].virtual_address,
                candidate.program_headers[1].memory_size,
                interpreter_header.virtual_address,
                interpreter_header.memory_size,
                "PT_INTERP memory containment",
            )?,
        "PT_INTERP flags, alignment, or read-only containment drifted",
    )?;
    crate::dynamic_executable::checked::require(
        dynamic_header.flags == PF_R | PF_W
            && dynamic_header.alignment == dynamic.alignment
            && dynamic_header.file_size <= dynamic_header.memory_size
            && range_contains(
                rw.file_offset,
                rw.file_size,
                dynamic_header.file_offset,
                dynamic_header.file_size,
                "PT_DYNAMIC file containment",
            )?
            && range_contains(
                rw.virtual_address,
                rw.memory_size,
                dynamic_header.virtual_address,
                dynamic_header.memory_size,
                "PT_DYNAMIC memory containment",
            )?,
        "PT_DYNAMIC flags, alignment, or writable containment drifted",
    )?;
    validate_deferred_constraint_envelope(candidate)
}

fn range_contains(
    outer_start: u64,
    outer_size: u64,
    inner_start: u64,
    inner_size: u64,
    context: &str,
) -> Result<bool, Diagnostic> {
    Ok(inner_start >= outer_start
        && crate::dynamic_executable::checked::checked_sum_u64(
            inner_start,
            inner_size,
            context,
            "Elf64_Off",
        )? <= crate::dynamic_executable::checked::checked_sum_u64(
            outer_start,
            outer_size,
            context,
            "Elf64_Off",
        )?)
}

pub(crate) fn validate_deferred_constraint_envelope(
    candidate: &Candidate,
) -> Result<(), Diagnostic> {
    let payloads = candidate.relative.payloads().contents();
    for constraint in &payloads.procedure_constraints {
        let fixup = payloads
            .procedure_fixups
            .iter()
            .find(|fixup| fixup.upstream_ordinal == constraint.fixup_ordinal)
            .ok_or_else(|| {
                Diagnostic::error("procedure placement constraint has no indexed fixup")
            })?;
        let source_base = match fixup.storage {
            ElfIndexedProcedureFixupStorage::SourceText => {
                candidate.image_memory.text_virtual_address
            }
            ElfIndexedProcedureFixupStorage::Section { index, kind } => candidate
                .sections
                .get(index as usize)
                .filter(|section| section.kind == public_section_kind(kind))
                .and_then(|section| section.virtual_address)
                .ok_or_else(|| {
                    Diagnostic::error("procedure fixup storage has no allocated placement")
                })?,
        };
        let source = crate::dynamic_executable::checked::checked_sum_u64(
            source_base,
            fixup.byte_offset as u64,
            "procedure fixup site",
            "Elf64_Off",
        )?;
        let target = candidate
            .sections
            .get(fixup.target_section_index as usize)
            .filter(|section| section.index == fixup.target_section_index)
            .ok_or_else(|| Diagnostic::error("procedure fixup target section is absent"))?;
        let target_start = target
            .virtual_address
            .ok_or_else(|| Diagnostic::error("procedure fixup target section is not allocated"))?;
        crate::dynamic_executable::checked::require(
            target.byte_size != 0,
            "procedure fixup target section has no bytes",
        )?;
        let target_last = crate::dynamic_executable::checked::checked_sum_u64(
            target_start,
            target.byte_size - 1,
            "procedure target last byte",
            "Elf64_Off",
        )?;
        let span = source
            .abs_diff(target_start)
            .max(source.abs_diff(target_last));
        match constraint.kind {
            ElfProcedureLinkagePlacementConstraintKind::X86Signed32 => {
                crate::dynamic_executable::checked::require(
                    span <= i32::MAX as u64,
                    "x86-64 procedure fixup exceeds signed-32 placement envelope",
                )?
            }
            ElfProcedureLinkagePlacementConstraintKind::Aarch64Branch26 => {
                crate::dynamic_executable::checked::require(
                    source % 4 == 0 && span < (1 << 27),
                    "AArch64 branch fixup exceeds aligned branch-26 placement envelope",
                )?
            }
            ElfProcedureLinkagePlacementConstraintKind::Aarch64PageDelta21 => {
                crate::dynamic_executable::checked::require(
                    aarch64_page_delta_covers_extent(source, target_start, target.byte_size)?,
                    "AArch64 ADRP fixup exceeds its 4-KiB-page delta envelope",
                )?
            }
            ElfProcedureLinkagePlacementConstraintKind::Aarch64Load64Low12Aligned => {
                crate::dynamic_executable::checked::require(
                    target.alignment >= 8 && target_start % 8 == 0,
                    "AArch64 low-12 load target is not eight-byte aligned",
                )?
            }
        }
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
    let target_last = crate::dynamic_executable::checked::checked_sum_u64(
        target_start,
        target_size - 1,
        "AArch64 ADRP target last byte",
        "Elf64_Off",
    )?;
    let page_number = |address: u64| address / AARCH64_RELOCATION_PAGE_SIZE;
    let source_page = i128::from(page_number(source));
    let minimum_delta = -(1_i128 << 20);
    let maximum_delta = (1_i128 << 20) - 1;
    Ok([target_start, target_last].into_iter().all(|target| {
        let delta = i128::from(page_number(target)) - source_page;
        (minimum_delta..=maximum_delta).contains(&delta)
    }))
}

pub(crate) fn retained_target(
    relative: &ValidatedElfRelativeSectionPayloadLayout,
) -> TargetProfile {
    relative
        .dynamic_table_plan()
        .payload()
        .plan()
        .descriptors()
        .templates()
        .linkage()
        .descriptors()
        .payloads()
        .plan()
        .inputs()
        .interpreter()
        .target()
}

pub(crate) fn retained_image(
    relative: &ValidatedElfRelativeSectionPayloadLayout,
) -> &image::FinalImage {
    relative
        .dynamic_table_plan()
        .payload()
        .plan()
        .descriptors()
        .templates()
        .linkage()
        .descriptors()
        .payloads()
        .plan()
        .inputs()
        .image()
}

pub(crate) fn section(
    sections: &[ElfPlacedDynamicSection],
    kind: ElfPlacedDynamicSectionKind,
) -> Result<&ElfPlacedDynamicSection, Diagnostic> {
    sections
        .get(kind as usize)
        .filter(|section| section.kind == kind && section.index as usize == kind as usize)
        .ok_or_else(|| Diagnostic::error("dynamic ELF placed-section roster drifted"))
}

pub(crate) fn align_for_region(
    relative: &ValidatedElfRelativeSectionPayloadLayout,
    region: ElfRelativeSectionPayloadRegion,
    cursor: u64,
) -> Result<u64, Diagnostic> {
    let alignment = relative
        .contents()
        .rows
        .iter()
        .filter(|row| row.region == Some(region))
        .map(|row| row.alignment)
        .max()
        .unwrap_or(1);
    checked_align(cursor, alignment, "absolute ELF region alignment")
}

pub(crate) fn placed_coordinates(
    base: u64,
    relative_offset: u64,
    allocated: bool,
) -> Result<(u64, Option<u64>), Diagnostic> {
    let file_offset = s!(base => relative_offset => "absolute ELF section offset" => "Elf64_Off")?;
    let virtual_address = allocated
        .then(|| s!(IMAGE_BASE => file_offset => "absolute ELF section address" => "Elf64_Off"))
        .transpose()?;
    Ok((file_offset, virtual_address))
}
