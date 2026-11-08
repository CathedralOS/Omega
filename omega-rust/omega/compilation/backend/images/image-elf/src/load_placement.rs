/*
We execute stages 16 through 18 of the dynamic ELF pipeline: planning virtual
segment and section placements (`plan_elf_dynamic_load_layout`), patching
placed offsets and addresses into the section-header table
(`apply_elf_section_header_placements`), and resolving virtual addresses in the
`.dynamic` table (`apply_elf_dynamic_address_fixups`).
*/

pub(crate) mod load_layout {
    use crate::constants::SECTION_COUNT;
    mod abi_validation {
        use crate::constants::{ELF_HEADER_SIZE, IMAGE_BASE, PROGRAM_HEADER_SIZE};
        use crate::dynamic_executable::checked::{checked_sum_u64, require};
        use crate::dynamic_executable::load_placement::load_layout::candidates::Candidate;
        use crate::dynamic_executable::load_placement::load_layout::compatibility_fingerprint::checked_align;
        use crate::dynamic_executable::load_placement::load_layout::{
            AARCH64_RELOCATION_PAGE_SIZE, DYNAMIC_PROGRAM_HEADER_COUNT, ElfLoadProgramHeaderKind,
            ElfPlacedDynamicSection, ElfPlacedDynamicSectionKind,
            ElfSectionPlacementResolutionKind, PF_R, PF_W, PF_X, PLACEMENT_FIXUP_COUNT,
            public_section_kind,
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
            ($a:expr => $b:expr => $c:expr) => {
                checked_sum_u64($a, $b, $c, "Elf64_Off")
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
            require(
                candidate.program_headers.len() == expected_kinds.len()
                    && candidate
                        .program_headers
                        .iter()
                        .zip(expected_kinds)
                        .all(|(header, kind)| header.kind == kind),
                "dynamic ELF program headers are not in canonical order",
            )?;
            for header in &candidate.program_headers[1..=3] {
                require(
                    header.alignment == candidate.max_page_alignment
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
            require(
                candidate.program_headers[1].flags == PF_R
                    && candidate.program_headers[2].flags == PF_R | PF_X
                    && candidate.program_headers[3].flags == PF_R | PF_W,
                "dynamic ELF PT_LOAD permissions drifted",
            )?;
            require(
                candidate
                    .program_headers
                    .iter()
                    .all(|header| header.physical_address == header.virtual_address),
                "dynamic ELF physical-address replay drifted from virtual placement",
            )?;
            require(
                candidate.program_headers[1].file_offset == 0
                    && candidate.program_headers[1].virtual_address == candidate.image_base
                    && candidate.program_headers[1].file_size
                        >= ELF_HEADER_SIZE as u64
                            + PROGRAM_HEADER_SIZE as u64 * DYNAMIC_PROGRAM_HEADER_COUNT,
                "read-only load no longer owns the ELF/program-header prefix",
            )?;
            for pair in candidate.program_headers[1..=3].windows(2) {
                require(
                    checked_sum_u64(
                        pair[0].file_offset,
                        pair[0].file_size,
                        "PT_LOAD extent",
                        "Elf64_Off",
                    )? <= pair[1].file_offset,
                    "dynamic ELF PT_LOAD file extents overlap",
                )?;
                require(
                    checked_sum_u64(
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
                    require(
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
                require(
                    placed.alignment != 0
                        && placed.alignment.is_power_of_two()
                        && placed.file_offset % placed.alignment == 0
                        && placed
                            .virtual_address
                            .is_none_or(|address| address % placed.alignment == 0),
                    "dynamic ELF section placement violates retained sh_addralign",
                )?;
                if region == ElfRelativeSectionPayloadRegion::FileOnly {
                    require(
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
                require(
                    placed_address
                        == s!(candidate.image_base => placed.file_offset => "section map" => "Elf64_Off")?
                        && placed.file_offset >= load.file_offset
                        && checked_sum_u64(
                            placed.file_offset,
                            placed.byte_size,
                            "section file extent",
                            "Elf64_Off",
                        )? <= s!(load.file_offset => load.file_size => "load file extent")?
                        && placed_address >= load.virtual_address
                        && checked_sum_u64(
                            placed_address,
                            placed.byte_size,
                            "section memory extent",
                            "Elf64_Off",
                        )? <= s!(load.virtual_address => load.memory_size => "load memory extent")?,
                    "allocated dynamic ELF section is not mapped exactly by its permission load",
                )?;
            }
            let shstrtab = section(
                &candidate.sections,
                ElfPlacedDynamicSectionKind::SectionNameTable,
            )?;
            require(
                shstrtab.region == Some(ElfRelativeSectionPayloadRegion::FileOnly)
                    && shstrtab.virtual_address.is_none(),
                "`.shstrtab` must remain file-only",
            )?;
            let rw = candidate.program_headers[3];
            let rx = candidate.program_headers[2];
            let text_file_end = checked_sum_u64(
                candidate.image_memory.text_file_offset,
                candidate.image_memory.text_size,
                "source text extent",
                "Elf64_Off",
            )?;
            let text_memory_end = checked_sum_u64(
                candidate.image_memory.text_virtual_address,
                candidate.image_memory.text_size,
                "source text memory extent",
                "Elf64_Off",
            )?;
            require(
                candidate.image_memory.text_virtual_address
                    == checked_sum_u64(
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
            let rw_file_end =
                s!(rw.file_offset => rw.file_size => "RW file extent" => "Elf64_Off")?;
            let rw_memory_end = s!(rw.virtual_address => rw.memory_size => "RW memory extent")?;
            let data_file_end = checked_sum_u64(
                candidate.image_memory.data_file_offset,
                candidate.image_memory.data_size,
                "source data extent",
                "Elf64_Off",
            )?;
            let data_memory_end = checked_sum_u64(
                candidate.image_memory.data_virtual_address,
                candidate.image_memory.data_size,
                "source data memory extent",
                "Elf64_Off",
            )?;
            require(
                candidate.image_memory.data_file_offset >= rw.file_offset
                    && data_file_end <= rw_file_end
                    && candidate.image_memory.data_virtual_address
                        == checked_sum_u64(
                            candidate.image_base,
                            candidate.image_memory.data_file_offset,
                            "source data mapping",
                            "Elf64_Off",
                        )?
                    && candidate.image_memory.data_virtual_address >= rw.virtual_address
                    && data_memory_end <= rw_memory_end
                    && candidate.image_memory.bss_virtual_address >= rw.virtual_address
                    && candidate.image_memory.bss_virtual_address >= data_memory_end
                    && checked_sum_u64(
                        candidate.image_memory.bss_virtual_address,
                        candidate.image_memory.bss_size,
                        "source BSS extent",
                        "Elf64_Off",
                    )? <= rw_memory_end,
                "source data or BSS is not contained by the writable load",
            )?;
            let target_alignment =
                retained_image(&candidate.relative).target.pointer_alignment as u64;
            require(
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
                .filter(|section| {
                    section.region == Some(ElfRelativeSectionPayloadRegion::ReadExecute)
                })
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
            require(
                text_file_end <= first_rx_section
                    && last_rw_section_end <= candidate.image_memory.data_file_offset,
                "source image storage overlaps dynamic-section storage",
            )?;
            require(
                shstrtab.file_offset >= rw_file_end
                    && candidate.section_header_table_file_offset
                        >= s!(shstrtab.file_offset => shstrtab.byte_size => "`.shstrtab` extent")?,
                "file-only metadata overlaps an allocated load",
            )?;
            require(
                candidate.section_header_resolutions.len() == PLACEMENT_FIXUP_COUNT,
                "dynamic ELF section-header resolution coverage is not exact",
            )?;
            let upstream = &candidate
                .relative
                .payloads()
                .section_headers()
                .contents()
                .placement_fixups;
            require(
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
                require(
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
                require(
                    header.file_offset == placed.file_offset
                        && header.virtual_address == placed.virtual_address.unwrap_or(0)
                        && header.file_size == placed.byte_size
                        && header.memory_size == placed.byte_size,
                    "special ELF program header no longer aliases its exact section",
                )?;
            }
            let interpreter_header = candidate.program_headers[0];
            let dynamic_header = candidate.program_headers[4];
            require(
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
            require(
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
                && checked_sum_u64(inner_start, inner_size, context, "Elf64_Off")?
                    <= s!(outer_start => outer_size => context)?)
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
                let source = s!(source_base => fixup.byte_offset as u64 => "procedure fixup site")?;
                let target = candidate
                    .sections
                    .get(fixup.target_section_index as usize)
                    .filter(|section| section.index == fixup.target_section_index)
                    .ok_or_else(|| Diagnostic::error("procedure fixup target section is absent"))?;
                let target_start = target.virtual_address.ok_or_else(|| {
                    Diagnostic::error("procedure fixup target section is not allocated")
                })?;
                require(
                    target.byte_size != 0,
                    "procedure fixup target section has no bytes",
                )?;
                let target_last =
                    s!(target_start => target.byte_size - 1 => "procedure target last byte")?;
                let span = source
                    .abs_diff(target_start)
                    .max(source.abs_diff(target_last));
                match constraint.kind {
                    ElfProcedureLinkagePlacementConstraintKind::X86Signed32 => require(
                        span <= i32::MAX as u64,
                        "x86-64 procedure fixup exceeds signed-32 placement envelope",
                    )?,
                    ElfProcedureLinkagePlacementConstraintKind::Aarch64Branch26 => require(
                        source % 4 == 0 && span < (1 << 27),
                        "AArch64 branch fixup exceeds aligned branch-26 placement envelope",
                    )?,
                    ElfProcedureLinkagePlacementConstraintKind::Aarch64PageDelta21 => require(
                        aarch64_page_delta_covers_extent(source, target_start, target.byte_size)?,
                        "AArch64 ADRP fixup exceeds its 4-KiB-page delta envelope",
                    )?,
                    ElfProcedureLinkagePlacementConstraintKind::Aarch64Load64Low12Aligned => {
                        require(
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
            let target_last =
                s!(target_start => target_size - 1 => "AArch64 ADRP target last byte")?;
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
            let file_offset =
                s!(base => relative_offset => "absolute ELF section offset" => "Elf64_Off")?;
            let virtual_address = allocated
                .then(|| s!(IMAGE_BASE => file_offset => "absolute ELF section address" => "Elf64_Off"))
                .transpose()?;
            Ok((file_offset, virtual_address))
        }
    }

    mod candidates {
        use crate::constants::{ELF_HEADER_SIZE, IMAGE_BASE, PROGRAM_HEADER_SIZE};
        use crate::dynamic_executable::load_placement::load_layout::abi_validation::{
            align_for_region, placed_coordinates, retained_image, retained_target, section,
            validate_abi,
        };
        use crate::dynamic_executable::load_placement::load_layout::compatibility_fingerprint::{
            checked_align, non_authoritative_layout_compatibility_fingerprint,
        };
        use crate::dynamic_executable::load_placement::load_layout::{
            DYNAMIC_MAX_PAGE_SIZE, DYNAMIC_PROGRAM_HEADER_COUNT, ElfLoadImageMemoryPlacement,
            ElfLoadProgramHeader, ElfLoadProgramHeaderKind, ElfPlacedDynamicSection,
            ElfPlacedDynamicSectionKind, ElfResolvedSectionHeaderPlacement,
            ElfSectionPlacementResolutionKind, PF_R, PF_W, PF_X, PLACEMENT_FIXUP_COUNT,
            SECTION_COUNT, ValidatedElfDynamicLoadLayout, public_section_kind,
        };
        use crate::dynamic_executable::section_headers::relative_section_layout::{
            ElfRelativeSectionPayloadRegion, ValidatedElfRelativeSectionPayloadLayout,
        };
        use crate::dynamic_executable::section_headers::section_header_bytes::ElfSectionHeaderPlacementFixupKind;
        use diagnostics::Diagnostic;
        use target::TargetProfile;

        use crate::dynamic_executable::checked::checked_sum_u64;

        macro_rules! s {
            ($a:expr => $b:expr => $c:expr) => {
                checked_sum_u64($a, $b, $c, "Elf64_Off")
            };
        }

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

        pub(super) use super::super::super::checked::CandidateValidationError;

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
            crate::dynamic_executable::checked::require(
                matches!(target, TargetProfile::LinuxX64 | TargetProfile::LinuxArm64),
                "dynamic ELF load placement requires an exact Linux x86-64 or AArch64 target",
            )?;
            let relative_contents = relative.contents();
            crate::dynamic_executable::checked::require(
                relative_contents.rows.len() == SECTION_COUNT,
                "dynamic ELF load placement requires the exact fourteen-row relative roster",
            )?;
            for region in [
                ElfRelativeSectionPayloadRegion::ReadOnly,
                ElfRelativeSectionPayloadRegion::ReadExecute,
                ElfRelativeSectionPayloadRegion::ReadWrite,
                ElfRelativeSectionPayloadRegion::FileOnly,
            ] {
                crate::dynamic_executable::checked::require(
                    relative.region_span(region) != 0,
                    "dynamic ELF load placement requires every closed relative region",
                )?;
            }

            let header_end = crate::dynamic_executable::checked::checked_sum_u64(
                ELF_HEADER_SIZE as u64,
                crate::dynamic_executable::checked::checked_product_u64(
                    PROGRAM_HEADER_SIZE as u64,
                    DYNAMIC_PROGRAM_HEADER_COUNT,
                    "dynamic ELF program-header bytes",
                    "Elf64_Off",
                )?,
                "dynamic ELF header prefix",
                "Elf64_Off",
            )?;
            let read_only_base = align_for_region(
                relative,
                ElfRelativeSectionPayloadRegion::ReadOnly,
                header_end,
            )?;
            let read_only_end = crate::dynamic_executable::checked::checked_sum_u64(
                read_only_base,
                relative.region_span(ElfRelativeSectionPayloadRegion::ReadOnly),
                "read-only ELF load end",
                "Elf64_Off",
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
            crate::dynamic_executable::checked::require(
                bss_alignment != 0 && bss_alignment.is_power_of_two(),
                "source BSS alignment must be a nonzero power of two",
            )?;
            let read_execute_section_base = align_for_region(
                relative,
                ElfRelativeSectionPayloadRegion::ReadExecute,
                s!(read_execute_base => text_size => "source text end")?,
            )?;
            let read_execute_end = crate::dynamic_executable::checked::checked_sum_u64(
                read_execute_section_base,
                relative.region_span(ElfRelativeSectionPayloadRegion::ReadExecute),
                "read-execute ELF load end",
                "Elf64_Off",
            )?;
            let read_write_base = checked_align(
                read_execute_end,
                DYNAMIC_MAX_PAGE_SIZE,
                "read-write ELF load base",
            )?;
            let read_write_section_end = crate::dynamic_executable::checked::checked_sum_u64(
                read_write_base,
                relative.region_span(ElfRelativeSectionPayloadRegion::ReadWrite),
                "read-write ELF load end",
                "Elf64_Off",
            )?;
            let data_file_offset = checked_align(
                read_write_section_end,
                image.target.pointer_alignment as u64,
                "source data placement",
            )?;
            let data_end = s!(data_file_offset => data_size => "source data end")?;
            let bss_file_cursor = checked_align(data_end, bss_alignment, "source BSS placement")?;
            let read_write_memory_end = s!(bss_file_cursor => bss_size => "source BSS end")?;
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
                    Some(ElfRelativeSectionPayloadRegion::ReadExecute) => placed_coordinates(
                        read_execute_section_base,
                        row.relative_file_offset,
                        true,
                    )?,
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

            let section_name_table =
                section(&sections, ElfPlacedDynamicSectionKind::SectionNameTable)?;
            let section_name_table_end = crate::dynamic_executable::checked::checked_sum_u64(
                section_name_table.file_offset,
                section_name_table.byte_size,
                "section-name table end",
                "Elf64_Off",
            )?;
            let section_header_table_file_offset =
                checked_align(section_name_table_end, 8, "section-header table placement")?;

            let image_memory = ElfLoadImageMemoryPlacement {
                text_file_offset: read_execute_base,
                text_virtual_address: s!(IMAGE_BASE => read_execute_base => "source text address")?,
                text_size,
                data_file_offset,
                data_virtual_address: s!(IMAGE_BASE => data_file_offset => "source data address")?,
                data_size,
                bss_virtual_address: s!(IMAGE_BASE => bss_file_cursor => "source BSS address")?,
                bss_size,
                bss_alignment,
            };

            let interpreter = section(&sections, ElfPlacedDynamicSectionKind::Interpreter)?;
            let dynamic = section(&sections, ElfPlacedDynamicSectionKind::DynamicTable)?;
            let dynamic_address = dynamic.virtual_address.ok_or_else(|| {
                Diagnostic::error(
                    "dynamic ELF load placement left `.dynamic` outside allocated memory",
                )
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
                    virtual_address: s!(IMAGE_BASE => read_execute_base => "RX load address")?,
                    physical_address: s!(IMAGE_BASE => read_execute_base => "RX load address")?,
                    file_size: read_execute_end - read_execute_base,
                    memory_size: read_execute_end - read_execute_base,
                    alignment: DYNAMIC_MAX_PAGE_SIZE,
                },
                ElfLoadProgramHeader {
                    kind: ElfLoadProgramHeaderKind::LoadReadWrite,
                    flags: PF_R | PF_W,
                    file_offset: read_write_base,
                    virtual_address: s!(IMAGE_BASE => read_write_base => "RW load address")?,
                    physical_address: s!(IMAGE_BASE => read_write_base => "RW load address")?,
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
            crate::dynamic_executable::checked::require(
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
                        Diagnostic::error(
                            "section-header placement fixup names no exact placed row",
                        )
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
        ) -> Result<ValidatedElfDynamicLoadLayout, CandidateValidationError<Candidate>> {
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
                    diagnostic: Diagnostic::error(
                        "dynamic ELF absolute load layout does not replay",
                    ),
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
    }

    mod compatibility_fingerprint {
        use crate::dynamic_executable::load_placement::load_layout::{
            DYNAMIC_LOAD_POLICY_TAG, ElfLoadImageMemoryPlacement, ElfLoadProgramHeader,
            ElfPlacedDynamicSection, ElfResolvedSectionHeaderPlacement,
        };
        use crate::dynamic_executable::section_headers::relative_section_layout::ValidatedElfRelativeSectionPayloadLayout;
        use diagnostics::Diagnostic;
        use target::TargetProfile;

        pub(crate) fn non_authoritative_layout_compatibility_fingerprint(
            relative: &ValidatedElfRelativeSectionPayloadLayout,
            target: TargetProfile,
            image_base: u64,
            max_page_alignment: u64,
            headers: &[ElfLoadProgramHeader],
            image_memory: &ElfLoadImageMemoryPlacement,
            sections: &[ElfPlacedDynamicSection],
            section_header_table_file_offset: u64,
            resolutions: &[ElfResolvedSectionHeaderPlacement],
        ) -> u64 {
            let mut hash = crate::dynamic_executable::checked::stage_fingerprint(
                b"omega.elf.dynamic-load-layout.v1",
                relative.non_authoritative_layout_compatibility_fingerprint(),
            );
            hash.byte(target_tag(target));
            hash.byte(DYNAMIC_LOAD_POLICY_TAG);
            hash.bytes(&image_base.to_le_bytes());
            hash.bytes(&max_page_alignment.to_le_bytes());
            for header in headers {
                hash.byte(header.kind as u8);
                hash.bytes(&header.flags.to_le_bytes());
                hash.bytes(&header.file_offset.to_le_bytes());
                hash.bytes(&header.virtual_address.to_le_bytes());
                hash.bytes(&header.physical_address.to_le_bytes());
                hash.bytes(&header.file_size.to_le_bytes());
                hash.bytes(&header.memory_size.to_le_bytes());
                hash.bytes(&header.alignment.to_le_bytes());
            }
            hash.bytes(&image_memory.text_file_offset.to_le_bytes());
            hash.bytes(&image_memory.text_virtual_address.to_le_bytes());
            hash.bytes(&image_memory.text_size.to_le_bytes());
            hash.bytes(&image_memory.data_file_offset.to_le_bytes());
            hash.bytes(&image_memory.data_virtual_address.to_le_bytes());
            hash.bytes(&image_memory.data_size.to_le_bytes());
            hash.bytes(&image_memory.bss_virtual_address.to_le_bytes());
            hash.bytes(&image_memory.bss_size.to_le_bytes());
            hash.bytes(&image_memory.bss_alignment.to_le_bytes());
            for section in sections {
                hash.bytes(&section.index.to_le_bytes());
                hash.byte(section.kind as u8);
                hash.byte(section.region.map_or(0, |region| region as u8));
                hash.bytes(&section.file_offset.to_le_bytes());
                hash.byte(u8::from(section.virtual_address.is_some()));
                hash.bytes(&section.virtual_address.unwrap_or(0).to_le_bytes());
                hash.bytes(&section.byte_size.to_le_bytes());
                hash.bytes(&section.alignment.to_le_bytes());
            }
            for resolution in resolutions {
                hash.bytes(&resolution.row_index.to_le_bytes());
                hash.byte(resolution.section_kind as u8);
                hash.bytes(&(resolution.byte_offset as u64).to_le_bytes());
                hash.byte(resolution.byte_width);
                hash.byte(resolution.kind as u8);
                hash.bytes(&resolution.value.to_le_bytes());
            }
            hash.bytes(&section_header_table_file_offset.to_le_bytes());
            hash.finish()
        }

        const fn target_tag(target: TargetProfile) -> u8 {
            match target {
                TargetProfile::LinuxArm64 => 1,
                TargetProfile::LinuxX64 => 2,
                TargetProfile::MacosArm64 => 3,
                TargetProfile::WindowsX64 => 4,
                TargetProfile::UefiX64 => 5,
                TargetProfile::CrossPlatformCli => 6,
                TargetProfile::LocalUnchecked => 7,
                TargetProfile::AlphaBootstrap => 8,
                TargetProfile::MacosX64 => 9,
            }
        }

        pub(crate) fn checked_align(
            value: u64,
            alignment: u64,
            context: &str,
        ) -> Result<u64, Diagnostic> {
            if alignment <= 1 {
                return Ok(value);
            }
            crate::dynamic_executable::checked::require(
                alignment.is_power_of_two(),
                "ELF placement alignment is not a power of two",
            )?;
            let mask = alignment - 1;
            crate::dynamic_executable::checked::checked_sum_u64(value, mask, context, "Elf64_Off")
                .map(|sum| sum & !mask)
        }
    }

    mod load_headers {
        use crate::dynamic_executable::section_headers::relative_section_layout::ElfRelativeSectionPayloadRegion;
        use crate::dynamic_executable::section_headers::section_roster::ElfDynamicRosterSectionKind;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u8)]
        pub enum ElfLoadProgramHeaderKind {
            Interpreter = 1,
            LoadReadOnly = 2,
            LoadReadExecute = 3,
            LoadReadWrite = 4,
            Dynamic = 5,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u8)]
        pub enum ElfPlacedDynamicSectionKind {
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

        pub(crate) const fn public_section_kind(
            kind: ElfDynamicRosterSectionKind,
        ) -> ElfPlacedDynamicSectionKind {
            kind
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct ElfLoadProgramHeader {
            pub(crate) kind: ElfLoadProgramHeaderKind,
            pub(crate) flags: u32,
            pub(crate) file_offset: u64,
            pub(crate) virtual_address: u64,
            pub(crate) physical_address: u64,
            pub(crate) file_size: u64,
            pub(crate) memory_size: u64,
            pub(crate) alignment: u64,
        }

        impl ElfLoadProgramHeader {
            pub const fn kind(&self) -> ElfLoadProgramHeaderKind {
                self.kind
            }

            pub const fn flags(&self) -> u32 {
                self.flags
            }

            pub const fn file_offset(&self) -> u64 {
                self.file_offset
            }

            pub const fn virtual_address(&self) -> u64 {
                self.virtual_address
            }

            pub const fn physical_address(&self) -> u64 {
                self.physical_address
            }

            pub const fn file_size(&self) -> u64 {
                self.file_size
            }

            pub const fn memory_size(&self) -> u64 {
                self.memory_size
            }

            pub const fn alignment(&self) -> u64 {
                self.alignment
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct ElfPlacedDynamicSection {
            pub(crate) index: u32,
            pub(crate) kind: ElfPlacedDynamicSectionKind,
            pub(crate) region: Option<ElfRelativeSectionPayloadRegion>,
            pub(crate) file_offset: u64,
            pub(crate) virtual_address: Option<u64>,
            pub(crate) byte_size: u64,
            pub(crate) alignment: u64,
        }

        impl ElfPlacedDynamicSection {
            pub const fn index(&self) -> u32 {
                self.index
            }

            pub const fn kind(&self) -> ElfPlacedDynamicSectionKind {
                self.kind
            }

            pub const fn region(&self) -> Option<ElfRelativeSectionPayloadRegion> {
                self.region
            }

            pub const fn file_offset(&self) -> u64 {
                self.file_offset
            }

            pub const fn virtual_address(&self) -> Option<u64> {
                self.virtual_address
            }

            pub const fn byte_size(&self) -> u64 {
                self.byte_size
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(u8)]
        pub(crate) enum ElfSectionPlacementResolutionKind {
            VirtualAddress = 1,
            FileOffset = 2,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) struct ElfResolvedSectionHeaderPlacement {
            pub(crate) row_index: u32,
            pub(crate) section_kind: ElfPlacedDynamicSectionKind,
            pub(crate) byte_offset: usize,
            pub(crate) byte_width: u8,
            pub(crate) kind: ElfSectionPlacementResolutionKind,
            pub(crate) value: u64,
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct ElfLoadImageMemoryPlacement {
            pub(crate) text_file_offset: u64,
            pub(crate) text_virtual_address: u64,
            pub(crate) text_size: u64,
            pub(crate) data_file_offset: u64,
            pub(crate) data_virtual_address: u64,
            pub(crate) data_size: u64,
            pub(crate) bss_virtual_address: u64,
            pub(crate) bss_size: u64,
            pub(crate) bss_alignment: u64,
        }

        impl ElfLoadImageMemoryPlacement {
            pub const fn text_file_offset(&self) -> u64 {
                self.text_file_offset
            }

            pub const fn text_virtual_address(&self) -> u64 {
                self.text_virtual_address
            }

            pub const fn text_size(&self) -> u64 {
                self.text_size
            }

            pub const fn data_file_offset(&self) -> u64 {
                self.data_file_offset
            }

            pub const fn data_virtual_address(&self) -> u64 {
                self.data_virtual_address
            }

            pub const fn data_size(&self) -> u64 {
                self.data_size
            }

            pub const fn bss_virtual_address(&self) -> u64 {
                self.bss_virtual_address
            }

            pub const fn bss_size(&self) -> u64 {
                self.bss_size
            }

            pub const fn bss_alignment(&self) -> u64 {
                self.bss_alignment
            }
        }

        impl ElfResolvedSectionHeaderPlacement {
            pub const fn row_index(&self) -> u32 {
                self.row_index
            }

            pub const fn section_kind(&self) -> ElfPlacedDynamicSectionKind {
                self.section_kind
            }

            pub const fn byte_offset(&self) -> usize {
                self.byte_offset
            }

            pub const fn byte_width(&self) -> u8 {
                self.byte_width
            }

            pub const fn kind(&self) -> ElfSectionPlacementResolutionKind {
                self.kind
            }

            pub const fn value(&self) -> u64 {
                self.value
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            Candidate, DYNAMIC_MAX_PAGE_SIZE, ElfLoadProgramHeader, ElfLoadProgramHeaderKind,
            ElfPlacedDynamicSectionKind, ElfSectionPlacementResolutionKind, IMAGE_BASE, PF_R, PF_W,
            PF_X, ValidatedElfRelativeSectionPayloadLayout, derive_contents,
            non_authoritative_layout_compatibility_fingerprint, plan_elf_dynamic_load_layout,
            retained_target, validate_candidate,
        };
        use crate::dynamic_executable::checked::{checked_product_u64, checked_sum_u64};
        use crate::dynamic_executable::load_placement::load_layout::abi_validation::{
            aarch64_page_delta_covers_extent, validate_abi, validate_deferred_constraint_envelope,
        };
        use crate::dynamic_executable::load_placement::load_layout::compatibility_fingerprint::checked_align;
        use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::ElfProcedureLinkagePlacementConstraintKind;
        use crate::dynamic_executable::section_headers::relative_section_layout::ElfRelativeSectionPayloadRegion;
        use crate::plan_elf_relative_section_payload_layout;
        use target::TargetProfile;

        fn relative(target: TargetProfile) -> ValidatedElfRelativeSectionPayloadLayout {
            relative_with_bss_alignment(target, 16)
        }

        fn relative_with_bss_alignment(
            target: TargetProfile,
            bss_alignment: usize,
        ) -> ValidatedElfRelativeSectionPayloadLayout {
            let mut image = crate::test_support::import_image(target, bss_alignment, 1);
            let symbol_handle = crate::test_support::versioned_import(
                &mut image,
                "__omega_load_layout_import",
                b"libload-layout.so",
                b"load_layout_call",
                b"LOAD_LAYOUT_1",
                target,
                "versioned import locator",
            );
            let payloads =
                crate::test_support::import_call_indexed_payloads(image, symbol_handle, target);
            plan_elf_relative_section_payload_layout(payloads).unwrap()
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let relative = relative(target);
            let target = retained_target(&relative);
            let (
                program_headers,
                image_memory,
                sections,
                section_header_table_file_offset,
                resolutions,
            ) = derive_contents(&relative, target).unwrap();
            let non_authoritative_layout_compatibility_fingerprint =
                non_authoritative_layout_compatibility_fingerprint(
                    &relative,
                    target,
                    IMAGE_BASE,
                    DYNAMIC_MAX_PAGE_SIZE,
                    &program_headers,
                    &image_memory,
                    &sections,
                    section_header_table_file_offset,
                    &resolutions,
                );
            Candidate {
                relative,
                target,
                image_base: IMAGE_BASE,
                max_page_alignment: DYNAMIC_MAX_PAGE_SIZE,
                program_headers,
                image_memory,
                sections,
                section_header_table_file_offset,
                section_header_resolutions: resolutions,
                non_authoritative_layout_compatibility_fingerprint,
            }
        }

        #[test]
        fn both_targets_close_ordered_congruent_wx_loads_and_owned_memory() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let layout = plan_elf_dynamic_load_layout(relative(target)).unwrap();
                assert_eq!(layout.target(), target);
                assert_eq!(layout.image_base(), IMAGE_BASE);
                assert_eq!(layout.max_page_alignment(), 0x1_0000);
                assert_ne!(
                    layout.non_authoritative_layout_compatibility_fingerprint(),
                    0
                );
                assert_eq!(
                    layout
                        .program_headers()
                        .iter()
                        .map(ElfLoadProgramHeader::kind)
                        .collect::<Vec<_>>(),
                    vec![
                        ElfLoadProgramHeaderKind::Interpreter,
                        ElfLoadProgramHeaderKind::LoadReadOnly,
                        ElfLoadProgramHeaderKind::LoadReadExecute,
                        ElfLoadProgramHeaderKind::LoadReadWrite,
                        ElfLoadProgramHeaderKind::Dynamic,
                    ]
                );
                for header in &layout.program_headers()[1..=3] {
                    assert_eq!(header.alignment(), layout.max_page_alignment());
                    assert_eq!(
                        header.file_offset() % header.alignment(),
                        header.virtual_address() % header.alignment()
                    );
                    assert_eq!(header.physical_address(), header.virtual_address());
                    assert_ne!(header.flags() & PF_R, 0);
                    assert_ne!(header.flags() & (PF_W | PF_X), PF_W | PF_X);
                }
                assert_eq!(layout.program_headers()[1].flags(), PF_R);
                assert_eq!(layout.program_headers()[2].flags(), PF_R | PF_X);
                assert_eq!(layout.program_headers()[3].flags(), PF_R | PF_W);
                assert_eq!(layout.image_memory().text_size(), 32);
                assert_eq!(layout.image_memory().data_size(), 13);
                assert_eq!(layout.image_memory().bss_size(), 23);
                assert_eq!(layout.image_memory().bss_alignment(), 16);
                assert_eq!(
                    layout.final_image_layout().text_address,
                    layout.image_memory().text_virtual_address()
                );
                validate_abi(&candidate(target)).unwrap();
            }
        }

        #[test]
        fn special_headers_alias_sections_and_file_only_metadata_stays_outside_loads() {
            let layout = plan_elf_dynamic_load_layout(relative(TargetProfile::LinuxX64)).unwrap();
            let interpreter = &layout.sections()[ElfPlacedDynamicSectionKind::Interpreter as usize];
            let dynamic = layout.dynamic_section();
            let shstrtab =
                &layout.sections()[ElfPlacedDynamicSectionKind::SectionNameTable as usize];
            for (header, placed) in [
                (&layout.program_headers()[0], interpreter),
                (&layout.program_headers()[4], dynamic),
            ] {
                assert_eq!(header.file_offset(), placed.file_offset());
                assert_eq!(header.virtual_address(), placed.virtual_address().unwrap());
                assert_eq!(header.file_size(), placed.byte_size());
                assert_eq!(header.memory_size(), placed.byte_size());
            }
            assert_eq!(
                shstrtab.region(),
                Some(ElfRelativeSectionPayloadRegion::FileOnly)
            );
            assert_eq!(shstrtab.virtual_address(), None);
            let rw = layout.program_headers()[3];
            assert!(shstrtab.file_offset() >= rw.file_offset() + rw.file_size());
            assert!(
                layout.section_header_table_file_offset()
                    >= shstrtab.file_offset() + shstrtab.byte_size()
            );
            assert_eq!(layout.section_header_table_byte_size(), 14 * 64);
        }

        #[test]
        fn exact_twenty_five_fixups_are_resolved_without_mutating_template_bytes() {
            let layout = plan_elf_dynamic_load_layout(relative(TargetProfile::LinuxArm64)).unwrap();
            let template = layout.relative().payloads().section_headers().contents();
            assert_eq!(layout.section_header_resolutions().len(), 25);
            assert_eq!(template.placement_fixups.len(), 25);
            for (fixup, resolution) in template
                .placement_fixups
                .iter()
                .zip(layout.section_header_resolutions())
            {
                assert_eq!(resolution.row_index(), fixup.row_index);
                assert_eq!(resolution.byte_offset(), fixup.byte_offset);
                assert_eq!(resolution.byte_width(), 8);
                assert_eq!(
                    &template.bytes[fixup.byte_offset..fixup.byte_offset + 8],
                    &[0; 8]
                );
                let placed = &layout.sections()[fixup.row_index as usize];
                let expected = match resolution.kind() {
                    ElfSectionPlacementResolutionKind::VirtualAddress => {
                        placed.virtual_address().unwrap()
                    }
                    ElfSectionPlacementResolutionKind::FileOffset => placed.file_offset(),
                };
                assert_eq!(resolution.value(), expected);
                assert_eq!(resolution.section_kind(), placed.kind());
            }
        }

        #[test]
        fn placement_is_deterministic_and_target_bound() {
            let first = plan_elf_dynamic_load_layout(relative(TargetProfile::LinuxX64)).unwrap();
            let second = plan_elf_dynamic_load_layout(relative(TargetProfile::LinuxX64)).unwrap();
            let arm = plan_elf_dynamic_load_layout(relative(TargetProfile::LinuxArm64)).unwrap();
            assert_eq!(first.program_headers(), second.program_headers());
            assert_eq!(first.sections(), second.sections());
            assert_eq!(
                first.non_authoritative_layout_compatibility_fingerprint(),
                second.non_authoritative_layout_compatibility_fingerprint()
            );
            assert_ne!(
                first.non_authoritative_layout_compatibility_fingerprint(),
                arm.non_authoritative_layout_compatibility_fingerprint()
            );
        }

        #[test]
        fn geometry_resolution_policy_and_identity_drift_reject_with_custody() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| candidate.program_headers.swap(0, 1)),
                Box::new(|candidate| candidate.program_headers[2].physical_address ^= 1),
                Box::new(|candidate| candidate.image_memory.data_file_offset ^= 1),
                Box::new(|candidate| candidate.sections[1].file_offset ^= 1),
                Box::new(|candidate| candidate.section_header_table_file_offset ^= 1),
                Box::new(|candidate| {
                    candidate
                        .section_header_resolutions
                        .pop()
                        .map(|_| ())
                        .unwrap()
                }),
                Box::new(|candidate| candidate.section_header_resolutions[0].value ^= 1),
                Box::new(|candidate| candidate.max_page_alignment >>= 1),
                Box::new(|candidate| {
                    candidate.non_authoritative_layout_compatibility_fingerprint ^= 1
                }),
            ];
            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxX64);
                let relative_identity = candidate
                    .relative
                    .non_authoritative_layout_compatibility_fingerprint();
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("absolute ELF load-layout corruption must reject");
                assert_eq!(
                    error
                        .candidate
                        .relative
                        .non_authoritative_layout_compatibility_fingerprint(),
                    relative_identity
                );
            }
        }

        #[test]
        fn independent_abi_replay_rejects_each_source_and_auxiliary_geometry_family() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| candidate.image_memory.text_file_offset ^= 1),
                Box::new(|candidate| candidate.image_memory.text_virtual_address ^= 1),
                Box::new(|candidate| candidate.image_memory.data_virtual_address ^= 1),
                Box::new(|candidate| {
                    candidate.image_memory.bss_virtual_address =
                        candidate.image_memory.data_virtual_address
                }),
                Box::new(|candidate| candidate.image_memory.bss_alignment = 0),
                Box::new(|candidate| candidate.program_headers[2].memory_size = 0),
                Box::new(|candidate| {
                    candidate.program_headers[3].virtual_address =
                        candidate.program_headers[2].virtual_address
                }),
                Box::new(|candidate| candidate.sections[1].alignment = 3),
                Box::new(|candidate| candidate.program_headers[0].flags = PF_R | PF_W),
                Box::new(|candidate| candidate.program_headers[0].alignment = 2),
                Box::new(|candidate| candidate.program_headers[4].alignment = 1),
            ];
            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxX64);
                corrupt(&mut candidate);
                validate_abi(&candidate).expect_err("independent ELF ABI replay must reject drift");
            }

            let mut source_section_overlap = candidate(TargetProfile::LinuxX64);
            let rw = source_section_overlap.program_headers[3];
            source_section_overlap.image_memory.data_file_offset = rw.file_offset;
            source_section_overlap.image_memory.data_virtual_address = rw.virtual_address;
            source_section_overlap.image_memory.bss_virtual_address = checked_align(
                rw.virtual_address + source_section_overlap.image_memory.data_size,
                source_section_overlap.image_memory.bss_alignment,
                "test BSS",
            )
            .unwrap();
            validate_abi(&source_section_overlap)
                .expect_err("source data overlapping dynamic storage must reject");
        }

        #[test]
        fn aarch64_relocation_envelopes_use_exact_four_kibibyte_page_boundaries() {
            let source = 0x2_0000_0000_u64;
            let maximum_positive_page = source + (((1_u64 << 20) - 1) * 0x1000);
            assert!(
                aarch64_page_delta_covers_extent(source, maximum_positive_page, 0x1000).unwrap()
            );
            assert!(
                !aarch64_page_delta_covers_extent(source, maximum_positive_page, 0x1001).unwrap(),
                "the last byte crossing into the first out-of-range page must reject"
            );
            let maximum_negative_page = source - ((1_u64 << 20) * 0x1000);
            assert!(aarch64_page_delta_covers_extent(source, maximum_negative_page, 1).unwrap());
            assert!(
                !aarch64_page_delta_covers_extent(source, maximum_negative_page - 0x1000, 1)
                    .unwrap()
            );
            assert!(!aarch64_page_delta_covers_extent(source, source, 0).unwrap());

            let arm = candidate(TargetProfile::LinuxArm64);
            let constraint_kinds = arm
                .relative
                .payloads()
                .contents()
                .procedure_constraints
                .iter()
                .map(|constraint| constraint.kind)
                .collect::<Vec<_>>();
            assert!(
                constraint_kinds
                    .contains(&ElfProcedureLinkagePlacementConstraintKind::Aarch64PageDelta21)
            );
            assert!(
                constraint_kinds.contains(
                    &ElfProcedureLinkagePlacementConstraintKind::Aarch64Load64Low12Aligned
                )
            );
            assert!(
                constraint_kinds
                    .contains(&ElfProcedureLinkagePlacementConstraintKind::Aarch64Branch26)
            );

            let mut branch_drift = candidate(TargetProfile::LinuxArm64);
            branch_drift.sections[ElfPlacedDynamicSectionKind::ProcedureLinkage as usize]
                .virtual_address = Some(1_u64 << 40);
            validate_deferred_constraint_envelope(&branch_drift)
                .expect_err("out-of-range AArch64 branch target must reject");

            let mut low_twelve_drift = candidate(TargetProfile::LinuxArm64);
            low_twelve_drift.sections[ElfPlacedDynamicSectionKind::ProcedureGot as usize]
                .virtual_address = low_twelve_drift.sections
                [ElfPlacedDynamicSectionKind::ProcedureGot as usize]
                .virtual_address
                .map(|address| address + 1);
            validate_deferred_constraint_envelope(&low_twelve_drift)
                .expect_err("misaligned AArch64 low-12 target must reject");
        }

        #[test]
        fn invalid_bss_alignment_rejects_with_exact_relative_layout_custody() {
            for alignment in [0, 3] {
                let relative = relative_with_bss_alignment(TargetProfile::LinuxX64, alignment);
                let identity = relative.non_authoritative_layout_compatibility_fingerprint();
                let error = plan_elf_dynamic_load_layout(relative)
                    .expect_err("invalid retained BSS alignment must reject");
                let (relative, _) = error.into_parts();
                assert_eq!(
                    relative.non_authoritative_layout_compatibility_fingerprint(),
                    identity
                );
            }
        }

        #[test]
        fn checked_arithmetic_rejects_overflow_and_invalid_alignment() {
            assert!(checked_sum_u64(u64::MAX, 1, "sum", "Elf64_Addr").is_err());
            assert!(checked_product_u64(u64::MAX, 2, "product", "Elf64_Addr").is_err());
            assert!(checked_align(u64::MAX, 8, "alignment").is_err());
            assert!(checked_align(7, 3, "alignment").is_err());
        }
    }

    mod validated_layout {
        #[cfg(test)]
        use crate::dynamic_executable::load_placement::load_layout::ElfPlacedDynamicSectionKind;
        use crate::dynamic_executable::load_placement::load_layout::abi_validation::retained_image;
        use crate::dynamic_executable::load_placement::load_layout::{
            ElfLoadImageMemoryPlacement, ElfLoadProgramHeader, ElfPlacedDynamicSection,
            ElfResolvedSectionHeaderPlacement,
        };
        use crate::dynamic_executable::section_headers::relative_section_layout::ValidatedElfRelativeSectionPayloadLayout;
        use diagnostics::Diagnostic;
        use target::TargetProfile;

        #[derive(Debug)]
        #[must_use = "validated dynamic ELF load layout retains all upstream payload custody"]
        pub struct ValidatedElfDynamicLoadLayout {
            pub(crate) relative: ValidatedElfRelativeSectionPayloadLayout,
            pub(crate) target: TargetProfile,
            pub(crate) image_base: u64,
            pub(crate) max_page_alignment: u64,
            pub(crate) program_headers: Vec<ElfLoadProgramHeader>,
            pub(crate) image_memory: ElfLoadImageMemoryPlacement,
            pub(crate) sections: Vec<ElfPlacedDynamicSection>,
            pub(crate) section_header_table_file_offset: u64,
            pub(crate) section_header_resolutions: Vec<ElfResolvedSectionHeaderPlacement>,
            pub(crate) non_authoritative_layout_compatibility_fingerprint: u64,
        }

        impl ValidatedElfDynamicLoadLayout {
            pub const fn relative(&self) -> &ValidatedElfRelativeSectionPayloadLayout {
                &self.relative
            }

            pub const fn target(&self) -> TargetProfile {
                self.target
            }

            pub const fn image_base(&self) -> u64 {
                self.image_base
            }

            pub const fn max_page_alignment(&self) -> u64 {
                self.max_page_alignment
            }

            pub fn program_headers(&self) -> &[ElfLoadProgramHeader] {
                &self.program_headers
            }

            pub const fn image_memory(&self) -> &ElfLoadImageMemoryPlacement {
                &self.image_memory
            }

            pub const fn final_image_layout(&self) -> image::FinalImageLayout {
                image::FinalImageLayout {
                    text_address: self.image_memory.text_virtual_address,
                    data_address: self.image_memory.data_virtual_address,
                    bss_address: self.image_memory.bss_virtual_address,
                }
            }

            pub(crate) fn retained_image(&self) -> &image::FinalImage {
                retained_image(&self.relative)
            }

            pub fn sections(&self) -> &[ElfPlacedDynamicSection] {
                &self.sections
            }

            pub(crate) fn section_header_resolutions(
                &self,
            ) -> &[ElfResolvedSectionHeaderPlacement] {
                &self.section_header_resolutions
            }

            pub const fn section_header_table_file_offset(&self) -> u64 {
                self.section_header_table_file_offset
            }

            #[cfg(test)]
            pub(crate) fn section_header_table_byte_size(&self) -> usize {
                self.relative.payloads().section_headers().byte_count()
            }

            #[cfg(test)]
            pub(crate) fn dynamic_section(&self) -> &ElfPlacedDynamicSection {
                &self.sections[ElfPlacedDynamicSectionKind::DynamicTable as usize]
            }

            pub const fn non_authoritative_layout_compatibility_fingerprint(&self) -> u64 {
                self.non_authoritative_layout_compatibility_fingerprint
            }

            pub(crate) fn into_relative(self) -> ValidatedElfRelativeSectionPayloadLayout {
                self.relative
            }
        }

        #[derive(Debug)]
        #[must_use = "dynamic ELF load-layout rejection retains the relative layout"]
        pub struct ElfDynamicLoadLayoutError {
            pub(crate) relative: ValidatedElfRelativeSectionPayloadLayout,
            pub(crate) diagnostic: Diagnostic,
        }

        image::rejection_carrier!(
            ElfDynamicLoadLayoutError;
            relative: ValidatedElfRelativeSectionPayloadLayout,
        );
    }

    pub(crate) use load_headers::public_section_kind;

    pub(crate) use load_headers::{
        ElfLoadImageMemoryPlacement, ElfLoadProgramHeader, ElfLoadProgramHeaderKind,
        ElfPlacedDynamicSection, ElfPlacedDynamicSectionKind, ElfResolvedSectionHeaderPlacement,
        ElfSectionPlacementResolutionKind,
    };
    pub(crate) use validated_layout::{ElfDynamicLoadLayoutError, ValidatedElfDynamicLoadLayout};

    use crate::constants::IMAGE_BASE;
    use crate::dynamic_executable::load_placement::load_layout::abi_validation::retained_target;
    use crate::dynamic_executable::load_placement::load_layout::candidates::Candidate;
    use crate::dynamic_executable::load_placement::load_layout::candidates::{
        derive_contents, validate_candidate,
    };
    use crate::dynamic_executable::load_placement::load_layout::compatibility_fingerprint::non_authoritative_layout_compatibility_fingerprint;
    use crate::dynamic_executable::section_headers::relative_section_layout::ValidatedElfRelativeSectionPayloadLayout;

    const PLACEMENT_FIXUP_COUNT: usize = 25;

    const DYNAMIC_PROGRAM_HEADER_COUNT: u64 = 5;

    const DYNAMIC_MAX_PAGE_SIZE: u64 = 0x1_0000;

    const DYNAMIC_LOAD_POLICY_TAG: u8 = 1;

    const AARCH64_RELOCATION_PAGE_SIZE: u64 = 0x1000;

    const PF_X: u32 = 1;

    const PF_W: u32 = 2;

    const PF_R: u32 = 4;

    pub(crate) fn plan_elf_dynamic_load_layout(
        relative: ValidatedElfRelativeSectionPayloadLayout,
    ) -> Result<ValidatedElfDynamicLoadLayout, Box<ElfDynamicLoadLayoutError>> {
        let target = retained_target(&relative);
        let (
            program_headers,
            image_memory,
            sections,
            section_header_table_file_offset,
            section_header_resolutions,
        ) = match derive_contents(&relative, target) {
            Ok(contents) => contents,
            Err(diagnostic) => {
                return Err(Box::new(ElfDynamicLoadLayoutError {
                    relative,
                    diagnostic,
                }));
            }
        };
        let image_base = IMAGE_BASE;
        let max_page_alignment = DYNAMIC_MAX_PAGE_SIZE;
        let non_authoritative_layout_compatibility_fingerprint =
            non_authoritative_layout_compatibility_fingerprint(
                &relative,
                target,
                image_base,
                max_page_alignment,
                &program_headers,
                &image_memory,
                &sections,
                section_header_table_file_offset,
                &section_header_resolutions,
            );
        let candidate = Candidate {
            relative,
            target,
            image_base,
            max_page_alignment,
            program_headers,
            image_memory,
            sections,
            section_header_table_file_offset,
            section_header_resolutions,
            non_authoritative_layout_compatibility_fingerprint,
        };
        validate_candidate(candidate).map_err(|error| {
            Box::new(ElfDynamicLoadLayoutError {
                relative: error.candidate.relative,
                diagnostic: error.diagnostic,
            })
        })
    }
}

pub(crate) mod placed_section_headers {
    use super::super::checked::{self, CandidateValidationError};
    use crate::constants::{ELF64_SECTION_HEADER_SIZE, SECTION_COUNT};
    use crate::dynamic_executable::checked::{
        DecodedElf64SectionHeader, checked_product, checked_sum, decode_section_header_rows,
        field_mut, read_u64, require,
    };
    use crate::dynamic_executable::load_placement::load_layout::{
        ElfResolvedSectionHeaderPlacement, ElfSectionPlacementResolutionKind,
        ValidatedElfDynamicLoadLayout, public_section_kind,
    };
    use crate::dynamic_executable::section_headers::section_roster::ElfNumericSectionDescriptor;
    use diagnostics::Diagnostic;

    const ELF64_SECTION_ADDRESS_OFFSET: usize = 16;
    const ELF64_SECTION_FILE_OFFSET: usize = 24;
    const ELF64_PLACEMENT_FIELD_SIZE: u8 = 8;
    const FILE_OFFSET_PLACEMENT_COUNT: usize = 13;
    const VIRTUAL_ADDRESS_PLACEMENT_COUNT: usize = 12;
    const PLACEMENT_COUNT: usize = 25;
    const SECTION_NAME_TABLE_INDEX: usize = 13;
    pub(crate) type ElfAppliedSectionHeaderPlacement = ElfResolvedSectionHeaderPlacement;

    #[derive(Debug)]
    #[must_use = "applied ELF section headers retain the absolute load layout"]
    pub struct ValidatedElfPlacedSectionHeaderTable {
        load_layout: ValidatedElfDynamicLoadLayout,
        contents: ElfPlacedSectionHeaderContents,
        non_authoritative_placed_compatibility_fingerprint: u64,
    }

    impl ValidatedElfPlacedSectionHeaderTable {
        pub const fn load_layout(&self) -> &ValidatedElfDynamicLoadLayout {
            &self.load_layout
        }

        pub fn bytes(&self) -> &[u8] {
            &self.contents.bytes
        }

        #[cfg(test)]
        pub(crate) fn applied_placements(&self) -> &[ElfAppliedSectionHeaderPlacement] {
            &self.contents.applications
        }

        pub const fn non_authoritative_placed_compatibility_fingerprint(&self) -> u64 {
            self.non_authoritative_placed_compatibility_fingerprint
        }

        pub(crate) fn into_parts(
            self,
        ) -> (
            ValidatedElfDynamicLoadLayout,
            ElfPlacedSectionHeaderContents,
        ) {
            (self.load_layout, self.contents)
        }
    }

    #[derive(Debug)]
    #[must_use = "section-header placement rejection retains the absolute load layout"]
    pub struct ElfSectionHeaderPlacementApplicationError {
        load_layout: ValidatedElfDynamicLoadLayout,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(
        ElfSectionHeaderPlacementApplicationError;
        load_layout: ValidatedElfDynamicLoadLayout,
    );

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfPlacedSectionHeaderContents {
        pub(crate) bytes: Vec<u8>,
        pub(crate) applications: Vec<ElfAppliedSectionHeaderPlacement>,
    }

    struct Candidate {
        load_layout: ValidatedElfDynamicLoadLayout,
        contents: ElfPlacedSectionHeaderContents,
        non_authoritative_placed_compatibility_fingerprint: u64,
    }

    pub(crate) fn apply_elf_section_header_placements(
        load_layout: ValidatedElfDynamicLoadLayout,
    ) -> Result<ValidatedElfPlacedSectionHeaderTable, Box<ElfSectionHeaderPlacementApplicationError>>
    {
        checked::plan_stage!(
            plan: load_layout,
            derive: derive_contents,
            contents: contents,
            fingerprint: non_authoritative_placed_compatibility_fingerprint,
            error: ElfSectionHeaderPlacementApplicationError,
        )
    }

    fn derive_contents(
        load_layout: &ValidatedElfDynamicLoadLayout,
    ) -> Result<ElfPlacedSectionHeaderContents, Diagnostic> {
        let template = load_layout
            .relative()
            .payloads()
            .section_headers()
            .contents();
        require(
            template.bytes.len() == SECTION_COUNT * ELF64_SECTION_HEADER_SIZE,
            "placed ELF section-header input is not the exact 832-byte template",
        )?;
        require(
            template.placement_fixups.len() == PLACEMENT_COUNT
                && load_layout.section_header_resolutions().len() == PLACEMENT_COUNT,
            "placed ELF section-header input does not have exactly twenty-five fixups and resolutions",
        )?;

        let mut bytes = template.bytes.clone();
        let mut applications = Vec::with_capacity(PLACEMENT_COUNT);
        for (fixup, resolution) in template
            .placement_fixups
            .iter()
            .zip(load_layout.section_header_resolutions())
        {
            let expected_kind = fixup.kind;
            require(
                fixup.row_index == resolution.row_index()
                    && public_section_kind(fixup.section_kind) == resolution.section_kind()
                    && fixup.byte_offset == resolution.byte_offset()
                    && fixup.byte_width == resolution.byte_width()
                    && expected_kind == resolution.kind(),
                "ELF section-header resolution does not match its upstream typed fixup",
            )?;
            require(
                fixup.byte_width == ELF64_PLACEMENT_FIELD_SIZE,
                "ELF section-header placement width is not eight bytes",
            )?;
            let field = field_mut(
                &mut bytes,
                fixup.byte_offset,
                fixup.byte_width,
                "section-header placement field",
            )?;
            require(
                field.iter().all(|byte| *byte == 0),
                "ELF section-header placement input is not an exact zero placeholder",
            )?;
            field.copy_from_slice(&resolution.value().to_le_bytes());
            applications.push(*resolution);
        }
        Ok(ElfPlacedSectionHeaderContents {
            bytes,
            applications,
        })
    }

    checked::seal_stage! {
        field: load_layout,
        contents: contents,
        validated: ValidatedElfPlacedSectionHeaderTable,
        validate: validate_contents,
        fingerprint: non_authoritative_placed_compatibility_fingerprint,
        nonzero,
        message: "placed ELF section-header compatibility fingerprint does not replay",
    }

    fn validate_contents(
        load_layout: &ValidatedElfDynamicLoadLayout,
        contents: &ElfPlacedSectionHeaderContents,
    ) -> Result<(), Diagnostic> {
        let template = load_layout
            .relative()
            .payloads()
            .section_headers()
            .contents();
        let roster = &load_layout
            .relative()
            .payloads()
            .section_headers()
            .roster()
            .contents()
            .rows;
        let resolutions = load_layout.section_header_resolutions();
        require(
            roster.len() == SECTION_COUNT
                && template.bytes.len() == SECTION_COUNT * ELF64_SECTION_HEADER_SIZE
                && contents.bytes.len() == SECTION_COUNT * ELF64_SECTION_HEADER_SIZE,
            "placed ELF section-header table does not retain exactly fourteen complete rows",
        )?;
        require(
            template.placement_fixups.len() == PLACEMENT_COUNT
                && resolutions.len() == PLACEMENT_COUNT
                && contents.applications.len() == PLACEMENT_COUNT,
            "placed ELF section-header ledger does not cover exactly twenty-five fields",
        )?;

        for ((fixup, resolution), application) in template
            .placement_fixups
            .iter()
            .zip(resolutions)
            .zip(&contents.applications)
        {
            let expected_kind = fixup.kind;
            require(
                fixup.row_index == resolution.row_index()
                    && public_section_kind(fixup.section_kind) == resolution.section_kind()
                    && fixup.byte_offset == resolution.byte_offset()
                    && fixup.byte_width == resolution.byte_width()
                    && expected_kind == resolution.kind(),
                "retained ELF section-header resolution drifted from its typed fixup",
            )?;
            require(
                application.row_index == resolution.row_index()
                    && application.section_kind == resolution.section_kind()
                    && application.byte_offset == resolution.byte_offset()
                    && application.byte_width == resolution.byte_width()
                    && application.kind == resolution.kind()
                    && application.value == resolution.value(),
                "applied ELF section-header placement drifted from its retained resolution",
            )?;
            require(
                application.byte_width == ELF64_PLACEMENT_FIELD_SIZE,
                "applied ELF section-header placement width is not eight bytes",
            )?;
            let expected_field_offset = match application.kind {
                ElfSectionPlacementResolutionKind::VirtualAddress => ELF64_SECTION_ADDRESS_OFFSET,
                ElfSectionPlacementResolutionKind::FileOffset => ELF64_SECTION_FILE_OFFSET,
            };
            let expected_offset = checked_sum(
                checked_product(
                    application.row_index as usize,
                    ELF64_SECTION_HEADER_SIZE,
                    "placed section-header row offset",
                )?,
                expected_field_offset,
                "placed section-header field offset",
            )?;
            require(
                application.byte_offset == expected_offset,
                "applied ELF section-header placement has a noncanonical coordinate",
            )?;
            require(
                read_u64(
                    &contents.bytes,
                    application.byte_offset,
                    "applied section-header placement value",
                )? == application.value,
                "applied ELF section-header bytes do not contain their resolved value",
            )?;
        }
        require(
            contents
                .applications
                .iter()
                .filter(|application| {
                    application.kind == ElfSectionPlacementResolutionKind::FileOffset
                })
                .count()
                == FILE_OFFSET_PLACEMENT_COUNT,
            "placed ELF section-header ledger does not contain exactly twelve file offsets",
        )?;
        require(
            contents
                .applications
                .iter()
                .filter(|application| {
                    application.kind == ElfSectionPlacementResolutionKind::VirtualAddress
                })
                .count()
                == VIRTUAL_ADDRESS_PLACEMENT_COUNT,
            "placed ELF section-header ledger does not contain exactly eleven virtual addresses",
        )?;
        validate_application_coverage(contents.bytes.len(), &contents.applications)?;

        let decoded = decode_section_header_rows(&contents.bytes, SECTION_COUNT)?;
        let mut address_by_row = [None; SECTION_COUNT];
        let mut offset_by_row = [None; SECTION_COUNT];
        for application in &contents.applications {
            let row = usize::try_from(application.row_index)
                .map_err(|_| Diagnostic::error("ELF section-header row index exceeds usize"))?;
            let slot = match application.kind {
                ElfSectionPlacementResolutionKind::VirtualAddress => &mut address_by_row[row],
                ElfSectionPlacementResolutionKind::FileOffset => &mut offset_by_row[row],
            };
            require(
                slot.replace(application.value).is_none(),
                "applied ELF section-header ledger duplicates a row field",
            )?;
        }
        require(
            address_by_row[0].is_none()
                && offset_by_row[0].is_none()
                && address_by_row[SECTION_NAME_TABLE_INDEX].is_none()
                && offset_by_row[SECTION_NAME_TABLE_INDEX].is_some(),
            "null or section-name-table placement semantics drifted",
        )?;
        for (ordinal, (decoded, descriptor)) in decoded.iter().zip(roster).enumerate() {
            validate_decoded_row(
                decoded,
                descriptor,
                address_by_row[ordinal].unwrap_or(0),
                offset_by_row[ordinal].unwrap_or(0),
            )?;
        }
        require(
            contents.bytes[..ELF64_SECTION_HEADER_SIZE] == [0; ELF64_SECTION_HEADER_SIZE],
            "placed ELF null section header is not exactly zero",
        )
    }

    fn validate_decoded_row(
        decoded: &DecodedElf64SectionHeader,
        descriptor: &ElfNumericSectionDescriptor,
        expected_address: u64,
        expected_offset: u64,
    ) -> Result<(), Diagnostic> {
        require(
            decoded.name_offset == descriptor.name_offset
                && decoded.section_type == descriptor.section_type
                && decoded.flags == descriptor.flags
                && decoded.address == expected_address
                && decoded.file_offset == expected_offset
                && decoded.payload_size == descriptor.payload_size
                && decoded.link == descriptor.link
                && decoded.info == descriptor.info
                && decoded.alignment == descriptor.alignment
                && decoded.entry_size == descriptor.entry_size,
            "decoded placed ELF section header drifted from its roster and resolutions",
        )
    }

    fn validate_application_coverage(
        byte_count: usize,
        applications: &[ElfAppliedSectionHeaderPlacement],
    ) -> Result<(), Diagnostic> {
        for (ordinal, application) in applications.iter().enumerate() {
            let end = checked_sum(
                application.byte_offset,
                usize::from(application.byte_width),
                "applied ELF section-header placement end",
            )?;
            require(
                end <= byte_count,
                "applied ELF section-header placement exceeds the table",
            )?;
            for other in &applications[ordinal + 1..] {
                let other_end = checked_sum(
                    other.byte_offset,
                    usize::from(other.byte_width),
                    "applied ELF section-header placement end",
                )?;
                require(
                    end <= other.byte_offset || other_end <= application.byte_offset,
                    "applied ELF section-header placements overlap or duplicate",
                )?;
            }
        }
        Ok(())
    }

    fn non_authoritative_placed_compatibility_fingerprint(
        load_layout: &ValidatedElfDynamicLoadLayout,
        contents: &ElfPlacedSectionHeaderContents,
    ) -> u64 {
        let mut hash = checked::stage_fingerprint(
            b"omega.elf.placed-section-header-table.v1",
            load_layout.non_authoritative_layout_compatibility_fingerprint(),
        );
        hash.bytes(&contents.bytes);
        hash.bytes(&(contents.applications.len() as u64).to_le_bytes());
        for application in &contents.applications {
            hash.bytes(&application.row_index.to_le_bytes());
            hash.byte(application.section_kind as u8);
            hash.bytes(&(application.byte_offset as u64).to_le_bytes());
            hash.byte(application.byte_width);
            hash.byte(application.kind as u8);
            hash.bytes(&application.value.to_le_bytes());
        }
        hash.finish()
    }

    #[cfg(test)]
    mod tests {
        use super::{
            Candidate, DecodedElf64SectionHeader, ELF64_SECTION_HEADER_SIZE,
            ElfAppliedSectionHeaderPlacement, ElfSectionPlacementResolutionKind, SECTION_COUNT,
            SECTION_NAME_TABLE_INDEX, ValidatedElfDynamicLoadLayout,
            apply_elf_section_header_placements, checked_product, checked_sum,
            decode_section_header_rows, derive_contents, field_mut,
            non_authoritative_placed_compatibility_fingerprint, validate_application_coverage,
            validate_candidate, validate_contents,
        };
        use crate::ElfPlacedDynamicSectionKind;
        use crate::dynamic_executable::checked::{read_u32, read_u64};
        use target::TargetProfile;

        fn load_layout(target: TargetProfile) -> ValidatedElfDynamicLoadLayout {
            let mut image = crate::test_support::import_image(target, 16, 1);
            let symbol_handle = crate::test_support::versioned_import(
                &mut image,
                "__omega_placed_section_header_import",
                b"libplaced-section-header.so",
                b"placed_section_header_call",
                b"PLACED_SECTION_HEADER_1",
                target,
                "versioned import locator",
            );

            crate::test_support::import_call_load_layout(image, symbol_handle, target)
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let load_layout = load_layout(target);
            let contents = derive_contents(&load_layout).unwrap();
            let non_authoritative_placed_compatibility_fingerprint =
                non_authoritative_placed_compatibility_fingerprint(&load_layout, &contents);
            Candidate {
                load_layout,
                contents,
                non_authoritative_placed_compatibility_fingerprint,
            }
        }

        #[test]
        fn both_linux_targets_apply_exact_twenty_five_little_endian_placements() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let placed = apply_elf_section_header_placements(load_layout(target)).unwrap();
                assert_eq!(placed.load_layout().target(), target);
                assert_eq!(placed.bytes().len(), 896);
                assert_eq!(placed.applied_placements().len(), 25);
                assert_ne!(
                    placed.non_authoritative_placed_compatibility_fingerprint(),
                    0
                );

                let template = placed
                    .load_layout()
                    .relative()
                    .payloads()
                    .section_headers()
                    .contents();
                assert_eq!(template.bytes.len(), placed.bytes().len());
                for (application, resolution) in placed
                    .applied_placements()
                    .iter()
                    .zip(placed.load_layout().section_header_resolutions())
                {
                    assert_eq!(application.row_index(), resolution.row_index());
                    assert_eq!(application.section_kind(), resolution.section_kind());
                    assert_eq!(application.byte_offset(), resolution.byte_offset());
                    assert_eq!(application.byte_width(), 8);
                    assert_eq!(application.kind(), resolution.kind());
                    assert_eq!(application.value(), resolution.value());
                    assert_eq!(
                        &placed.bytes()[application.byte_offset()..application.byte_offset() + 8],
                        &application.value().to_le_bytes(),
                    );
                    assert_eq!(
                        &template.bytes[application.byte_offset()..application.byte_offset() + 8],
                        &[0; 8],
                        "the retained upstream template must remain unchanged",
                    );
                }
            }
        }

        #[test]
        fn unchanged_template_bytes_are_copied_exactly() {
            let placed =
                apply_elf_section_header_placements(load_layout(TargetProfile::LinuxX64)).unwrap();
            let template = placed
                .load_layout()
                .relative()
                .payloads()
                .section_headers()
                .contents();
            let mut mutable = vec![false; placed.bytes().len()];
            for application in placed.applied_placements() {
                mutable[application.byte_offset()..application.byte_offset() + 8].fill(true);
            }
            for (offset, (&actual, &source)) in
                placed.bytes().iter().zip(&template.bytes).enumerate()
            {
                if !mutable[offset] {
                    assert_eq!(actual, source, "non-fixup byte {offset} drifted");
                }
            }
            validate_contents(placed.load_layout(), &placed.contents).unwrap();
        }

        #[test]
        fn ledger_has_exact_field_split_and_file_only_section_name_table() {
            let placed =
                apply_elf_section_header_placements(load_layout(TargetProfile::LinuxX64)).unwrap();
            assert_eq!(
                placed
                    .applied_placements()
                    .iter()
                    .filter(|application| {
                        application.kind() == ElfSectionPlacementResolutionKind::FileOffset
                    })
                    .count(),
                13,
            );
            assert_eq!(
                placed
                    .applied_placements()
                    .iter()
                    .filter(|application| {
                        application.kind() == ElfSectionPlacementResolutionKind::VirtualAddress
                    })
                    .count(),
                12,
            );
            assert!(
                placed
                    .applied_placements()
                    .iter()
                    .all(|application| application.row_index() != 0),
            );
            assert!(placed.applied_placements().iter().any(|application| {
                application.row_index() == SECTION_NAME_TABLE_INDEX as u32
                    && application.kind() == ElfSectionPlacementResolutionKind::FileOffset
            }));
            assert!(!placed.applied_placements().iter().any(|application| {
                application.row_index() == SECTION_NAME_TABLE_INDEX as u32
                    && application.kind() == ElfSectionPlacementResolutionKind::VirtualAddress
            }));
            let decoded = decode_section_header_rows(placed.bytes(), SECTION_COUNT).unwrap();
            assert_eq!(
                decoded[0],
                DecodedElf64SectionHeader {
                    name_offset: 0,
                    section_type: 0,
                    flags: 0,
                    address: 0,
                    file_offset: 0,
                    payload_size: 0,
                    link: 0,
                    info: 0,
                    alignment: 0,
                    entry_size: 0,
                }
            );
            assert_eq!(decoded[SECTION_NAME_TABLE_INDEX].address, 0);
            assert_ne!(decoded[SECTION_NAME_TABLE_INDEX].file_offset, 0);
        }

        #[test]
        fn placement_is_deterministic_for_same_input_and_target_bound() {
            let first =
                apply_elf_section_header_placements(load_layout(TargetProfile::LinuxX64)).unwrap();
            let second =
                apply_elf_section_header_placements(load_layout(TargetProfile::LinuxX64)).unwrap();
            let arm = apply_elf_section_header_placements(load_layout(TargetProfile::LinuxArm64))
                .unwrap();
            assert_eq!(first.bytes(), second.bytes());
            assert_eq!(first.applied_placements(), second.applied_placements());
            assert_eq!(
                first.non_authoritative_placed_compatibility_fingerprint(),
                second.non_authoritative_placed_compatibility_fingerprint()
            );
            assert_ne!(
                first.non_authoritative_placed_compatibility_fingerprint(),
                arm.non_authoritative_placed_compatibility_fingerprint()
            );
        }

        #[test]
        fn missing_reordered_width_coordinate_value_and_identity_drift_reject_with_custody() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| {
                    candidate.contents.applications.pop();
                }),
                Box::new(|candidate| candidate.contents.applications.swap(0, 1)),
                Box::new(|candidate| candidate.contents.applications[0].byte_width = 4),
                Box::new(|candidate| candidate.contents.applications[0].byte_offset += 1),
                Box::new(|candidate| candidate.contents.applications[0].value ^= 1),
                Box::new(|candidate| {
                    candidate.contents.applications[0].kind =
                        ElfSectionPlacementResolutionKind::FileOffset
                }),
                Box::new(|candidate| candidate.contents.applications[0].row_index = 2),
                Box::new(|candidate| {
                    candidate.non_authoritative_placed_compatibility_fingerprint ^= 1
                }),
            ];
            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxArm64);
                let expected_layout_identity = candidate
                    .load_layout
                    .non_authoritative_layout_compatibility_fingerprint();
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("drifted placement application must reject");
                assert_eq!(
                    error
                        .candidate
                        .load_layout
                        .non_authoritative_layout_compatibility_fingerprint(),
                    expected_layout_identity
                );
            }
        }

        #[test]
        fn resolved_field_corruption_and_non_fixup_roster_drift_reject() {
            let mut resolved = candidate(TargetProfile::LinuxX64);
            let first_offset = resolved.contents.applications[0].byte_offset;
            resolved.contents.bytes[first_offset] ^= 1;
            validate_candidate(resolved).expect_err("resolved field byte corruption must reject");

            let mut non_fixup = candidate(TargetProfile::LinuxX64);
            let first_payload_size = ELF64_SECTION_HEADER_SIZE + 32;
            assert!(non_fixup.contents.applications.iter().all(|application| {
                !(application.byte_offset..application.byte_offset + 8)
                    .contains(&first_payload_size)
            }));
            non_fixup.contents.bytes[first_payload_size] ^= 1;
            validate_candidate(non_fixup).expect_err("non-fixup roster byte drift must reject");

            let mut truncated = candidate(TargetProfile::LinuxX64);
            truncated.contents.bytes.pop();
            validate_candidate(truncated).expect_err("truncated table must reject");

            let mut trailing = candidate(TargetProfile::LinuxX64);
            trailing.contents.bytes.push(0);
            validate_candidate(trailing).expect_err("trailing table byte must reject");
        }

        #[test]
        fn bounds_helpers_reject_without_panicking() {
            assert!(checked_product(usize::MAX, 64, "product").is_err());
            assert!(checked_sum(usize::MAX, 8, "sum").is_err());
            assert!(read_u32(&[0; 3], 0, "word").is_err());
            assert!(read_u64(&[0; 7], 0, "xword").is_err());
            assert!(field_mut(&mut [0; 7], 0, 8, "test placement field").is_err());

            let application = ElfAppliedSectionHeaderPlacement {
                row_index: u32::MAX,
                section_kind: ElfPlacedDynamicSectionKind::Interpreter,
                byte_offset: usize::MAX,
                byte_width: 8,
                kind: ElfSectionPlacementResolutionKind::VirtualAddress,
                value: 1,
            };
            assert!(validate_application_coverage(768, &[application]).is_err());
        }
    }
}

pub(crate) mod resolved_dynamic_table {
    use super::super::checked::{self, CandidateValidationError};
    use crate::constants::{
        ELF64_DYNAMIC_ROW_SIZE, ELF64_DYNAMIC_VALUE_OFFSET, ELF64_DYNAMIC_VALUE_SIZE,
    };
    use crate::dynamic_executable::checked::{
        checked_product, checked_sum, decode_dynamic_rows, field_mut, read_u64, require,
    };
    use crate::dynamic_executable::dynamic_table::dynamic_tag_bytes::{
        ElfDynamicPayloadFixupKind, ValidatedElfDynamicTablePayload,
    };
    use crate::dynamic_executable::dynamic_table::dynamic_tags::{
        ElfDynamicAddressTarget, ElfDynamicTag, ElfDynamicValue, ValidatedElfDynamicTagPlan,
        encoded_value,
    };
    use crate::dynamic_executable::load_placement::load_layout::{
        ElfPlacedDynamicSectionKind, ValidatedElfDynamicLoadLayout,
    };
    use crate::dynamic_executable::load_placement::placed_section_headers::ValidatedElfPlacedSectionHeaderTable;
    use crate::dynamic_executable::section_headers::section_payload_roster::{
        ElfIndexedDynamicFixup, ValidatedElfIndexedSectionPayloadPlan,
    };
    use crate::dynamic_executable::section_headers::section_roster::ElfDynamicRosterSectionKind;
    use diagnostics::Diagnostic;

    const DYNAMIC_SECTION_INDEX: u32 = 12;
    const EXPECTED_TARGETS: [ElfDynamicAddressApplicationTarget; 9] = [
        ElfDynamicAddressApplicationTarget::ProcedureGot,
        ElfDynamicAddressApplicationTarget::SystemVHash,
        ElfDynamicAddressApplicationTarget::GnuHash,
        ElfDynamicAddressApplicationTarget::DynamicString,
        ElfDynamicAddressApplicationTarget::DynamicSymbol,
        ElfDynamicAddressApplicationTarget::ProcedureRelocation,
        ElfDynamicAddressApplicationTarget::GeneralRelocation,
        ElfDynamicAddressApplicationTarget::GnuSymbolVersion,
        ElfDynamicAddressApplicationTarget::GnuVersionRequirement,
    ];

    fn expected_targets(
        placed: &ValidatedElfPlacedSectionHeaderTable,
    ) -> &'static [ElfDynamicAddressApplicationTarget] {
        if semantic_plan(placed).has_version_requirements() {
            &EXPECTED_TARGETS
        } else {
            &EXPECTED_TARGETS[..EXPECTED_TARGETS.len() - 2]
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    #[cfg(test)]
    pub(crate) enum ElfDynamicAddressApplicationKind {
        Elf64AbsoluteAddress = 1,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub(crate) enum ElfDynamicAddressApplicationTarget {
        ProcedureGot = 1,
        SystemVHash = 2,
        DynamicString = 3,
        DynamicSymbol = 4,
        ProcedureRelocation = 5,
        GnuSymbolVersion = 6,
        GnuVersionRequirement = 7,
        GnuHash = 8,
        GeneralRelocation = 9,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfAppliedDynamicAddress {
        row_ordinal: u32,
        storage_section_index: u32,
        byte_offset: usize,
        byte_width: u8,
        kind_tag: u8,
        target: ElfDynamicAddressApplicationTarget,
        target_section_index: u32,
        target_section_kind: ElfPlacedDynamicSectionKind,
        value: u64,
    }

    impl ElfAppliedDynamicAddress {
        #[cfg(test)]
        pub const fn row_ordinal(&self) -> u32 {
            self.row_ordinal
        }

        #[cfg(test)]
        pub const fn storage_section_index(&self) -> u32 {
            self.storage_section_index
        }

        #[cfg(test)]
        pub const fn byte_offset(&self) -> usize {
            self.byte_offset
        }

        #[cfg(test)]
        pub const fn byte_width(&self) -> u8 {
            self.byte_width
        }

        #[cfg(test)]
        pub const fn kind(&self) -> ElfDynamicAddressApplicationKind {
            ElfDynamicAddressApplicationKind::Elf64AbsoluteAddress
        }

        #[cfg(test)]
        pub const fn target(&self) -> ElfDynamicAddressApplicationTarget {
            self.target
        }

        #[cfg(test)]
        pub const fn target_section_index(&self) -> u32 {
            self.target_section_index
        }

        #[cfg(test)]
        pub const fn target_section_kind(&self) -> ElfPlacedDynamicSectionKind {
            self.target_section_kind
        }

        #[cfg(test)]
        pub const fn value(&self) -> u64 {
            self.value
        }
    }

    #[derive(Debug)]
    #[must_use = "resolved ELF dynamic bytes retain placed-section and load-layout custody"]
    pub struct ValidatedElfResolvedDynamicTable {
        placed_section_headers: ValidatedElfPlacedSectionHeaderTable,
        contents: ElfResolvedDynamicTableContents,
        non_authoritative_resolved_compatibility_fingerprint: u64,
    }

    impl ValidatedElfResolvedDynamicTable {
        pub const fn placed_section_headers(&self) -> &ValidatedElfPlacedSectionHeaderTable {
            &self.placed_section_headers
        }

        pub fn bytes(&self) -> &[u8] {
            &self.contents.bytes
        }

        #[cfg(test)]
        pub(crate) fn applied_addresses(&self) -> &[ElfAppliedDynamicAddress] {
            &self.contents.applications
        }

        pub const fn non_authoritative_resolved_compatibility_fingerprint(&self) -> u64 {
            self.non_authoritative_resolved_compatibility_fingerprint
        }

        pub(crate) fn into_parts(
            self,
        ) -> (
            ValidatedElfPlacedSectionHeaderTable,
            ElfResolvedDynamicTableContents,
        ) {
            (self.placed_section_headers, self.contents)
        }
    }

    #[derive(Debug)]
    #[must_use = "dynamic-address rejection retains the placed section-header owner"]
    pub struct ElfDynamicAddressApplicationError {
        placed_section_headers: ValidatedElfPlacedSectionHeaderTable,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(
        ElfDynamicAddressApplicationError;
        placed_section_headers: ValidatedElfPlacedSectionHeaderTable,
    );

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfResolvedDynamicTableContents {
        pub(crate) bytes: Vec<u8>,
        pub(crate) applications: Vec<ElfAppliedDynamicAddress>,
    }

    struct Candidate {
        placed_section_headers: ValidatedElfPlacedSectionHeaderTable,
        contents: ElfResolvedDynamicTableContents,
        non_authoritative_resolved_compatibility_fingerprint: u64,
    }

    pub(crate) fn apply_elf_dynamic_address_fixups(
        placed_section_headers: ValidatedElfPlacedSectionHeaderTable,
    ) -> Result<ValidatedElfResolvedDynamicTable, Box<ElfDynamicAddressApplicationError>> {
        checked::plan_stage!(
            plan: placed_section_headers,
            derive: derive_contents,
            contents: contents,
            fingerprint: non_authoritative_resolved_compatibility_fingerprint,
            error: ElfDynamicAddressApplicationError,
        )
    }

    fn derive_contents(
        placed: &ValidatedElfPlacedSectionHeaderTable,
    ) -> Result<ElfResolvedDynamicTableContents, Diagnostic> {
        let indexed = indexed_payloads(placed);
        let source = dynamic_row(indexed)?;
        let expected_targets = expected_targets(placed);
        require(
            indexed.contents().dynamic_fixups.len() == expected_targets.len(),
            "internal ELF dynamic application requires the exact version-aware indexed fixups",
        )?;
        let mut bytes = source.bytes.clone();
        let mut applications = Vec::with_capacity(expected_targets.len());
        for (fixup, expected_target) in indexed
            .contents()
            .dynamic_fixups
            .iter()
            .zip(expected_targets.iter().copied())
        {
            let application = application_for_fixup(placed.load_layout(), fixup)?;
            require(
                application.target == expected_target,
                "indexed ELF dynamic address targets are not in canonical order",
            )?;
            let field = field_mut(
                &mut bytes,
                application.byte_offset,
                application.byte_width,
                "dynamic address field",
            )?;
            require(
                field.iter().all(|byte| *byte == 0),
                "indexed ELF dynamic address is not an exact zero placeholder",
            )?;
            field.copy_from_slice(&application.value.to_le_bytes());
            applications.push(application);
        }
        Ok(ElfResolvedDynamicTableContents {
            bytes,
            applications,
        })
    }

    fn application_for_fixup(
        load_layout: &ValidatedElfDynamicLoadLayout,
        fixup: &ElfIndexedDynamicFixup,
    ) -> Result<ElfAppliedDynamicAddress, Diagnostic> {
        require(
            fixup.storage_section_index == DYNAMIC_SECTION_INDEX,
            "indexed ELF dynamic fixup names the wrong storage section",
        )?;
        require(
            fixup.byte_width == ELF64_DYNAMIC_VALUE_SIZE
                && fixup.kind == ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress,
            "indexed ELF dynamic fixup is not an eight-byte absolute address",
        )?;
        let target = public_target(fixup.target);
        let target_kind = target_section_kind(target);
        let expected_index = target_section_index(target);
        require(
            fixup.target_section_index == expected_index,
            "indexed ELF dynamic fixup names the wrong target section index",
        )?;
        let section = load_layout
            .sections()
            .get(expected_index as usize)
            .filter(|section| {
                section.index() == expected_index
                    && section.kind() == target_kind
                    && section.virtual_address().is_some()
            })
            .ok_or_else(|| {
                Diagnostic::error("indexed ELF dynamic target has no exact allocated placement")
            })?;
        Ok(ElfAppliedDynamicAddress {
            row_ordinal: fixup.row_ordinal,
            storage_section_index: fixup.storage_section_index,
            byte_offset: fixup.byte_offset,
            byte_width: fixup.byte_width,
            kind_tag: fixup.kind as u8,
            target,
            target_section_index: fixup.target_section_index,
            target_section_kind: target_kind,
            value: section
                .virtual_address()
                .expect("filtered allocated ELF dynamic target"),
        })
    }

    checked::seal_stage! {
        field: placed_section_headers,
        contents: contents,
        validated: ValidatedElfResolvedDynamicTable,
        validate: validate_contents,
        fingerprint: non_authoritative_resolved_compatibility_fingerprint,
        nonzero,
        message: "resolved ELF dynamic-table compatibility fingerprint does not replay",
    }

    fn validate_contents(
        placed: &ValidatedElfPlacedSectionHeaderTable,
        contents: &ElfResolvedDynamicTableContents,
    ) -> Result<(), Diagnostic> {
        let indexed = indexed_payloads(placed);
        let source = dynamic_row(indexed)?;
        let expected_targets = expected_targets(placed);
        let fixups = &indexed.contents().dynamic_fixups;
        let serialized_fixups = &dynamic_payload(placed).contents().address_fixups;
        let semantic = semantic_plan(placed).contents();
        require(
            fixups.len() == expected_targets.len()
                && serialized_fixups.len() == expected_targets.len()
                && semantic.address_obligations.len() == expected_targets.len()
                && contents.applications.len() == expected_targets.len(),
            "resolved ELF dynamic table does not retain the exact version-aware applications",
        )?;
        require(
            contents.bytes.len() == source.bytes.len(),
            "resolved ELF dynamic table length drifted from indexed storage",
        )?;

        let mut mutable = vec![false; source.bytes.len()];
        for ((((fixup, serialized), expected_target), application), obligation) in fixups
            .iter()
            .zip(serialized_fixups)
            .zip(expected_targets.iter().copied())
            .zip(&contents.applications)
            .zip(&semantic.address_obligations)
        {
            require(
                fixup.row_ordinal == serialized.row_ordinal
                    && fixup.byte_offset == serialized.byte_offset
                    && fixup.byte_width == serialized.byte_width
                    && fixup.kind == serialized.kind
                    && fixup.target == serialized.target,
                "indexed ELF dynamic fixup drifted from its serialized typed fixup",
            )?;
            let expected = application_for_fixup(placed.load_layout(), fixup)?;
            require(
                expected.target == expected_target
                    && application.row_ordinal == expected.row_ordinal
                    && application.storage_section_index == expected.storage_section_index
                    && application.byte_offset == expected.byte_offset
                    && application.byte_width == expected.byte_width
                    && application.kind_tag == expected.kind_tag
                    && application.target == expected.target
                    && application.target_section_index == expected.target_section_index
                    && application.target_section_kind == expected.target_section_kind
                    && application.value == expected.value,
                "resolved ELF dynamic application drifted from its indexed fixup or placed target",
            )?;
            require(
                application.row_ordinal == obligation.row_ordinal
                    && application.byte_width == obligation.byte_width
                    && application.target == public_target(obligation.target),
                "resolved ELF dynamic application drifted from its semantic address obligation",
            )?;
            let semantic_row = semantic
                .rows
                .get(application.row_ordinal as usize)
                .ok_or_else(|| {
                    Diagnostic::error("resolved ELF dynamic application row is missing")
                })?;
            require(
                semantic_row.tag == target_tag(application.target)
                    && semantic_row.value == ElfDynamicValue::AddressPlaceholder,
                "resolved ELF dynamic target does not match its exact semantic tag row",
            )?;
            let expected_offset = checked_sum(
                checked_product(
                    application.row_ordinal as usize,
                    ELF64_DYNAMIC_ROW_SIZE,
                    "resolved Elf64_Dyn row offset",
                )?,
                ELF64_DYNAMIC_VALUE_OFFSET,
                "resolved Elf64_Dyn value offset",
            )?;
            require(
                application.storage_section_index == DYNAMIC_SECTION_INDEX
                    && application.byte_offset == expected_offset
                    && application.byte_width == ELF64_DYNAMIC_VALUE_SIZE
                    && application.kind_tag
                        == ElfDynamicPayloadFixupKind::Elf64AbsoluteAddress as u8,
                "resolved ELF dynamic application has the wrong storage, coordinate, width, or kind",
            )?;
            let end = checked_sum(
                application.byte_offset,
                usize::from(application.byte_width),
                "resolved ELF dynamic application end",
            )?;
            require(
                end <= mutable.len(),
                "resolved ELF dynamic application exceeds its payload",
            )?;
            require(
                mutable[application.byte_offset..end]
                    .iter()
                    .all(|covered| !*covered),
                "resolved ELF dynamic applications overlap or duplicate one field",
            )?;
            mutable[application.byte_offset..end].fill(true);
            require(
                read_u64(
                    &source.bytes,
                    application.byte_offset,
                    "indexed Elf64_Dyn address placeholder",
                )? == 0,
                "indexed Elf64_Dyn address field is not an exact zero placeholder",
            )?;
            require(
                read_u64(
                    &contents.bytes,
                    application.byte_offset,
                    "resolved Elf64_Dyn address value",
                )? == application.value,
                "resolved Elf64_Dyn bytes do not contain the placed target address",
            )?;
        }
        for (offset, (actual, upstream)) in contents.bytes.iter().zip(&source.bytes).enumerate() {
            if !mutable[offset] {
                require(
                    actual == upstream,
                    "non-fixup Elf64_Dyn byte drifted from indexed storage",
                )?;
            }
        }

        let decoded = decode_dynamic_rows(&contents.bytes, semantic.rows.len())?;
        for (ordinal, (decoded, row)) in decoded.iter().zip(&semantic.rows).enumerate() {
            let application = contents
                .applications
                .iter()
                .find(|application| application.row_ordinal as usize == ordinal);
            let expected_value = match row.value {
                ElfDynamicValue::AddressPlaceholder => {
                    application
                        .ok_or_else(|| {
                            Diagnostic::error("resolved Elf64_Dyn row is missing its address")
                        })?
                        .value
                }
                value => {
                    require(
                        application.is_none(),
                        "literal Elf64_Dyn row has an orphan address application",
                    )?;
                    encoded_value(value)
                }
            };
            require(
                decoded.tag == row.tag as i64 && decoded.value == expected_value,
                "decoded resolved Elf64_Dyn row drifted from semantic order or value",
            )?;
        }
        Ok(())
    }

    fn indexed_payloads(
        placed: &ValidatedElfPlacedSectionHeaderTable,
    ) -> &ValidatedElfIndexedSectionPayloadPlan {
        placed.load_layout().relative().payloads()
    }

    fn dynamic_row(
        indexed: &ValidatedElfIndexedSectionPayloadPlan,
    ) -> Result<&crate::dynamic_executable::section_headers::section_payload_roster::ElfIndexedSectionPayloadRow, Diagnostic>{
        indexed
            .contents()
            .rows
            .get(DYNAMIC_SECTION_INDEX as usize)
            .filter(|row| {
                row.index == DYNAMIC_SECTION_INDEX
                    && row.kind == ElfDynamicRosterSectionKind::DynamicTable
            })
            .ok_or_else(|| Diagnostic::error("indexed ELF .dynamic storage row is missing"))
    }

    fn dynamic_payload(
        placed: &ValidatedElfPlacedSectionHeaderTable,
    ) -> &ValidatedElfDynamicTablePayload {
        placed
            .load_layout()
            .relative()
            .payloads()
            .section_headers()
            .roster()
            .section_names()
            .dynamic_table()
            .payload()
    }

    fn semantic_plan(placed: &ValidatedElfPlacedSectionHeaderTable) -> &ValidatedElfDynamicTagPlan {
        dynamic_payload(placed).plan()
    }

    const fn public_target(target: ElfDynamicAddressTarget) -> ElfDynamicAddressApplicationTarget {
        target
    }

    pub(crate) const fn target_section_index(target: ElfDynamicAddressApplicationTarget) -> u32 {
        match target {
            ElfDynamicAddressApplicationTarget::ProcedureGot => 9,
            ElfDynamicAddressApplicationTarget::SystemVHash => 4,
            ElfDynamicAddressApplicationTarget::GnuHash => 7,
            ElfDynamicAddressApplicationTarget::DynamicString => 2,
            ElfDynamicAddressApplicationTarget::DynamicSymbol => 3,
            ElfDynamicAddressApplicationTarget::ProcedureRelocation => 10,
            ElfDynamicAddressApplicationTarget::GnuSymbolVersion => 5,
            ElfDynamicAddressApplicationTarget::GnuVersionRequirement => 6,
            ElfDynamicAddressApplicationTarget::GeneralRelocation => 11,
        }
    }

    const fn target_section_kind(
        target: ElfDynamicAddressApplicationTarget,
    ) -> ElfPlacedDynamicSectionKind {
        match target {
            ElfDynamicAddressApplicationTarget::ProcedureGot => {
                ElfPlacedDynamicSectionKind::ProcedureGot
            }
            ElfDynamicAddressApplicationTarget::SystemVHash => {
                ElfPlacedDynamicSectionKind::SystemVHash
            }
            ElfDynamicAddressApplicationTarget::GnuHash => ElfPlacedDynamicSectionKind::GnuHash,
            ElfDynamicAddressApplicationTarget::DynamicString => {
                ElfPlacedDynamicSectionKind::DynamicString
            }
            ElfDynamicAddressApplicationTarget::DynamicSymbol => {
                ElfPlacedDynamicSectionKind::DynamicSymbol
            }
            ElfDynamicAddressApplicationTarget::ProcedureRelocation => {
                ElfPlacedDynamicSectionKind::ProcedureRelocation
            }
            ElfDynamicAddressApplicationTarget::GnuSymbolVersion => {
                ElfPlacedDynamicSectionKind::GnuSymbolVersion
            }
            ElfDynamicAddressApplicationTarget::GnuVersionRequirement => {
                ElfPlacedDynamicSectionKind::GnuVersionRequirement
            }
            ElfDynamicAddressApplicationTarget::GeneralRelocation => {
                ElfPlacedDynamicSectionKind::GeneralRelocation
            }
        }
    }

    const fn target_tag(target: ElfDynamicAddressApplicationTarget) -> ElfDynamicTag {
        match target {
            ElfDynamicAddressApplicationTarget::ProcedureGot => ElfDynamicTag::ProcedureGot,
            ElfDynamicAddressApplicationTarget::SystemVHash => ElfDynamicTag::SystemVHash,
            ElfDynamicAddressApplicationTarget::GnuHash => ElfDynamicTag::GnuHash,
            ElfDynamicAddressApplicationTarget::DynamicString => ElfDynamicTag::DynamicString,
            ElfDynamicAddressApplicationTarget::DynamicSymbol => ElfDynamicTag::DynamicSymbol,
            ElfDynamicAddressApplicationTarget::ProcedureRelocation => {
                ElfDynamicTag::ProcedureRelocation
            }
            ElfDynamicAddressApplicationTarget::GnuSymbolVersion => ElfDynamicTag::GnuSymbolVersion,
            ElfDynamicAddressApplicationTarget::GnuVersionRequirement => {
                ElfDynamicTag::GnuVersionRequirement
            }
            ElfDynamicAddressApplicationTarget::GeneralRelocation => ElfDynamicTag::Rela,
        }
    }

    fn non_authoritative_resolved_compatibility_fingerprint(
        placed: &ValidatedElfPlacedSectionHeaderTable,
        contents: &ElfResolvedDynamicTableContents,
    ) -> u64 {
        let mut hash = checked::stage_fingerprint(
            b"omega.elf.resolved-dynamic-table.v1",
            placed.non_authoritative_placed_compatibility_fingerprint(),
        );
        hash.bytes(&contents.bytes);
        hash.bytes(&(contents.applications.len() as u64).to_le_bytes());
        for application in &contents.applications {
            hash.bytes(&application.row_ordinal.to_le_bytes());
            hash.bytes(&application.storage_section_index.to_le_bytes());
            hash.bytes(&(application.byte_offset as u64).to_le_bytes());
            hash.byte(application.byte_width);
            hash.byte(application.kind_tag);
            hash.byte(application.target as u8);
            hash.bytes(&application.target_section_index.to_le_bytes());
            hash.byte(application.target_section_kind as u8);
            hash.bytes(&application.value.to_le_bytes());
        }
        hash.finish()
    }

    #[cfg(test)]
    mod tests {
        use super::{
            Candidate, EXPECTED_TARGETS, ElfAppliedDynamicAddress,
            ElfDynamicAddressApplicationKind, ElfDynamicAddressApplicationTarget,
            ElfPlacedDynamicSectionKind, ValidatedElfPlacedSectionHeaderTable,
            apply_elf_dynamic_address_fixups, checked_product, checked_sum, decode_dynamic_rows,
            derive_contents, dynamic_row, field_mut, indexed_payloads,
            non_authoritative_resolved_compatibility_fingerprint, read_u64, validate_candidate,
        };
        use crate::apply_elf_section_header_placements;
        use crate::dynamic_executable::checked::read_i64;
        use target::TargetProfile;

        fn placed(
            target: TargetProfile,
            imported_symbol: &[u8],
        ) -> ValidatedElfPlacedSectionHeaderTable {
            let mut image = crate::test_support::import_image(target, 16, 1);
            let symbol_handle = crate::test_support::versioned_import(
                &mut image,
                "__omega_resolved_dynamic_import",
                b"libresolved-dynamic.so",
                imported_symbol,
                b"RESOLVED_DYNAMIC_1",
                target,
                "versioned import locator",
            );
            let load = crate::test_support::import_call_load_layout(image, symbol_handle, target);
            apply_elf_section_header_placements(load).unwrap()
        }

        fn standard_placed(target: TargetProfile) -> ValidatedElfPlacedSectionHeaderTable {
            placed(target, b"resolved_dynamic_call")
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let placed_section_headers = standard_placed(target);
            let contents = derive_contents(&placed_section_headers).unwrap();
            let non_authoritative_resolved_compatibility_fingerprint =
                non_authoritative_resolved_compatibility_fingerprint(
                    &placed_section_headers,
                    &contents,
                );
            Candidate {
                placed_section_headers,
                contents,
                non_authoritative_resolved_compatibility_fingerprint,
            }
        }

        fn write_application_value(candidate: &mut Candidate, ordinal: usize) {
            let application = candidate.contents.applications[ordinal];
            candidate.contents.bytes[application.byte_offset
                ..application.byte_offset + usize::from(application.byte_width)]
                .copy_from_slice(&application.value.to_le_bytes());
        }

        #[test]
        fn both_linux_targets_apply_exact_nine_target_virtual_addresses() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let resolved = apply_elf_dynamic_address_fixups(standard_placed(target)).unwrap();
                assert_eq!(resolved.applied_addresses().len(), 9);
                assert_ne!(
                    resolved.non_authoritative_resolved_compatibility_fingerprint(),
                    0
                );
                assert_eq!(
                    resolved
                        .applied_addresses()
                        .iter()
                        .map(ElfAppliedDynamicAddress::target)
                        .collect::<Vec<_>>(),
                    EXPECTED_TARGETS,
                );
                let layout = resolved.placed_section_headers().load_layout();
                for application in resolved.applied_addresses() {
                    assert_eq!(application.storage_section_index(), 12);
                    assert_eq!(application.byte_width(), 8);
                    assert_eq!(
                        application.kind(),
                        ElfDynamicAddressApplicationKind::Elf64AbsoluteAddress
                    );
                    assert_eq!(
                        application.byte_offset(),
                        application.row_ordinal() as usize * 16 + 8
                    );
                    let target_section =
                        &layout.sections()[application.target_section_index() as usize];
                    assert_eq!(target_section.kind(), application.target_section_kind());
                    assert_eq!(target_section.virtual_address(), Some(application.value()));
                    assert_eq!(
                        read_u64(
                            resolved.bytes(),
                            application.byte_offset(),
                            "test resolved address"
                        )
                        .unwrap(),
                        application.value(),
                    );
                }
            }
        }

        #[test]
        fn exactly_seventy_two_mutable_bytes_change_and_every_other_byte_is_preserved() {
            let resolved =
                apply_elf_dynamic_address_fixups(standard_placed(TargetProfile::LinuxX64)).unwrap();
            let indexed = indexed_payloads(resolved.placed_section_headers());
            let source = dynamic_row(indexed).unwrap();
            let mut mutable = vec![false; source.bytes.len()];
            for application in resolved.applied_addresses() {
                let end = application.byte_offset() + usize::from(application.byte_width());
                mutable[application.byte_offset()..end].fill(true);
            }
            assert_eq!(mutable.iter().filter(|byte| **byte).count(), 72);
            for (offset, (&actual, &upstream)) in
                resolved.bytes().iter().zip(&source.bytes).enumerate()
            {
                if mutable[offset] {
                    assert_eq!(upstream, 0);
                } else {
                    assert_eq!(actual, upstream, "non-fixup byte {offset} changed");
                }
            }
        }

        #[test]
        fn resolution_is_deterministic_and_binds_target_and_selected_input() {
            let first =
                apply_elf_dynamic_address_fixups(standard_placed(TargetProfile::LinuxX64)).unwrap();
            let second =
                apply_elf_dynamic_address_fixups(standard_placed(TargetProfile::LinuxX64)).unwrap();
            let arm = apply_elf_dynamic_address_fixups(standard_placed(TargetProfile::LinuxArm64))
                .unwrap();
            let other = apply_elf_dynamic_address_fixups(placed(
                TargetProfile::LinuxX64,
                b"resolved_dynamic_peer",
            ))
            .unwrap();
            assert_eq!(first.bytes(), second.bytes());
            assert_eq!(first.applied_addresses(), second.applied_addresses());
            assert_eq!(
                first.non_authoritative_resolved_compatibility_fingerprint(),
                second.non_authoritative_resolved_compatibility_fingerprint()
            );
            assert_ne!(
                first.non_authoritative_resolved_compatibility_fingerprint(),
                arm.non_authoritative_resolved_compatibility_fingerprint()
            );
            assert_ne!(
                first.non_authoritative_resolved_compatibility_fingerprint(),
                other.non_authoritative_resolved_compatibility_fingerprint()
            );
        }

        #[test]
        fn missing_duplicate_reordered_and_ledger_field_drift_reject_with_custody() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| {
                    candidate.contents.applications.pop();
                }),
                Box::new(|candidate| {
                    candidate
                        .contents
                        .applications
                        .push(candidate.contents.applications[0])
                }),
                Box::new(|candidate| candidate.contents.applications.swap(0, 1)),
                Box::new(|candidate| candidate.contents.applications[0].row_ordinal += 1),
                Box::new(|candidate| candidate.contents.applications[0].byte_offset += 16),
                Box::new(|candidate| candidate.contents.applications[0].byte_width = 4),
                Box::new(|candidate| candidate.contents.applications[0].kind_tag ^= 1),
                Box::new(|candidate| candidate.contents.applications[0].storage_section_index = 9),
                Box::new(|candidate| {
                    candidate.contents.applications[0].target =
                        ElfDynamicAddressApplicationTarget::SystemVHash
                }),
                Box::new(|candidate| candidate.contents.applications[0].target_section_index = 4),
                Box::new(|candidate| {
                    candidate.contents.applications[0].target_section_kind =
                        ElfPlacedDynamicSectionKind::SystemVHash
                }),
                Box::new(|candidate| candidate.contents.applications[0].value ^= 1),
                Box::new(|candidate| {
                    candidate.non_authoritative_resolved_compatibility_fingerprint = 0
                }),
                Box::new(|candidate| {
                    candidate.non_authoritative_resolved_compatibility_fingerprint ^= 1
                }),
            ];
            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxArm64);
                let expected_custody = candidate
                    .placed_section_headers
                    .non_authoritative_placed_compatibility_fingerprint();
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("corrupt resolved dynamic ledger must reject");
                assert_eq!(
                    error
                        .candidate
                        .placed_section_headers
                        .non_authoritative_placed_compatibility_fingerprint(),
                    expected_custody
                );
            }
        }

        #[test]
        fn valid_sibling_file_only_and_null_target_substitution_reject() {
            let mut sibling = candidate(TargetProfile::LinuxX64);
            let hash = sibling.placed_section_headers.load_layout().sections()[4];
            sibling.contents.applications[0].target =
                ElfDynamicAddressApplicationTarget::SystemVHash;
            sibling.contents.applications[0].target_section_index = 4;
            sibling.contents.applications[0].target_section_kind =
                ElfPlacedDynamicSectionKind::SystemVHash;
            sibling.contents.applications[0].value = hash.virtual_address().unwrap();
            write_application_value(&mut sibling, 0);
            validate_candidate(sibling).expect_err("valid sibling target substitution must reject");

            let mut file_only = candidate(TargetProfile::LinuxX64);
            let shstrtab = file_only.placed_section_headers.load_layout().sections()[12];
            file_only.contents.applications[0].target_section_index = 12;
            file_only.contents.applications[0].target_section_kind =
                ElfPlacedDynamicSectionKind::SectionNameTable;
            file_only.contents.applications[0].value = shstrtab.file_offset();
            write_application_value(&mut file_only, 0);
            validate_candidate(file_only).expect_err("file-only target substitution must reject");

            let mut null = candidate(TargetProfile::LinuxX64);
            null.contents.applications[0].target_section_index = 0;
            null.contents.applications[0].target_section_kind = ElfPlacedDynamicSectionKind::Null;
            null.contents.applications[0].value = 0;
            write_application_value(&mut null, 0);
            validate_candidate(null).expect_err("null target substitution must reject");
        }

        #[test]
        fn byte_corruption_wrong_endian_truncation_and_trailing_data_reject() {
            let mut applied = candidate(TargetProfile::LinuxX64);
            let offset = applied.contents.applications[0].byte_offset;
            applied.contents.bytes[offset] ^= 1;
            validate_candidate(applied).expect_err("applied-byte/ledger disagreement must reject");

            let mut wrong_endian = candidate(TargetProfile::LinuxX64);
            let offset = wrong_endian.contents.applications[0].byte_offset;
            wrong_endian.contents.bytes[offset..offset + 8].reverse();
            validate_candidate(wrong_endian).expect_err("big-endian address write must reject");

            let mut tag = candidate(TargetProfile::LinuxX64);
            tag.contents.bytes[0] ^= 1;
            validate_candidate(tag).expect_err("dynamic tag drift must reject");

            let mut literal = candidate(TargetProfile::LinuxX64);
            literal.contents.bytes[8] ^= 1;
            validate_candidate(literal).expect_err("non-fixup literal drift must reject");

            let mut null = candidate(TargetProfile::LinuxX64);
            let last = null.contents.bytes.len() - 1;
            null.contents.bytes[last] ^= 1;
            validate_candidate(null).expect_err("final null row drift must reject");

            let mut truncated = candidate(TargetProfile::LinuxX64);
            truncated.contents.bytes.pop();
            validate_candidate(truncated).expect_err("truncated dynamic payload must reject");

            let mut trailing = candidate(TargetProfile::LinuxX64);
            trailing.contents.bytes.push(0);
            validate_candidate(trailing).expect_err("trailing dynamic payload data must reject");
        }

        #[test]
        fn bounds_helpers_reject_without_panicking() {
            assert!(checked_product(usize::MAX, 16, "product").is_err());
            assert!(checked_sum(usize::MAX, 8, "sum").is_err());
            assert!(read_i64(&[0; 7], 0, "sxword").is_err());
            assert!(read_u64(&[0; 7], 0, "xword").is_err());
            assert!(field_mut(&mut [0; 7], 0, 8, "test address field").is_err());
            assert!(decode_dynamic_rows(&[], usize::MAX).is_err());
        }
    }
}
