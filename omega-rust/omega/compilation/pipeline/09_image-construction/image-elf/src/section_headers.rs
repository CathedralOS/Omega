/*
We execute stages 11 through 15 of the dynamic ELF pipeline: planning the
`.shstrtab` section-name table (`plan_elf_section_name_table`), assembling the
complete section roster (`plan_elf_dynamic_section_roster`), serializing the
`Elf64_Shdr` template (`serialize_elf_section_header_table`), indexing section
payloads (`plan_elf_indexed_section_payloads`), and computing relative section
offsets (`plan_elf_relative_section_payload_layout`).
*/

pub(crate) mod relative_section_layout {
    use super::super::checked::{self, CandidateValidationError};
    use crate::dynamic_executable::checked::{checked_sum_u64, require};
    use crate::dynamic_executable::dynamic_table::dynamic_table_descriptor::ValidatedElfDynamicTableSectionDescriptorPlan;
    use crate::dynamic_executable::section_headers::section_payload_roster::ValidatedElfIndexedSectionPayloadPlan;
    use crate::dynamic_executable::section_headers::section_roster::ElfDynamicRosterSectionKind;
    use diagnostics::Diagnostic;

    use crate::constants::{SECTION_COUNT, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE};
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub enum ElfRelativeSectionPayloadRegion {
        ReadOnly = 1,
        ReadExecute = 2,
        ReadWrite = 3,
        FileOnly = 4,
    }

    #[derive(Debug)]
    #[must_use = "validated relative ELF layout retains the indexed payload plan"]
    pub struct ValidatedElfRelativeSectionPayloadLayout {
        payloads: ValidatedElfIndexedSectionPayloadPlan,
        contents: ElfRelativeSectionPayloadLayoutContents,
        non_authoritative_layout_compatibility_fingerprint: u64,
    }

    impl ValidatedElfRelativeSectionPayloadLayout {
        pub const fn payloads(&self) -> &ValidatedElfIndexedSectionPayloadPlan {
            &self.payloads
        }

        pub(crate) fn dynamic_table_plan(&self) -> &ValidatedElfDynamicTableSectionDescriptorPlan {
            self.payloads()
                .section_headers()
                .roster()
                .section_names()
                .dynamic_table()
        }

        pub fn row_count(&self) -> usize {
            self.contents.rows.len()
        }

        pub const fn region_span(&self, region: ElfRelativeSectionPayloadRegion) -> u64 {
            self.contents.region_spans.get(region)
        }

        pub const fn non_authoritative_layout_compatibility_fingerprint(&self) -> u64 {
            self.non_authoritative_layout_compatibility_fingerprint
        }

        pub(crate) const fn contents(&self) -> &ElfRelativeSectionPayloadLayoutContents {
            &self.contents
        }

        pub(crate) fn into_parts(
            self,
        ) -> (
            ValidatedElfIndexedSectionPayloadPlan,
            ElfRelativeSectionPayloadLayoutContents,
        ) {
            (self.payloads, self.contents)
        }
    }

    #[derive(Debug)]
    #[must_use = "relative ELF layout rejection retains the indexed payload plan"]
    pub struct ElfRelativeSectionPayloadLayoutError {
        payloads: ValidatedElfIndexedSectionPayloadPlan,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(
        ElfRelativeSectionPayloadLayoutError;
        payloads: ValidatedElfIndexedSectionPayloadPlan,
    );

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfRelativeSectionPayloadLayoutContents {
        pub(crate) rows: Vec<ElfRelativeSectionPayloadPlacement>,
        pub(crate) region_spans: ElfRelativeSectionPayloadRegionSpans,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfRelativeSectionPayloadPlacement {
        pub(crate) index: u32,
        pub(crate) kind: ElfDynamicRosterSectionKind,
        pub(crate) region: Option<ElfRelativeSectionPayloadRegion>,
        pub(crate) relative_file_offset: u64,
        pub(crate) byte_size: u64,
        pub(crate) alignment: u64,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub(crate) struct ElfRelativeSectionPayloadRegionSpans {
        pub(crate) read_only: u64,
        pub(crate) read_execute: u64,
        pub(crate) read_write: u64,
        pub(crate) file_only: u64,
    }

    impl ElfRelativeSectionPayloadRegionSpans {
        const fn get(self, region: ElfRelativeSectionPayloadRegion) -> u64 {
            match region {
                ElfRelativeSectionPayloadRegion::ReadOnly => self.read_only,
                ElfRelativeSectionPayloadRegion::ReadExecute => self.read_execute,
                ElfRelativeSectionPayloadRegion::ReadWrite => self.read_write,
                ElfRelativeSectionPayloadRegion::FileOnly => self.file_only,
            }
        }

        fn cursor_mut(&mut self, region: ElfRelativeSectionPayloadRegion) -> &mut u64 {
            match region {
                ElfRelativeSectionPayloadRegion::ReadOnly => &mut self.read_only,
                ElfRelativeSectionPayloadRegion::ReadExecute => &mut self.read_execute,
                ElfRelativeSectionPayloadRegion::ReadWrite => &mut self.read_write,
                ElfRelativeSectionPayloadRegion::FileOnly => &mut self.file_only,
            }
        }
    }

    struct Candidate {
        payloads: ValidatedElfIndexedSectionPayloadPlan,
        contents: ElfRelativeSectionPayloadLayoutContents,
        non_authoritative_layout_compatibility_fingerprint: u64,
    }

    pub(crate) fn plan_elf_relative_section_payload_layout(
        payloads: ValidatedElfIndexedSectionPayloadPlan,
    ) -> Result<ValidatedElfRelativeSectionPayloadLayout, Box<ElfRelativeSectionPayloadLayoutError>>
    {
        checked::plan_stage!(
            plan: payloads,
            derive: derive_contents,
            contents: contents,
            fingerprint: non_authoritative_layout_compatibility_fingerprint,
            error: ElfRelativeSectionPayloadLayoutError,
        )
    }

    fn derive_contents(
        payloads: &ValidatedElfIndexedSectionPayloadPlan,
    ) -> Result<ElfRelativeSectionPayloadLayoutContents, Diagnostic> {
        let payload_rows = &payloads.contents().rows;
        let roster_rows = &payloads.section_headers().roster().contents().rows;
        require(
            payload_rows.len() == SECTION_COUNT && roster_rows.len() == SECTION_COUNT,
            "relative ELF layout requires the exact fourteen-row payload roster",
        )?;

        let mut rows = Vec::with_capacity(SECTION_COUNT);
        let mut region_spans = ElfRelativeSectionPayloadRegionSpans::default();
        for ordinal in physical_section_order() {
            let payload = &payload_rows[ordinal];
            let descriptor = &roster_rows[ordinal];
            let byte_size = u64::try_from(payload.bytes.len())
                .map_err(|_| Diagnostic::error("ELF payload size exceeds Elf64_Xword"))?;
            let (region, relative_file_offset) = if ordinal == 0 {
                require(
                    payload.kind == ElfDynamicRosterSectionKind::Null
                        && byte_size == 0
                        && descriptor.alignment == 0,
                    "relative ELF layout null row is not canonical",
                )?;
                (None, 0)
            } else {
                let region = classify_region(descriptor.flags)?;
                let cursor = region_spans.cursor_mut(region);
                let offset =
                    checked_align(*cursor, descriptor.alignment, "relative ELF payload offset")?;
                *cursor =
                    checked_sum_u64(offset, byte_size, "relative ELF payload end", "Elf64_Off")?;
                (Some(region), offset)
            };
            rows.push(ElfRelativeSectionPayloadPlacement {
                index: payload.index,
                kind: payload.kind,
                region,
                relative_file_offset,
                byte_size,
                alignment: descriptor.alignment,
            });
        }
        rows.sort_unstable_by_key(|row| row.index);
        Ok(ElfRelativeSectionPayloadLayoutContents { rows, region_spans })
    }

    checked::seal_stage! {
        field: payloads,
        contents: contents,
        validated: ValidatedElfRelativeSectionPayloadLayout,
        validate: validate_contents,
        fingerprint: non_authoritative_layout_compatibility_fingerprint,
        message: "relative ELF payload-layout compatibility fingerprint does not replay",
    }

    fn validate_contents(
        payloads: &ValidatedElfIndexedSectionPayloadPlan,
        contents: &ElfRelativeSectionPayloadLayoutContents,
    ) -> Result<(), Diagnostic> {
        let payload_rows = &payloads.contents().rows;
        let roster_rows = &payloads.section_headers().roster().contents().rows;
        require(
            payload_rows.len() == SECTION_COUNT
                && roster_rows.len() == SECTION_COUNT
                && contents.rows.len() == SECTION_COUNT,
            "relative ELF layout does not contain exactly fourteen rows",
        )?;

        let mut region_spans = ElfRelativeSectionPayloadRegionSpans::default();
        for ordinal in physical_section_order() {
            let placement = &contents.rows[ordinal];
            let payload = &payload_rows[ordinal];
            let descriptor = &roster_rows[ordinal];
            let byte_size = u64::try_from(payload.bytes.len())
                .map_err(|_| Diagnostic::error("replayed ELF payload size exceeds Elf64_Xword"))?;
            let (expected_region, expected_offset) = if ordinal == 0 {
                require(
                    payload.kind == ElfDynamicRosterSectionKind::Null
                        && byte_size == 0
                        && descriptor.alignment == 0,
                    "replayed relative ELF null row is not canonical",
                )?;
                (None, 0)
            } else {
                let region = classify_region(descriptor.flags)?;
                let cursor = region_spans.cursor_mut(region);
                let offset = checked_align(
                    *cursor,
                    descriptor.alignment,
                    "replayed relative ELF offset",
                )?;
                *cursor = checked_sum_u64(
                    offset,
                    byte_size,
                    "replayed relative ELF payload end",
                    "Elf64_Off",
                )?;
                (Some(region), offset)
            };
            require(
                placement.index == payload.index
                    && placement.index == descriptor.index
                    && placement.kind == payload.kind
                    && placement.kind == descriptor.kind
                    && placement.region == expected_region
                    && placement.relative_file_offset == expected_offset
                    && placement.byte_size == byte_size
                    && placement.byte_size == descriptor.payload_size
                    && placement.alignment == descriptor.alignment,
                "relative ELF payload placement drifted from its exact roster row",
            )?;
        }
        require(
            contents.region_spans == region_spans,
            "relative ELF region spans do not exactly cover the packed rows",
        )
    }

    fn physical_section_order() -> [usize; SECTION_COUNT] {
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 10, 12, 13]
    }

    fn classify_region(flags: u64) -> Result<ElfRelativeSectionPayloadRegion, Diagnostic> {
        match (
            flags & SHF_ALLOC != 0,
            flags & SHF_WRITE != 0,
            flags & SHF_EXECINSTR != 0,
        ) {
            (false, false, false) => Ok(ElfRelativeSectionPayloadRegion::FileOnly),
            (true, false, false) => Ok(ElfRelativeSectionPayloadRegion::ReadOnly),
            (true, false, true) => Ok(ElfRelativeSectionPayloadRegion::ReadExecute),
            (true, true, false) => Ok(ElfRelativeSectionPayloadRegion::ReadWrite),
            _ => Err(Diagnostic::error(
                "relative ELF layout rejects write/execute flags without allocation or writable executable sections",
            )),
        }
    }

    fn checked_align(value: u64, alignment: u64, context: &'static str) -> Result<u64, Diagnostic> {
        let alignment = alignment.max(1);
        let remainder = value % alignment;
        if remainder == 0 {
            Ok(value)
        } else {
            checked_sum_u64(value, alignment - remainder, context, "Elf64_Off")
        }
    }

    fn non_authoritative_layout_compatibility_fingerprint(
        payloads: &ValidatedElfIndexedSectionPayloadPlan,
        contents: &ElfRelativeSectionPayloadLayoutContents,
    ) -> u64 {
        let mut hash = checked::stage_fingerprint(
            b"omega.elf-relative-section-payload-layout.v1",
            payloads.non_authoritative_payload_roster_compatibility_fingerprint(),
        );
        for row in &contents.rows {
            hash.bytes(&row.index.to_le_bytes());
            hash.byte(row.kind as u8);
            hash.byte(row.region.map_or(0, |region| region as u8));
            hash.bytes(&row.relative_file_offset.to_le_bytes());
            hash.bytes(&row.byte_size.to_le_bytes());
            hash.bytes(&row.alignment.to_le_bytes());
        }
        hash.bytes(&contents.region_spans.read_only.to_le_bytes());
        hash.bytes(&contents.region_spans.read_execute.to_le_bytes());
        hash.bytes(&contents.region_spans.read_write.to_le_bytes());
        hash.bytes(&contents.region_spans.file_only.to_le_bytes());
        hash.finish()
    }

    #[cfg(test)]
    mod tests {
        use super::{
            Candidate, ElfDynamicRosterSectionKind, ElfRelativeSectionPayloadRegion,
            ElfRelativeSectionPayloadRegionSpans, SECTION_COUNT, SHF_ALLOC, SHF_EXECINSTR,
            SHF_WRITE, ValidatedElfIndexedSectionPayloadPlan, checked_align, checked_sum_u64,
            classify_region, derive_contents, non_authoritative_layout_compatibility_fingerprint,
            plan_elf_relative_section_payload_layout, validate_candidate, validate_contents,
        };

        use arena::Handle;
        use image::{FinalImage, FinalImageMemory};
        use target::TargetProfile;

        fn payloads(target: TargetProfile) -> ValidatedElfIndexedSectionPayloadPlan {
            let mut image = FinalImage::with_capacity(
                target.native_target(),
                FinalImageMemory {
                    text: vec![0; 32],
                    ..FinalImageMemory::default()
                },
                Handle::invalid(),
                1,
                1,
                1,
            );
            let symbol_handle = crate::test_support::versioned_import(
                &mut image,
                "__omega_relative_layout_import",
                b"liblayout.so",
                b"layout_call",
                b"LAYOUT_1",
                target,
                "versioned import locator",
            );
            crate::test_support::import_call_indexed_payloads(image, symbol_handle, target)
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let payloads = payloads(target);
            let contents = derive_contents(&payloads).unwrap();
            let non_authoritative_layout_compatibility_fingerprint =
                non_authoritative_layout_compatibility_fingerprint(&payloads, &contents);
            Candidate {
                payloads,
                contents,
                non_authoritative_layout_compatibility_fingerprint,
            }
        }

        #[test]
        fn both_targets_pack_exact_aligned_rows_without_absolute_authority() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let layout = plan_elf_relative_section_payload_layout(payloads(target)).unwrap();
                assert_eq!(layout.row_count(), SECTION_COUNT);
                assert_ne!(
                    layout.non_authoritative_layout_compatibility_fingerprint(),
                    0
                );
                assert_eq!(layout.contents.rows[0].relative_file_offset, 0);
                assert_eq!(layout.contents.rows[0].byte_size, 0);
                assert_eq!(layout.contents.rows[0].region, None);
                assert!(
                    layout.contents.rows[1..=7].iter().all(|row| {
                        row.region == Some(ElfRelativeSectionPayloadRegion::ReadOnly)
                    })
                );
                assert_eq!(
                    layout.contents.rows[8].region,
                    Some(ElfRelativeSectionPayloadRegion::ReadExecute)
                );
                assert_eq!(
                    layout.contents.rows[9].region,
                    Some(ElfRelativeSectionPayloadRegion::ReadWrite)
                );
                assert_eq!(
                    layout.contents.rows[10].region,
                    Some(ElfRelativeSectionPayloadRegion::ReadOnly)
                );
                assert_eq!(
                    layout.contents.rows[11].region,
                    Some(ElfRelativeSectionPayloadRegion::ReadOnly)
                );
                assert_eq!(
                    layout.contents.rows[12].region,
                    Some(ElfRelativeSectionPayloadRegion::ReadWrite)
                );
                assert_eq!(
                    layout.contents.rows[13].region,
                    Some(ElfRelativeSectionPayloadRegion::FileOnly)
                );
                for region in [
                    ElfRelativeSectionPayloadRegion::ReadOnly,
                    ElfRelativeSectionPayloadRegion::ReadExecute,
                    ElfRelativeSectionPayloadRegion::ReadWrite,
                    ElfRelativeSectionPayloadRegion::FileOnly,
                ] {
                    assert_ne!(layout.region_span(region), 0);
                }
                let mut prior_ends = ElfRelativeSectionPayloadRegionSpans::default();
                for ordinal in super::physical_section_order().into_iter().skip(1) {
                    let row = &layout.contents.rows[ordinal];
                    let region = row.region.expect("non-null row must select one region");
                    let prior_end = prior_ends.cursor_mut(region);
                    assert!(*prior_end <= row.relative_file_offset);
                    *prior_end = row.relative_file_offset.checked_add(row.byte_size).unwrap();
                    assert_eq!(
                        row.relative_file_offset % row.alignment.max(1),
                        0,
                        "section {} must satisfy retained sh_addralign",
                        row.index,
                    );
                }
                validate_contents(layout.payloads(), &layout.contents).unwrap();
            }
        }

        #[test]
        fn relative_layout_is_deterministic_and_target_bound() {
            let first = plan_elf_relative_section_payload_layout(payloads(TargetProfile::LinuxX64))
                .unwrap();
            let second =
                plan_elf_relative_section_payload_layout(payloads(TargetProfile::LinuxX64))
                    .unwrap();
            let arm = plan_elf_relative_section_payload_layout(payloads(TargetProfile::LinuxArm64))
                .unwrap();
            assert_eq!(first.contents, second.contents);
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
        fn row_geometry_span_and_identity_drift_reject_with_payload_custody() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|c| {
                    c.contents.rows.pop();
                }),
                Box::new(|c| c.contents.rows.push(c.contents.rows[0])),
                Box::new(|c| c.contents.rows.swap(1, 2)),
                Box::new(|c| c.contents.rows[1].index = u32::MAX),
                Box::new(|c| c.contents.rows[1].kind = ElfDynamicRosterSectionKind::DynamicString),
                Box::new(|c| {
                    c.contents.rows[1].region = Some(ElfRelativeSectionPayloadRegion::ReadWrite)
                }),
                Box::new(|c| c.contents.rows[1].relative_file_offset += 1),
                Box::new(|c| c.contents.rows[1].byte_size += 1),
                Box::new(|c| c.contents.rows[1].alignment += 1),
                Box::new(|c| c.contents.region_spans.read_only += 1),
                Box::new(|c| c.contents.region_spans.read_execute += 1),
                Box::new(|c| c.contents.region_spans.read_write += 1),
                Box::new(|c| c.contents.region_spans.file_only += 1),
                Box::new(|c| c.non_authoritative_layout_compatibility_fingerprint ^= 1),
            ];
            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxX64);
                let non_authoritative_payload_compatibility_fingerprint = candidate
                    .payloads
                    .non_authoritative_payload_roster_compatibility_fingerprint();
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("relative layout corruption must reject fail closed");
                assert_eq!(
                    error
                        .candidate
                        .payloads
                        .non_authoritative_payload_roster_compatibility_fingerprint(),
                    non_authoritative_payload_compatibility_fingerprint
                );
            }
        }

        #[test]
        fn arithmetic_overflow_rejects_without_panicking() {
            assert!(checked_sum_u64(u64::MAX, 1, "sum", "Elf64_Off").is_err());
            assert!(checked_align(u64::MAX, 8, "alignment").is_err());
            assert_eq!(checked_align(7, 0, "zero alignment").unwrap(), 7);
            assert!(classify_region(SHF_WRITE).is_err());
            assert!(classify_region(SHF_EXECINSTR).is_err());
            assert!(classify_region(SHF_ALLOC | SHF_WRITE | SHF_EXECINSTR).is_err());
            assert_eq!(
                classify_region(SHF_ALLOC | (1 << 63)).unwrap(),
                ElfRelativeSectionPayloadRegion::ReadOnly
            );
        }
    }
}

pub(crate) mod section_header_bytes {
    use super::super::checked::{self, CandidateValidationError};
    use crate::bytes::{write_u32, write_u64};
    use crate::constants::{ELF64_SECTION_HEADER_SIZE, SECTION_COUNT, SHF_ALLOC};
    use crate::dynamic_executable::checked::{
        checked_product, checked_sum, decode_section_header_rows, read_u64, require,
    };
    use crate::dynamic_executable::section_headers::section_roster::{
        ElfDynamicRosterSectionKind, ElfNumericSectionDescriptor, ValidatedElfDynamicSectionRoster,
    };
    use diagnostics::Diagnostic;

    const ELF64_SECTION_ADDRESS_OFFSET: usize = 16;
    const ELF64_SECTION_FILE_OFFSET: usize = 24;
    const ELF64_PLACEMENT_FIELD_SIZE: u8 = 8;
    const FILE_OFFSET_FIXUP_COUNT: usize = 13;
    const VIRTUAL_ADDRESS_FIXUP_COUNT: usize = 12;
    const PLACEMENT_FIXUP_COUNT: usize = FILE_OFFSET_FIXUP_COUNT + VIRTUAL_ADDRESS_FIXUP_COUNT;
    #[derive(Debug)]
    #[must_use = "validated ELF section-header bytes retain the numeric roster"]
    pub struct ValidatedElfSectionHeaderTableTemplate {
        roster: ValidatedElfDynamicSectionRoster,
        contents: ElfSectionHeaderTableTemplateContents,
        non_authoritative_template_compatibility_fingerprint: u64,
    }

    impl ValidatedElfSectionHeaderTableTemplate {
        pub const fn roster(&self) -> &ValidatedElfDynamicSectionRoster {
            &self.roster
        }

        pub const fn row_count(&self) -> usize {
            SECTION_COUNT
        }

        pub fn byte_count(&self) -> usize {
            self.contents.bytes.len()
        }

        #[cfg(test)]
        pub(crate) fn placement_fixup_count(&self) -> usize {
            self.contents.placement_fixups.len()
        }

        pub const fn non_authoritative_template_compatibility_fingerprint(&self) -> u64 {
            self.non_authoritative_template_compatibility_fingerprint
        }

        pub(crate) const fn contents(&self) -> &ElfSectionHeaderTableTemplateContents {
            &self.contents
        }

        pub(crate) fn into_parts(
            self,
        ) -> (
            ValidatedElfDynamicSectionRoster,
            ElfSectionHeaderTableTemplateContents,
        ) {
            (self.roster, self.contents)
        }
    }

    #[derive(Debug)]
    #[must_use = "ELF section-header serialization rejection retains the numeric roster"]
    pub struct ElfSectionHeaderTableSerializationError {
        roster: ValidatedElfDynamicSectionRoster,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(
        ElfSectionHeaderTableSerializationError;
        roster: ValidatedElfDynamicSectionRoster,
    );

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfSectionHeaderTableTemplateContents {
        pub(crate) bytes: Vec<u8>,
        pub(crate) placement_fixups: Vec<ElfSectionHeaderPlacementFixup>,
    }

    pub(crate) type ElfSectionHeaderPlacementFixupKind = crate::ElfSectionPlacementResolutionKind;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfSectionHeaderPlacementFixup {
        pub(crate) row_index: u32,
        pub(crate) section_kind: ElfDynamicRosterSectionKind,
        pub(crate) byte_offset: usize,
        pub(crate) byte_width: u8,
        pub(crate) kind: ElfSectionHeaderPlacementFixupKind,
    }

    struct Candidate {
        roster: ValidatedElfDynamicSectionRoster,
        contents: ElfSectionHeaderTableTemplateContents,
        non_authoritative_template_compatibility_fingerprint: u64,
    }

    pub(crate) fn serialize_elf_section_header_table(
        roster: ValidatedElfDynamicSectionRoster,
    ) -> Result<ValidatedElfSectionHeaderTableTemplate, Box<ElfSectionHeaderTableSerializationError>>
    {
        checked::plan_stage!(
            plan: roster,
            derive: encode_contents,
            contents: contents,
            fingerprint: non_authoritative_template_compatibility_fingerprint,
            error: ElfSectionHeaderTableSerializationError,
        )
    }

    fn encode_contents(
        roster: &ValidatedElfDynamicSectionRoster,
    ) -> Result<ElfSectionHeaderTableTemplateContents, Diagnostic> {
        let rows = &roster.contents().rows;
        let mut bytes = Vec::with_capacity(checked_product(
            rows.len(),
            ELF64_SECTION_HEADER_SIZE,
            "ELF64 section-header table size",
        )?);
        let mut placement_fixups = Vec::with_capacity(PLACEMENT_FIXUP_COUNT);
        for row in rows {
            write_u32(&mut bytes, row.name_offset);
            write_u32(&mut bytes, row.section_type);
            write_u64(&mut bytes, row.flags);
            write_u64(&mut bytes, 0);
            write_u64(&mut bytes, 0);
            write_u64(&mut bytes, row.payload_size);
            write_u32(&mut bytes, row.link);
            write_u32(&mut bytes, row.info);
            write_u64(&mut bytes, row.alignment);
            write_u64(&mut bytes, row.entry_size);

            if row.index != 0 {
                if row.flags & SHF_ALLOC != 0 {
                    placement_fixups.push(placement_fixup(
                        row,
                        ELF64_SECTION_ADDRESS_OFFSET,
                        ElfSectionHeaderPlacementFixupKind::VirtualAddress,
                    )?);
                }
                placement_fixups.push(placement_fixup(
                    row,
                    ELF64_SECTION_FILE_OFFSET,
                    ElfSectionHeaderPlacementFixupKind::FileOffset,
                )?);
            }
        }
        Ok(ElfSectionHeaderTableTemplateContents {
            bytes,
            placement_fixups,
        })
    }

    fn placement_fixup(
        row: &ElfNumericSectionDescriptor,
        field_offset: usize,
        kind: ElfSectionHeaderPlacementFixupKind,
    ) -> Result<ElfSectionHeaderPlacementFixup, Diagnostic> {
        let row_offset = checked_product(
            row.index as usize,
            ELF64_SECTION_HEADER_SIZE,
            "ELF64 section-header fixup row offset",
        )?;
        Ok(ElfSectionHeaderPlacementFixup {
            row_index: row.index,
            section_kind: row.kind,
            byte_offset: checked_sum(
                row_offset,
                field_offset,
                "ELF64 section-header fixup field offset",
            )?,
            byte_width: ELF64_PLACEMENT_FIELD_SIZE,
            kind,
        })
    }

    checked::seal_stage! {
        field: roster,
        contents: contents,
        validated: ValidatedElfSectionHeaderTableTemplate,
        validate: validate_contents,
        fingerprint: non_authoritative_template_compatibility_fingerprint,
        message: "ELF section-header template compatibility fingerprint does not replay",
    }

    fn validate_contents(
        roster: &ValidatedElfDynamicSectionRoster,
        contents: &ElfSectionHeaderTableTemplateContents,
    ) -> Result<(), Diagnostic> {
        require(
            roster.section_count() == SECTION_COUNT,
            "ELF section-header serialization requires the exact thirteen-row roster",
        )?;
        let decoded = decode_section_header_rows(&contents.bytes, roster.section_count())?;
        require(
            decoded.len() == SECTION_COUNT,
            "decoded ELF section-header row count is not exact",
        )?;
        for (decoded, roster_row) in decoded.iter().zip(&roster.contents().rows) {
            require(
                decoded.name_offset == roster_row.name_offset
                    && decoded.section_type == roster_row.section_type
                    && decoded.flags == roster_row.flags
                    && decoded.address == 0
                    && decoded.file_offset == 0
                    && decoded.payload_size == roster_row.payload_size
                    && decoded.link == roster_row.link
                    && decoded.info == roster_row.info
                    && decoded.alignment == roster_row.alignment
                    && decoded.entry_size == roster_row.entry_size,
                "decoded ELF64 section header drifted from its numeric roster row",
            )?;
        }
        require(
            contents.bytes.get(..ELF64_SECTION_HEADER_SIZE) == Some(&[0; 64]),
            "ELF null section header is not exactly 64 zero bytes",
        )?;
        validate_fixups(
            &contents.bytes,
            &roster.contents().rows,
            &contents.placement_fixups,
        )
    }

    fn validate_fixups(
        bytes: &[u8],
        rows: &[ElfNumericSectionDescriptor],
        fixups: &[ElfSectionHeaderPlacementFixup],
    ) -> Result<(), Diagnostic> {
        require(
            fixups.len() == PLACEMENT_FIXUP_COUNT,
            "ELF section-header placement-fixup count is not exactly twenty-three",
        )?;
        let allocated_count = rows
            .iter()
            .filter(|row| row.index != 0 && row.flags & SHF_ALLOC != 0)
            .count();
        require(
            allocated_count == VIRTUAL_ADDRESS_FIXUP_COUNT,
            "ELF section-header roster does not own exactly eleven allocated rows",
        )?;

        let mut expected_ordinal = 0usize;
        for row in rows {
            let expected = if row.index == 0 {
                [None, None]
            } else if row.flags & SHF_ALLOC != 0 {
                [
                    Some(ElfSectionHeaderPlacementFixupKind::VirtualAddress),
                    Some(ElfSectionHeaderPlacementFixupKind::FileOffset),
                ]
            } else {
                [Some(ElfSectionHeaderPlacementFixupKind::FileOffset), None]
            };
            for kind in expected.into_iter().flatten() {
                let fixup = fixups.get(expected_ordinal).ok_or_else(|| {
                    Diagnostic::error("ELF section-header placement fixup is missing")
                })?;
                let field_offset = match kind {
                    ElfSectionHeaderPlacementFixupKind::VirtualAddress => {
                        ELF64_SECTION_ADDRESS_OFFSET
                    }
                    ElfSectionHeaderPlacementFixupKind::FileOffset => ELF64_SECTION_FILE_OFFSET,
                };
                let row_offset = checked_product(
                    row.index as usize,
                    ELF64_SECTION_HEADER_SIZE,
                    "replayed section-header fixup row",
                )?;
                let byte_offset = checked_sum(
                    row_offset,
                    field_offset,
                    "replayed section-header fixup field",
                )?;
                require(
                    fixup.row_index == row.index
                        && fixup.section_kind == row.kind
                        && fixup.byte_offset == byte_offset
                        && fixup.byte_width == ELF64_PLACEMENT_FIELD_SIZE
                        && fixup.kind == kind,
                    "ELF section-header placement fixup drifted from its exact roster field",
                )?;
                require(
                    read_u64(
                        bytes,
                        fixup.byte_offset,
                        "section-header placement placeholder",
                    )? == 0,
                    "ELF section-header placement field is not an exact zero placeholder",
                )?;
                expected_ordinal = checked_sum(
                    expected_ordinal,
                    1,
                    "section-header placement-fixup ordinal",
                )?;
            }
        }
        require(
            expected_ordinal == fixups.len(),
            "ELF section-header placement fixups contain an orphan row",
        )?;
        validate_fixup_coverage(bytes.len(), fixups)
    }

    fn validate_fixup_coverage(
        byte_count: usize,
        fixups: &[ElfSectionHeaderPlacementFixup],
    ) -> Result<(), Diagnostic> {
        for (index, fixup) in fixups.iter().enumerate() {
            let end = checked_sum(
                fixup.byte_offset,
                usize::from(fixup.byte_width),
                "section-header placement-fixup end",
            )?;
            require(
                end <= byte_count,
                "section-header placement fixup exceeds the serialized table",
            )?;
            for other in &fixups[index + 1..] {
                let other_end = checked_sum(
                    other.byte_offset,
                    usize::from(other.byte_width),
                    "section-header placement-fixup end",
                )?;
                require(
                    end <= other.byte_offset || other_end <= fixup.byte_offset,
                    "section-header placement fixups overlap or duplicate a field",
                )?;
            }
        }
        Ok(())
    }

    fn non_authoritative_template_compatibility_fingerprint(
        roster: &ValidatedElfDynamicSectionRoster,
        contents: &ElfSectionHeaderTableTemplateContents,
    ) -> u64 {
        let mut hash = checked::stage_fingerprint(
            b"omega.elf-section-header-table-template.v1",
            roster.non_authoritative_roster_compatibility_fingerprint(),
        );
        hash.bytes(&contents.bytes);
        hash.bytes(&(contents.placement_fixups.len() as u64).to_le_bytes());
        for fixup in &contents.placement_fixups {
            hash.bytes(&fixup.row_index.to_le_bytes());
            hash.byte(fixup.section_kind as u8);
            hash.bytes(&(fixup.byte_offset as u64).to_le_bytes());
            hash.byte(fixup.byte_width);
            hash.byte(fixup.kind as u8);
        }
        hash.finish()
    }

    #[cfg(test)]
    mod tests {
        use super::{
            Candidate, ELF64_SECTION_HEADER_SIZE, ElfDynamicRosterSectionKind,
            ElfSectionHeaderPlacementFixup, ElfSectionHeaderPlacementFixupKind, SECTION_COUNT,
            ValidatedElfDynamicSectionRoster, checked_product, checked_sum, encode_contents,
            non_authoritative_template_compatibility_fingerprint,
            serialize_elf_section_header_table, validate_candidate, validate_contents,
            validate_fixup_coverage,
        };
        use crate::dynamic_executable::checked::{decode_section_header_rows, read_u32, read_u64};
        use crate::plan_elf_dynamic_section_roster;
        use target::TargetProfile;

        const IMPORTS: [crate::test_support::DynamicImportFixture; 2] =
            crate::test_support::PAIR_IMPORTS;

        use crate::test_support::DynamicImportFixture as ImportFixture;
        fn roster(
            target: TargetProfile,
            imports: &[ImportFixture],
        ) -> ValidatedElfDynamicSectionRoster {
            let inputs = crate::test_support::dynamic_import_inputs(
                target,
                imports,
                "__omega_section_header_import_",
                "valid section-header locator",
                "valid section-header interpreter",
            );
            let names = crate::test_support::dynamic_section_names(inputs);
            plan_elf_dynamic_section_roster(names).expect("valid section roster")
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let roster = roster(target, &IMPORTS);
            let contents = encode_contents(&roster).expect("encoded section-header table");
            let non_authoritative_template_compatibility_fingerprint =
                non_authoritative_template_compatibility_fingerprint(&roster, &contents);
            Candidate {
                roster,
                contents,
                non_authoritative_template_compatibility_fingerprint,
            }
        }

        fn row(bytes: &[u8], index: usize) -> &[u8] {
            let offset = index * ELF64_SECTION_HEADER_SIZE;
            &bytes[offset..offset + ELF64_SECTION_HEADER_SIZE]
        }

        #[test]
        fn both_targets_serialize_exact_rows_and_twenty_five_zero_fixups() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let template = serialize_elf_section_header_table(roster(target, &IMPORTS))
                    .expect("validated section-header template");
                assert_eq!(template.row_count(), 14);
                assert_eq!(template.byte_count(), 896);
                assert_eq!(template.placement_fixup_count(), 25);
                assert_eq!(row(&template.contents.bytes, 0), &[0; 64]);

                let decoded = decode_section_header_rows(&template.contents.bytes, 14).unwrap();
                assert_eq!(decoded[3].name_offset, 17);
                assert_eq!(decoded[3].section_type, 11);
                assert_eq!(decoded[3].flags, 2);
                assert_eq!(decoded[3].payload_size, 72);
                assert_eq!((decoded[3].link, decoded[3].info), (2, 1));
                assert_eq!((decoded[3].alignment, decoded[3].entry_size), (8, 24));
                assert_eq!((decoded[7].section_type, decoded[7].link), (0x6fff_fff6, 3));
                assert_eq!((decoded[10].link, decoded[10].info), (3, 9));
                assert_eq!((decoded[11].link, decoded[11].info), (3, 0));
                assert_eq!(decoded[12].link, 2);
                assert_eq!(decoded[13].name_offset, 59);
                assert_eq!(decoded[13].payload_size, 122);
                assert_eq!((decoded[13].address, decoded[13].file_offset), (0, 0));
                assert_ne!(
                    template.non_authoritative_template_compatibility_fingerprint(),
                    0
                );
                validate_contents(template.roster(), &template.contents).unwrap();
            }
        }

        #[test]
        fn exact_fixup_coordinates_cover_only_owned_placement_fields() {
            let template =
                serialize_elf_section_header_table(roster(TargetProfile::LinuxX64, &IMPORTS))
                    .unwrap();
            assert_eq!(
                template.contents.placement_fixups[0],
                ElfSectionHeaderPlacementFixup {
                    row_index: 1,
                    section_kind: ElfDynamicRosterSectionKind::Interpreter,
                    byte_offset: 80,
                    byte_width: 8,
                    kind: ElfSectionHeaderPlacementFixupKind::VirtualAddress,
                },
            );
            assert_eq!(template.contents.placement_fixups[1].byte_offset, 88);
            assert_eq!(
                template.contents.placement_fixups[20],
                ElfSectionHeaderPlacementFixup {
                    row_index: 11,
                    section_kind: ElfDynamicRosterSectionKind::GeneralRelocation,
                    byte_offset: 720,
                    byte_width: 8,
                    kind: ElfSectionHeaderPlacementFixupKind::VirtualAddress,
                },
            );
            assert_eq!(template.contents.placement_fixups[21].byte_offset, 728);
            assert_eq!(
                template.contents.placement_fixups[22],
                ElfSectionHeaderPlacementFixup {
                    row_index: 12,
                    section_kind: ElfDynamicRosterSectionKind::DynamicTable,
                    byte_offset: 784,
                    byte_width: 8,
                    kind: ElfSectionHeaderPlacementFixupKind::VirtualAddress,
                },
            );
            assert_eq!(template.contents.placement_fixups[23].byte_offset, 792);
            assert_eq!(
                template.contents.placement_fixups[24],
                ElfSectionHeaderPlacementFixup {
                    row_index: 13,
                    section_kind: ElfDynamicRosterSectionKind::SectionNameTable,
                    byte_offset: 856,
                    byte_width: 8,
                    kind: ElfSectionHeaderPlacementFixupKind::FileOffset,
                },
            );
            assert_eq!(
                template
                    .contents
                    .placement_fixups
                    .iter()
                    .filter(|fixup| {
                        fixup.kind == ElfSectionHeaderPlacementFixupKind::VirtualAddress
                    })
                    .count(),
                12,
            );
            assert!(template.contents.placement_fixups.iter().all(|fixup| {
                read_u64(
                    &template.contents.bytes,
                    fixup.byte_offset,
                    "test placeholder",
                ) == Ok(0)
            }));
        }

        #[test]
        fn import_permutation_preserves_templates_and_target_remains_identity_bound() {
            let forward =
                serialize_elf_section_header_table(roster(TargetProfile::LinuxX64, &IMPORTS))
                    .unwrap();
            let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
            let reverse = serialize_elf_section_header_table(roster(
                TargetProfile::LinuxX64,
                &reverse_imports,
            ))
            .unwrap();
            let arm =
                serialize_elf_section_header_table(roster(TargetProfile::LinuxArm64, &IMPORTS))
                    .unwrap();
            assert_eq!(forward.contents, reverse.contents);
            assert_eq!(
                forward.non_authoritative_template_compatibility_fingerprint(),
                reverse.non_authoritative_template_compatibility_fingerprint()
            );
            assert_ne!(
                forward.non_authoritative_template_compatibility_fingerprint(),
                arm.non_authoritative_template_compatibility_fingerprint()
            );
        }

        #[test]
        fn header_field_boundaries_reject_with_roster_custody() {
            for row_index in [0, 1, SECTION_COUNT - 1] {
                for (field_offset, field_width) in [
                    (0, 4),
                    (4, 4),
                    (8, 8),
                    (16, 8),
                    (24, 8),
                    (32, 8),
                    (40, 4),
                    (44, 4),
                    (48, 8),
                    (56, 8),
                ] {
                    for byte_offset in [field_offset, field_offset + field_width - 1] {
                        let mut candidate = candidate(TargetProfile::LinuxX64);
                        let expected_identity = candidate
                            .roster
                            .non_authoritative_roster_compatibility_fingerprint();
                        candidate.contents.bytes
                            [row_index * ELF64_SECTION_HEADER_SIZE + byte_offset] ^= 1;
                        let error = validate_candidate(candidate)
                            .expect_err("mutated ELF section-header field must reject");
                        assert!(
                            error
                                .diagnostic
                                .to_string()
                                .contains("decoded ELF64 section header drifted")
                        );
                        assert_eq!(
                            error
                                .candidate
                                .roster
                                .non_authoritative_roster_compatibility_fingerprint(),
                            expected_identity,
                        );
                    }
                }
            }
        }

        #[test]
        fn independent_replay_rejects_every_fixup_field_length_order_and_identity_corruption() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| {
                    candidate.contents.bytes.pop();
                }),
                Box::new(|candidate| candidate.contents.bytes.push(0)),
                Box::new(|candidate| {
                    candidate.contents.placement_fixups.pop();
                }),
                Box::new(|candidate| {
                    candidate
                        .contents
                        .placement_fixups
                        .push(candidate.contents.placement_fixups[0])
                }),
                Box::new(|candidate| candidate.contents.placement_fixups.swap(0, 1)),
                Box::new(|candidate| candidate.contents.placement_fixups[0].row_index = u32::MAX),
                Box::new(|candidate| {
                    candidate.contents.placement_fixups[0].section_kind =
                        ElfDynamicRosterSectionKind::DynamicString
                }),
                Box::new(|candidate| candidate.contents.placement_fixups[0].byte_offset += 1),
                Box::new(|candidate| candidate.contents.placement_fixups[0].byte_width = 4),
                Box::new(|candidate| {
                    candidate.contents.placement_fixups[0].kind =
                        ElfSectionHeaderPlacementFixupKind::FileOffset
                }),
                Box::new(|candidate| {
                    candidate.non_authoritative_template_compatibility_fingerprint ^= 1
                }),
            ];
            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxArm64);
                let expected_identity = candidate
                    .roster
                    .non_authoritative_roster_compatibility_fingerprint();
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("corrupt section-header candidate must reject");
                assert_eq!(
                    error
                        .candidate
                        .roster
                        .non_authoritative_roster_compatibility_fingerprint(),
                    expected_identity
                );
            }
        }

        #[test]
        fn decoder_fixup_bounds_and_arithmetic_reject_without_panicking() {
            assert!(checked_product(usize::MAX, 64, "product").is_err());
            assert!(checked_sum(usize::MAX, 8, "sum").is_err());
            assert!(decode_section_header_rows(&[], usize::MAX).is_err());
            assert!(decode_section_header_rows(&[0; 63], 1).is_err());
            assert!(decode_section_header_rows(&[0; 65], 1).is_err());
            assert!(read_u32(&[0; 3], 0, "word").is_err());
            assert!(read_u64(&[0; 7], 0, "xword").is_err());

            let overflow = ElfSectionHeaderPlacementFixup {
                row_index: u32::MAX,
                section_kind: ElfDynamicRosterSectionKind::Interpreter,
                byte_offset: usize::MAX,
                byte_width: 8,
                kind: ElfSectionHeaderPlacementFixupKind::VirtualAddress,
            };
            assert!(validate_fixup_coverage(768, &[overflow, overflow]).is_err());
        }
    }
}

pub(crate) mod section_name_table {
    use super::super::checked::{self, CandidateValidationError};
    use crate::bytes::{name_at, require_name};
    use crate::dynamic_executable::checked::{checked_sum, checked_u32, require};
    use crate::dynamic_executable::dynamic_table::dynamic_table_descriptor::ValidatedElfDynamicTableSectionDescriptorPlan;
    use crate::dynamic_executable::import_sections::dynamic_section_descriptors::ElfDynamicSectionKind;
    use diagnostics::Diagnostic;

    const SHT_STRTAB: u32 = 3;
    const UPSTREAM_DESCRIPTOR_COUNT: usize = 12;
    const UPSTREAM_NAME_SEED_SIZE: usize = 122;
    const SECTION_NAME_TABLE_NAME_OFFSET: u32 = 59;
    const COMPLETE_SECTION_NAME_TABLE_SIZE: usize = 122;
    const COMPLETE_SECTION_NAME_TABLE: &[u8] = b"\0.interp\0.dynstr\0.dynsym\0.hash\0.gnu.version\0.gnu.version_r\0.shstrtab\0.gnu.hash\0.plt\0.got.plt\0.rela.plt\0.rela.dyn\0.dynamic\0";
    #[derive(Debug)]
    #[must_use = "validated ELF section-name table retains the dynamic descriptor owner"]
    pub struct ValidatedElfSectionNameTablePlan {
        dynamic_table: ValidatedElfDynamicTableSectionDescriptorPlan,
        contents: ElfSectionNameTableContents,
        non_authoritative_table_compatibility_fingerprint: u64,
    }

    impl ValidatedElfSectionNameTablePlan {
        pub const fn dynamic_table(&self) -> &ValidatedElfDynamicTableSectionDescriptorPlan {
            &self.dynamic_table
        }

        pub const fn descriptor_count(&self) -> usize {
            UPSTREAM_DESCRIPTOR_COUNT + 1
        }

        pub const fn appended_descriptor_count(&self) -> usize {
            1
        }

        pub fn byte_count(&self) -> usize {
            self.contents.bytes.len()
        }

        pub const fn non_authoritative_table_compatibility_fingerprint(&self) -> u64 {
            self.non_authoritative_table_compatibility_fingerprint
        }

        pub(crate) const fn contents(&self) -> &ElfSectionNameTableContents {
            &self.contents
        }

        pub(crate) fn into_parts(
            self,
        ) -> (
            ValidatedElfDynamicTableSectionDescriptorPlan,
            ElfSectionNameTableContents,
        ) {
            (self.dynamic_table, self.contents)
        }
    }

    #[derive(Debug)]
    #[must_use = "ELF section-name-table rejection retains the dynamic descriptor owner"]
    pub struct ElfSectionNameTablePlanningError {
        dynamic_table: ValidatedElfDynamicTableSectionDescriptorPlan,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(
        ElfSectionNameTablePlanningError;
        dynamic_table: ValidatedElfDynamicTableSectionDescriptorPlan,
    );

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfSectionNameTableContents {
        pub(crate) bytes: Vec<u8>,
        pub(crate) descriptor: ElfSectionNameTableDescriptor,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u8)]
    pub(crate) enum ElfSectionNameTableSectionKind {
        SectionNameTable = 1,
    }

    impl ElfSectionNameTableSectionKind {
        const fn name(self) -> &'static [u8] {
            match self {
                Self::SectionNameTable => b".shstrtab",
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfSectionNameTableDescriptor {
        pub(crate) kind: ElfSectionNameTableSectionKind,
        pub(crate) name_offset: u32,
        pub(crate) section_type: u32,
        pub(crate) flags: u64,
        pub(crate) payload_size: u64,
        pub(crate) alignment: u64,
        pub(crate) entry_size: u64,
        pub(crate) link: Option<ElfDynamicSectionKind>,
        pub(crate) info: Option<ElfDynamicSectionKind>,
    }

    struct Candidate {
        dynamic_table: ValidatedElfDynamicTableSectionDescriptorPlan,
        contents: ElfSectionNameTableContents,
        non_authoritative_table_compatibility_fingerprint: u64,
    }

    pub(crate) fn plan_elf_section_name_table(
        dynamic_table: ValidatedElfDynamicTableSectionDescriptorPlan,
    ) -> Result<ValidatedElfSectionNameTablePlan, Box<ElfSectionNameTablePlanningError>> {
        checked::plan_stage!(
            plan: dynamic_table,
            derive: derive_contents,
            contents: contents,
            fingerprint: non_authoritative_table_compatibility_fingerprint,
            error: ElfSectionNameTablePlanningError,
        )
    }

    fn derive_contents(
        dynamic_table: &ValidatedElfDynamicTableSectionDescriptorPlan,
    ) -> Result<ElfSectionNameTableContents, Diagnostic> {
        let base_seed = upstream_name_seed(dynamic_table);
        let name_offset = checked_u32(
            SECTION_NAME_TABLE_NAME_OFFSET as usize,
            "section-name-table name offset",
        )?;
        let bytes = base_seed.to_vec();
        let payload_size = u64::try_from(bytes.len())
            .map_err(|_| Diagnostic::error("section-name-table payload exceeds Elf64_Xword"))?;
        Ok(ElfSectionNameTableContents {
            bytes,
            descriptor: ElfSectionNameTableDescriptor {
                kind: ElfSectionNameTableSectionKind::SectionNameTable,
                name_offset,
                section_type: SHT_STRTAB,
                flags: 0,
                payload_size,
                alignment: 1,
                entry_size: 0,
                link: None,
                info: None,
            },
        })
    }

    fn upstream_name_seed(dynamic_table: &ValidatedElfDynamicTableSectionDescriptorPlan) -> &[u8] {
        &dynamic_table.contents().section_name_table_seed
    }

    checked::seal_stage! {
        field: dynamic_table,
        contents: contents,
        validated: ValidatedElfSectionNameTablePlan,
        validate: validate_contents,
        fingerprint: non_authoritative_table_compatibility_fingerprint,
        message: "ELF section-name-table compatibility fingerprint does not replay",
    }

    fn validate_contents(
        dynamic_table: &ValidatedElfDynamicTableSectionDescriptorPlan,
        contents: &ElfSectionNameTableContents,
    ) -> Result<(), Diagnostic> {
        require(
            dynamic_table.descriptor_count() == UPSTREAM_DESCRIPTOR_COUNT,
            "section-name table requires the exact sealed twelve-row base",
        )?;
        let base_seed = upstream_name_seed(dynamic_table);
        require(
            base_seed.len() == UPSTREAM_NAME_SEED_SIZE,
            "section-name table requires the exact 122-byte upstream name seed",
        )?;
        require(
            base_seed.len() == COMPLETE_SECTION_NAME_TABLE_SIZE
                && COMPLETE_SECTION_NAME_TABLE.len() == COMPLETE_SECTION_NAME_TABLE_SIZE,
            "section-name-table fixed size does not replay",
        )?;
        require(
            base_seed == COMPLETE_SECTION_NAME_TABLE
                && contents.bytes.len() == COMPLETE_SECTION_NAME_TABLE_SIZE
                && contents.bytes == base_seed
                && contents.bytes == COMPLETE_SECTION_NAME_TABLE,
            "section-name-table payload is not the exact bounded upstream seed",
        )?;
        validate_complete_names(&contents.bytes)?;

        let row = &contents.descriptor;
        validate_name(&contents.bytes, row)?;
        require(
            row.kind == ElfSectionNameTableSectionKind::SectionNameTable
                && row.name_offset == SECTION_NAME_TABLE_NAME_OFFSET
                && row.section_type == SHT_STRTAB
                && row.flags == 0
                && row.payload_size
                    == u64::try_from(contents.bytes.len()).map_err(|_| {
                        Diagnostic::error("validated section-name-table size exceeds Elf64_Xword")
                    })?
                && row.payload_size == COMPLETE_SECTION_NAME_TABLE_SIZE as u64
                && row.alignment == 1
                && row.entry_size == 0
                && row.link.is_none()
                && row.info.is_none(),
            "section-name-table descriptor metadata drifted from its exact payload",
        )
    }

    fn validate_complete_names(bytes: &[u8]) -> Result<(), Diagnostic> {
        const EXPECTED: &[(usize, &[u8])] = &[
            (0, b""),
            (1, b".interp"),
            (9, b".dynstr"),
            (17, b".dynsym"),
            (25, b".hash"),
            (31, b".gnu.version"),
            (44, b".gnu.version_r"),
            (59, b".shstrtab"),
            (69, b".gnu.hash"),
            (79, b".plt"),
            (84, b".got.plt"),
            (93, b".rela.plt"),
            (103, b".rela.dyn"),
            (113, b".dynamic"),
        ];
        let mut next_offset = 0usize;
        for (offset, expected) in EXPECTED {
            require(
                *offset == next_offset,
                "section-name-table expected offsets are not contiguous",
            )?;
            let name = name_at(bytes, *offset as u32, "section name")?;
            require(
                name == *expected,
                "section-name-table entry does not replay",
            )?;
            next_offset = checked_sum(*offset, name.len() + 1, "section-name-table traversal")?;
        }
        require(
            next_offset == bytes.len(),
            "section-name-table traversal leaves trailing bytes",
        )
    }

    fn validate_name(bytes: &[u8], row: &ElfSectionNameTableDescriptor) -> Result<(), Diagnostic> {
        require_name(
            bytes,
            row.name_offset,
            row.kind.name(),
            "section-name-table sh_name",
        )
    }

    fn non_authoritative_table_compatibility_fingerprint(
        dynamic_table: &ValidatedElfDynamicTableSectionDescriptorPlan,
        contents: &ElfSectionNameTableContents,
    ) -> u64 {
        let mut hash = checked::stage_fingerprint(
            b"omega.elf-section-name-table.v1",
            dynamic_table.non_authoritative_descriptor_compatibility_fingerprint(),
        );
        hash.bytes(&contents.bytes);
        let row = &contents.descriptor;
        hash.byte(row.kind as u8);
        hash.bytes(&row.name_offset.to_le_bytes());
        hash.bytes(&row.section_type.to_le_bytes());
        hash.bytes(&row.flags.to_le_bytes());
        hash.bytes(&row.payload_size.to_le_bytes());
        hash.bytes(&row.alignment.to_le_bytes());
        hash.bytes(&row.entry_size.to_le_bytes());
        match row.link {
            Some(kind) => {
                hash.byte(1);
                hash.byte(kind as u8);
            }
            None => hash.byte(0),
        }
        match row.info {
            Some(kind) => {
                hash.byte(1);
                hash.byte(kind as u8);
            }
            None => hash.byte(0),
        }
        hash.finish()
    }

    #[cfg(test)]
    mod tests {
        use super::{
            COMPLETE_SECTION_NAME_TABLE, Candidate, ElfDynamicSectionKind,
            ElfSectionNameTableDescriptor, ElfSectionNameTableSectionKind,
            SECTION_NAME_TABLE_NAME_OFFSET, SHT_STRTAB,
            ValidatedElfDynamicTableSectionDescriptorPlan, checked_sum, checked_u32,
            derive_contents, non_authoritative_table_compatibility_fingerprint,
            plan_elf_section_name_table, upstream_name_seed, validate_candidate,
            validate_complete_names, validate_contents, validate_name,
        };
        use crate::test_support::DynamicImportFixture as ImportFixture;
        use crate::{
            plan_elf_dynamic_table_section_descriptor, plan_elf_dynamic_tags,
            serialize_elf_dynamic_table,
        };

        use target::TargetProfile;

        const IMPORTS: [ImportFixture; 2] = crate::test_support::PAIR_IMPORTS;
        fn dynamic_table(
            target: TargetProfile,
            imports: &[ImportFixture],
        ) -> ValidatedElfDynamicTableSectionDescriptorPlan {
            let inputs = crate::test_support::dynamic_import_inputs(
                target,
                imports,
                "__omega_section_name_import_",
                "valid section-name locator",
                "valid section-name interpreter",
            );
            let descriptors = crate::test_support::linkage_descriptors(inputs);
            let tags = plan_elf_dynamic_tags(descriptors).expect("valid dynamic tags");
            let payload = serialize_elf_dynamic_table(tags).expect("valid dynamic payload");
            plan_elf_dynamic_table_section_descriptor(payload).expect("valid dynamic descriptor")
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let dynamic_table = dynamic_table(target, &IMPORTS);
            let contents = derive_contents(&dynamic_table).expect("derived section-name table");
            let non_authoritative_table_compatibility_fingerprint =
                non_authoritative_table_compatibility_fingerprint(&dynamic_table, &contents);
            Candidate {
                dynamic_table,
                contents,
                non_authoritative_table_compatibility_fingerprint,
            }
        }

        #[test]
        fn both_targets_adopt_exact_payload_and_address_free_descriptor() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let plan = plan_elf_section_name_table(dynamic_table(target, &IMPORTS))
                    .expect("validated section-name table");
                assert_eq!(plan.dynamic_table().descriptor_count(), 12);
                assert_eq!(plan.descriptor_count(), 13);
                assert_eq!(plan.appended_descriptor_count(), 1);
                assert_eq!(plan.byte_count(), 122);
                assert_eq!(plan.contents.bytes, COMPLETE_SECTION_NAME_TABLE);
                assert_eq!(
                    plan.contents.bytes,
                    upstream_name_seed(plan.dynamic_table())
                );
                let dynstr = &plan
                    .dynamic_table()
                    .payload()
                    .plan()
                    .descriptors()
                    .templates()
                    .linkage()
                    .descriptors()
                    .payloads()
                    .payloads()
                    .dynstr;
                assert!(dynstr.windows(8).any(|window| window == b"liba\xff.so"));
                assert_eq!(
                    plan.contents.descriptor,
                    ElfSectionNameTableDescriptor {
                        kind: ElfSectionNameTableSectionKind::SectionNameTable,
                        name_offset: 59,
                        section_type: SHT_STRTAB,
                        flags: 0,
                        payload_size: 122,
                        alignment: 1,
                        entry_size: 0,
                        link: None,
                        info: None,
                    },
                );
                assert_ne!(plan.non_authoritative_table_compatibility_fingerprint(), 0);
                validate_contents(plan.dynamic_table(), &plan.contents)
                    .expect("independent section-name-table replay");
            }
        }

        #[test]
        fn import_permutation_preserves_table_identity_and_target_remains_bound() {
            let forward =
                plan_elf_section_name_table(dynamic_table(TargetProfile::LinuxX64, &IMPORTS))
                    .unwrap();
            let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
            let reverse = plan_elf_section_name_table(dynamic_table(
                TargetProfile::LinuxX64,
                &reverse_imports,
            ))
            .unwrap();
            let arm =
                plan_elf_section_name_table(dynamic_table(TargetProfile::LinuxArm64, &IMPORTS))
                    .unwrap();
            assert_eq!(forward.contents, reverse.contents);
            assert_eq!(
                forward.non_authoritative_table_compatibility_fingerprint(),
                reverse.non_authoritative_table_compatibility_fingerprint()
            );
            assert_ne!(
                forward.non_authoritative_table_compatibility_fingerprint(),
                arm.non_authoritative_table_compatibility_fingerprint()
            );
        }

        #[test]
        fn independent_replay_rejects_length_metadata_link_info_and_identity_corruption() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| candidate.contents.bytes[0] ^= 1),
                Box::new(|candidate| {
                    candidate.contents.bytes[SECTION_NAME_TABLE_NAME_OFFSET as usize] ^= 1
                }),
                Box::new(|candidate| *candidate.contents.bytes.last_mut().unwrap() ^= 1),
                Box::new(|candidate| {
                    candidate.contents.bytes.pop();
                }),
                Box::new(|candidate| candidate.contents.bytes.push(0)),
                Box::new(|candidate| candidate.contents.descriptor.name_offset = u32::MAX),
                Box::new(|candidate| candidate.contents.descriptor.section_type = 1),
                Box::new(|candidate| candidate.contents.descriptor.flags = 1),
                Box::new(|candidate| candidate.contents.descriptor.payload_size += 1),
                Box::new(|candidate| candidate.contents.descriptor.alignment = 8),
                Box::new(|candidate| candidate.contents.descriptor.entry_size = 1),
                Box::new(|candidate| {
                    candidate.contents.descriptor.link = Some(ElfDynamicSectionKind::DynamicString)
                }),
                Box::new(|candidate| {
                    candidate.contents.descriptor.info = Some(ElfDynamicSectionKind::DynamicSymbol)
                }),
                Box::new(|candidate| {
                    candidate.non_authoritative_table_compatibility_fingerprint ^= 1
                }),
            ];
            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxArm64);
                let expected_identity = candidate
                    .dynamic_table
                    .non_authoritative_descriptor_compatibility_fingerprint();
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("corrupt section-name-table candidate must reject");
                assert_eq!(
                    error
                        .candidate
                        .dynamic_table
                        .non_authoritative_descriptor_compatibility_fingerprint(),
                    expected_identity,
                );
            }
        }

        #[test]
        fn bounded_name_traversal_and_arithmetic_reject_without_panicking() {
            assert!(checked_sum(usize::MAX, 1, "sum").is_err());
            assert!(checked_u32(usize::MAX, "word").is_err());
            assert!(validate_complete_names(&[]).is_err());
            assert!(validate_complete_names(&COMPLETE_SECTION_NAME_TABLE[..101]).is_err());
            let mut trailing = COMPLETE_SECTION_NAME_TABLE.to_vec();
            trailing.push(0);
            assert!(validate_complete_names(&trailing).is_err());

            let mut descriptor = ElfSectionNameTableDescriptor {
                kind: ElfSectionNameTableSectionKind::SectionNameTable,
                name_offset: u32::MAX,
                section_type: SHT_STRTAB,
                flags: 0,
                payload_size: 122,
                alignment: 1,
                entry_size: 0,
                link: None,
                info: None,
            };
            assert!(validate_name(b"\0.shstrtab\0", &descriptor).is_err());
            descriptor.name_offset = 1;
            assert!(validate_name(b"\0.shstrtab", &descriptor).is_err());
            descriptor.name_offset = 2;
            assert!(validate_name(b"\0.shstrtab\0", &descriptor).is_err());
        }
    }
}

pub(crate) mod section_payload_roster {
    use target::compatibility_fingerprint::Fnv1a;

    use super::super::checked::{self, CandidateValidationError};
    use crate::constants::SECTION_COUNT;
    use crate::dynamic_executable::checked::{checked_u32, read_field, require};
    use crate::dynamic_executable::dynamic_table::dynamic_tag_bytes::{
        ElfDynamicPayloadFixupKind, ValidatedElfDynamicTablePayload,
    };
    use crate::dynamic_executable::dynamic_table::dynamic_tags::ElfDynamicAddressTarget;
    use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::{
        ElfProcedureLinkageFixupKind, ElfProcedureLinkageFixupStorage,
        ElfProcedureLinkagePlacementConstraint, ElfProcedureLinkageSemanticTarget,
        ValidatedElfProcedureLinkageTemplatePlan,
    };
    use crate::dynamic_executable::section_headers::section_header_bytes::ValidatedElfSectionHeaderTableTemplate;
    use crate::dynamic_executable::section_headers::section_roster::ElfDynamicRosterSectionKind;
    use diagnostics::Diagnostic;

    #[derive(Debug)]
    #[must_use = "validated indexed ELF payloads retain the section-header template"]
    pub struct ValidatedElfIndexedSectionPayloadPlan {
        section_headers: ValidatedElfSectionHeaderTableTemplate,
        contents: ElfIndexedSectionPayloadContents,
        non_authoritative_payload_roster_compatibility_fingerprint: u64,
    }

    impl ValidatedElfIndexedSectionPayloadPlan {
        pub const fn section_headers(&self) -> &ValidatedElfSectionHeaderTableTemplate {
            &self.section_headers
        }

        pub fn row_count(&self) -> usize {
            self.contents.rows.len()
        }
        #[cfg(test)]
        pub(crate) fn dynamic_fixup_count(&self) -> usize {
            self.contents.dynamic_fixups.len()
        }

        pub const fn non_authoritative_payload_roster_compatibility_fingerprint(&self) -> u64 {
            self.non_authoritative_payload_roster_compatibility_fingerprint
        }

        pub(crate) const fn contents(&self) -> &ElfIndexedSectionPayloadContents {
            &self.contents
        }

        pub(crate) fn into_parts(
            self,
        ) -> (
            ValidatedElfSectionHeaderTableTemplate,
            ElfIndexedSectionPayloadContents,
        ) {
            (self.section_headers, self.contents)
        }
    }

    #[derive(Debug)]
    #[must_use = "ELF indexed-payload rejection retains the section-header template"]
    pub struct ElfIndexedSectionPayloadPlanningError {
        section_headers: ValidatedElfSectionHeaderTableTemplate,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(
        ElfIndexedSectionPayloadPlanningError;
        section_headers: ValidatedElfSectionHeaderTableTemplate,
    );

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfIndexedSectionPayloadContents {
        pub(crate) rows: Vec<ElfIndexedSectionPayloadRow>,
        pub(crate) procedure_fixups: Vec<ElfIndexedProcedureFixup>,
        pub(crate) procedure_constraints: Vec<ElfProcedureLinkagePlacementConstraint>,
        pub(crate) dynamic_fixups: Vec<ElfIndexedDynamicFixup>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfIndexedSectionPayloadRow {
        pub(crate) index: u32,
        pub(crate) kind: ElfDynamicRosterSectionKind,
        pub(crate) bytes: Vec<u8>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ElfIndexedProcedureFixupStorage {
        SourceText,
        Section {
            index: u32,
            kind: ElfDynamicRosterSectionKind,
        },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfIndexedProcedureFixup {
        pub(crate) upstream_ordinal: u32,
        pub(crate) storage: ElfIndexedProcedureFixupStorage,
        pub(crate) byte_offset: usize,
        pub(crate) byte_width: u8,
        pub(crate) mutable_mask: u64,
        pub(crate) kind: ElfProcedureLinkageFixupKind,
        pub(crate) target: ElfProcedureLinkageSemanticTarget,
        pub(crate) target_section_index: u32,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfIndexedDynamicFixup {
        pub(crate) row_ordinal: u32,
        pub(crate) storage_section_index: u32,
        pub(crate) byte_offset: usize,
        pub(crate) byte_width: u8,
        pub(crate) kind: ElfDynamicPayloadFixupKind,
        pub(crate) target: ElfDynamicAddressTarget,
        pub(crate) target_section_index: u32,
    }

    struct Candidate {
        section_headers: ValidatedElfSectionHeaderTableTemplate,
        contents: ElfIndexedSectionPayloadContents,
        non_authoritative_payload_roster_compatibility_fingerprint: u64,
    }

    pub(crate) fn plan_elf_indexed_section_payloads(
        section_headers: ValidatedElfSectionHeaderTableTemplate,
    ) -> Result<ValidatedElfIndexedSectionPayloadPlan, Box<ElfIndexedSectionPayloadPlanningError>>
    {
        checked::plan_stage!(
            plan: section_headers,
            derive: derive_contents,
            contents: contents,
            fingerprint: non_authoritative_payload_roster_compatibility_fingerprint,
            error: ElfIndexedSectionPayloadPlanningError,
        )
    }

    fn derive_contents(
        section_headers: &ValidatedElfSectionHeaderTableTemplate,
    ) -> Result<ElfIndexedSectionPayloadContents, Diagnostic> {
        let roster = &section_headers.roster().contents().rows;
        let mut rows = Vec::with_capacity(SECTION_COUNT);
        for descriptor in roster {
            rows.push(ElfIndexedSectionPayloadRow {
                index: descriptor.index,
                kind: descriptor.kind,
                bytes: upstream_payload(section_headers, descriptor.kind)?.to_vec(),
            });
        }

        let templates = procedure_templates(section_headers);
        let procedure_fixups = templates
            .contents()
            .fixups
            .iter()
            .enumerate()
            .map(|(ordinal, fixup)| {
                Ok(ElfIndexedProcedureFixup {
                    upstream_ordinal: checked_u32(ordinal, "procedure fixup ordinal")?,
                    storage: indexed_procedure_storage(fixup.storage),
                    byte_offset: fixup.byte_offset,
                    byte_width: fixup.byte_width,
                    mutable_mask: fixup.mutable_mask,
                    kind: fixup.kind,
                    target: fixup.target,
                    target_section_index: procedure_target_section(fixup.target),
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let dynamic_fixups = dynamic_payload(section_headers)
            .contents()
            .address_fixups
            .iter()
            .map(|fixup| ElfIndexedDynamicFixup {
                row_ordinal: fixup.row_ordinal,
                storage_section_index: 12,
                byte_offset: fixup.byte_offset,
                byte_width: fixup.byte_width,
                kind: fixup.kind,
                target: fixup.target,
                target_section_index: dynamic_target_section(fixup.target),
            })
            .collect();
        Ok(ElfIndexedSectionPayloadContents {
            rows,
            procedure_fixups,
            procedure_constraints: templates.contents().constraints.clone(),
            dynamic_fixups,
        })
    }

    fn procedure_templates(
        section_headers: &ValidatedElfSectionHeaderTableTemplate,
    ) -> &ValidatedElfProcedureLinkageTemplatePlan {
        section_headers
            .roster()
            .section_names()
            .dynamic_table()
            .payload()
            .plan()
            .descriptors()
            .templates()
    }

    fn dynamic_payload(
        section_headers: &ValidatedElfSectionHeaderTableTemplate,
    ) -> &ValidatedElfDynamicTablePayload {
        section_headers
            .roster()
            .section_names()
            .dynamic_table()
            .payload()
    }

    fn upstream_payload(
        section_headers: &ValidatedElfSectionHeaderTableTemplate,
        kind: ElfDynamicRosterSectionKind,
    ) -> Result<&[u8], Diagnostic> {
        let templates = procedure_templates(section_headers);
        let base = templates.linkage().descriptors().payloads().payloads();
        let bytes = &templates.contents().bytes;
        Ok(match kind {
            ElfDynamicRosterSectionKind::Null => &[],
            ElfDynamicRosterSectionKind::Interpreter => &base.interpreter,
            ElfDynamicRosterSectionKind::DynamicString => &base.dynstr,
            ElfDynamicRosterSectionKind::DynamicSymbol => &base.dynsym,
            ElfDynamicRosterSectionKind::SystemVHash => &base.sysv_hash,
            ElfDynamicRosterSectionKind::GnuHash => &base.gnu_hash,
            ElfDynamicRosterSectionKind::GnuSymbolVersion => &base.versym,
            ElfDynamicRosterSectionKind::GnuVersionRequirement => &base.verneed,
            ElfDynamicRosterSectionKind::ProcedureLinkage => &bytes.plt,
            ElfDynamicRosterSectionKind::ProcedureGot => &bytes.got_plt,
            ElfDynamicRosterSectionKind::ProcedureRelocation => &bytes.rela_plt,
            ElfDynamicRosterSectionKind::GeneralRelocation => &bytes.rela_dyn,
            ElfDynamicRosterSectionKind::DynamicTable => {
                &dynamic_payload(section_headers).contents().bytes
            }
            ElfDynamicRosterSectionKind::SectionNameTable => {
                &section_headers.roster().section_names().contents().bytes
            }
        })
    }

    const fn indexed_procedure_storage(
        storage: ElfProcedureLinkageFixupStorage,
    ) -> ElfIndexedProcedureFixupStorage {
        match storage {
            ElfProcedureLinkageFixupStorage::SourceText => {
                ElfIndexedProcedureFixupStorage::SourceText
            }
            ElfProcedureLinkageFixupStorage::Plt => ElfIndexedProcedureFixupStorage::Section {
                index: 8,
                kind: ElfDynamicRosterSectionKind::ProcedureLinkage,
            },
            ElfProcedureLinkageFixupStorage::GotPlt => ElfIndexedProcedureFixupStorage::Section {
                index: 9,
                kind: ElfDynamicRosterSectionKind::ProcedureGot,
            },
            ElfProcedureLinkageFixupStorage::RelaPlt => ElfIndexedProcedureFixupStorage::Section {
                index: 10,
                kind: ElfDynamicRosterSectionKind::ProcedureRelocation,
            },
            ElfProcedureLinkageFixupStorage::RelaDyn => ElfIndexedProcedureFixupStorage::Section {
                index: 11,
                kind: ElfDynamicRosterSectionKind::GeneralRelocation,
            },
        }
    }

    const fn procedure_target_section(target: ElfProcedureLinkageSemanticTarget) -> u32 {
        match target {
            ElfProcedureLinkageSemanticTarget::FutureDynamicSection => 12,
            ElfProcedureLinkageSemanticTarget::PltHeader
            | ElfProcedureLinkageSemanticTarget::PltEntry { .. }
            | ElfProcedureLinkageSemanticTarget::PltLazyTail { .. } => 8,
            ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { .. }
            | ElfProcedureLinkageSemanticTarget::GotPltSlot { .. } => 9,
            ElfProcedureLinkageSemanticTarget::RelocatedImageSection { .. } => u32::MAX,
        }
    }

    fn dynamic_target_section(target: ElfDynamicAddressTarget) -> u32 {
        crate::dynamic_executable::load_placement::resolved_dynamic_table::target_section_index(
            target,
        )
    }

    checked::seal_stage! {
        field: section_headers,
        contents: contents,
        validated: ValidatedElfIndexedSectionPayloadPlan,
        validate: validate_contents,
        fingerprint: non_authoritative_payload_roster_compatibility_fingerprint,
        message: "ELF indexed payload-roster compatibility fingerprint does not replay",
    }

    fn validate_contents(
        section_headers: &ValidatedElfSectionHeaderTableTemplate,
        contents: &ElfIndexedSectionPayloadContents,
    ) -> Result<(), Diagnostic> {
        let roster = &section_headers.roster().contents().rows;
        require(
            roster.len() == SECTION_COUNT && contents.rows.len() == SECTION_COUNT,
            "indexed ELF payload roster must contain exactly fourteen rows",
        )?;
        for (ordinal, (row, descriptor)) in contents.rows.iter().zip(roster).enumerate() {
            require(
                row.index == checked_u32(ordinal, "indexed payload ordinal")?
                    && row.index == descriptor.index
                    && row.kind == descriptor.kind
                    && row.bytes == upstream_payload(section_headers, row.kind)?
                    && u64::try_from(row.bytes.len()).ok() == Some(descriptor.payload_size),
                "indexed ELF payload row drifted from its roster or upstream bytes",
            )?;
        }
        validate_procedure_fixups(section_headers, contents)?;
        validate_dynamic_fixups(section_headers, contents)
    }

    fn validate_procedure_fixups(
        section_headers: &ValidatedElfSectionHeaderTableTemplate,
        contents: &ElfIndexedSectionPayloadContents,
    ) -> Result<(), Diagnostic> {
        let templates = procedure_templates(section_headers);
        require(
            contents.procedure_fixups.len() == templates.contents().fixups.len()
                && contents.procedure_constraints == templates.contents().constraints,
            "indexed procedure fixups or constraints do not exactly cover the upstream templates",
        )?;
        for (ordinal, (indexed, upstream)) in contents
            .procedure_fixups
            .iter()
            .zip(&templates.contents().fixups)
            .enumerate()
        {
            require(
                indexed.upstream_ordinal == checked_u32(ordinal, "replayed procedure fixup")?
                    && indexed.storage == indexed_procedure_storage(upstream.storage)
                    && indexed.byte_offset == upstream.byte_offset
                    && indexed.byte_width == upstream.byte_width
                    && indexed.mutable_mask == upstream.mutable_mask
                    && indexed.kind == upstream.kind
                    && indexed.target == upstream.target
                    && indexed.target_section_index == procedure_target_section(upstream.target),
                "indexed procedure fixup drifted from the upstream template",
            )?;
            let storage = procedure_storage_bytes(section_headers, contents, indexed.storage)?;
            let field = read_field(
                storage,
                indexed.byte_offset,
                indexed.byte_width,
                "indexed procedure fixup field",
            )?;
            require(
                field & indexed.mutable_mask == 0,
                "indexed procedure fixup does not select an exact zero placeholder",
            )?;
        }
        for constraint in &contents.procedure_constraints {
            require(
                (constraint.fixup_ordinal as usize) < contents.procedure_fixups.len(),
                "indexed procedure constraint references a missing fixup",
            )?;
        }
        Ok(())
    }

    fn procedure_storage_bytes<'a>(
        section_headers: &'a ValidatedElfSectionHeaderTableTemplate,
        contents: &'a ElfIndexedSectionPayloadContents,
        storage: ElfIndexedProcedureFixupStorage,
    ) -> Result<&'a [u8], Diagnostic> {
        match storage {
            ElfIndexedProcedureFixupStorage::SourceText => {
                Ok(&procedure_templates(section_headers)
                    .linkage()
                    .descriptors()
                    .payloads()
                    .plan()
                    .inputs()
                    .image()
                    .memory
                    .text)
            }
            ElfIndexedProcedureFixupStorage::Section { index, kind } => contents
                .rows
                .get(index as usize)
                .filter(|row| row.kind == kind)
                .map(|row| row.bytes.as_slice())
                .ok_or_else(|| Diagnostic::error("indexed procedure-fixup storage is missing")),
        }
    }

    fn validate_dynamic_fixups(
        section_headers: &ValidatedElfSectionHeaderTableTemplate,
        contents: &ElfIndexedSectionPayloadContents,
    ) -> Result<(), Diagnostic> {
        let payload = dynamic_payload(section_headers);
        let upstream = &payload.contents().address_fixups;
        require(
            upstream.len() == payload.plan().address_obligation_count()
                && contents.dynamic_fixups.len() == upstream.len(),
            "indexed dynamic fixups do not exactly cover the retained semantic address obligations",
        )?;
        let storage = contents
            .rows
            .get(12)
            .ok_or_else(|| Diagnostic::error("indexed .dynamic payload is missing"))?;
        for (indexed, upstream) in contents.dynamic_fixups.iter().zip(upstream) {
            require(
                indexed.row_ordinal == upstream.row_ordinal
                    && indexed.storage_section_index == 12
                    && indexed.byte_offset == upstream.byte_offset
                    && indexed.byte_width == upstream.byte_width
                    && indexed.kind == upstream.kind
                    && indexed.target == upstream.target
                    && indexed.target_section_index == dynamic_target_section(upstream.target),
                "indexed dynamic fixup drifted from the serialized .dynamic payload",
            )?;
            require(
                read_field(
                    &storage.bytes,
                    indexed.byte_offset,
                    indexed.byte_width,
                    "indexed dynamic fixup field",
                )? == 0,
                "indexed dynamic fixup does not select an exact zero placeholder",
            )?;
        }
        Ok(())
    }

    fn non_authoritative_payload_roster_compatibility_fingerprint(
        section_headers: &ValidatedElfSectionHeaderTableTemplate,
        contents: &ElfIndexedSectionPayloadContents,
    ) -> u64 {
        let mut hash = checked::stage_fingerprint(
            b"omega.elf-indexed-section-payloads.v1",
            section_headers.non_authoritative_template_compatibility_fingerprint(),
        );
        for row in &contents.rows {
            hash.bytes(&row.index.to_le_bytes());
            hash.byte(row.kind as u8);
            hash.bytes(&row.bytes);
        }
        for fixup in &contents.procedure_fixups {
            hash.bytes(&fixup.upstream_ordinal.to_le_bytes());
            hash_procedure_storage(&mut hash, fixup.storage);
            hash.bytes(&(fixup.byte_offset as u64).to_le_bytes());
            hash.byte(fixup.byte_width);
            hash.bytes(&fixup.mutable_mask.to_le_bytes());
            hash.byte(fixup.kind as u8);
            hash_procedure_target(&mut hash, fixup.target);
            hash.bytes(&fixup.target_section_index.to_le_bytes());
        }
        for constraint in &contents.procedure_constraints {
            hash.bytes(&constraint.fixup_ordinal.to_le_bytes());
            hash.byte(constraint.kind as u8);
        }
        for fixup in &contents.dynamic_fixups {
            hash.bytes(&fixup.row_ordinal.to_le_bytes());
            hash.bytes(&fixup.storage_section_index.to_le_bytes());
            hash.bytes(&(fixup.byte_offset as u64).to_le_bytes());
            hash.byte(fixup.byte_width);
            hash.byte(fixup.kind as u8);
            hash.byte(fixup.target as u8);
            hash.bytes(&fixup.target_section_index.to_le_bytes());
        }
        hash.finish()
    }

    fn hash_procedure_storage(hash: &mut Fnv1a, storage: ElfIndexedProcedureFixupStorage) {
        match storage {
            ElfIndexedProcedureFixupStorage::SourceText => hash.byte(0),
            ElfIndexedProcedureFixupStorage::Section { index, kind } => {
                hash.byte(1);
                hash.bytes(&index.to_le_bytes());
                hash.byte(kind as u8);
            }
        }
    }

    fn hash_procedure_target(hash: &mut Fnv1a, target: ElfProcedureLinkageSemanticTarget) {
        match target {
            ElfProcedureLinkageSemanticTarget::FutureDynamicSection => hash.byte(0),
            ElfProcedureLinkageSemanticTarget::PltHeader => hash.byte(1),
            ElfProcedureLinkageSemanticTarget::PltEntry { logical_ordinal } => {
                hash.byte(2);
                hash.bytes(&logical_ordinal.to_le_bytes());
            }
            ElfProcedureLinkageSemanticTarget::PltLazyTail { logical_ordinal } => {
                hash.byte(3);
                hash.bytes(&logical_ordinal.to_le_bytes());
            }
            ElfProcedureLinkageSemanticTarget::GotPltHeaderWord { word_index } => {
                hash.byte(4);
                hash.byte(word_index);
            }
            ElfProcedureLinkageSemanticTarget::GotPltSlot { logical_ordinal } => {
                hash.byte(5);
                hash.bytes(&logical_ordinal.to_le_bytes());
            }
            ElfProcedureLinkageSemanticTarget::RelocatedImageSection {
                section,
                byte_offset,
            } => {
                hash.byte(6);
                hash.byte(crate::dynamic_executable::image_section_tag(section));
                hash.bytes(&(byte_offset as u64).to_le_bytes());
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{
            Candidate, ElfDynamicRosterSectionKind, ElfIndexedProcedureFixupStorage,
            ValidatedElfSectionHeaderTableTemplate, checked_u32, derive_contents,
            non_authoritative_payload_roster_compatibility_fingerprint,
            plan_elf_indexed_section_payloads, procedure_target_section, procedure_templates,
            read_field, upstream_payload, validate_candidate, validate_contents,
        };
        use crate::dynamic_executable::checked::checked_sum;
        use crate::plan_elf_dynamic_link_inputs;
        use arena::Handle;
        use image::{FinalImage, FinalImageMemory};
        use target::{TargetProfile, normalize_elf_interpreter_plan};

        #[derive(Clone, Copy)]
        struct ImportFixture {
            object: &'static [u8],
            symbol: &'static [u8],
            version: &'static [u8],
            sites: &'static [usize],
        }

        const IMPORTS: [ImportFixture; 2] = [
            ImportFixture {
                object: b"liba\xff.so",
                symbol: b"alpha\xfe",
                version: b"V1\xfd",
                sites: &[0, 32],
            },
            ImportFixture {
                object: b"libb.so",
                symbol: b"beta",
                version: b"V2",
                sites: &[16],
            },
        ];

        fn headers(
            target: TargetProfile,
            imports: &[ImportFixture],
        ) -> ValidatedElfSectionHeaderTableTemplate {
            let mut image = FinalImage::with_capacity(
                target.native_target(),
                FinalImageMemory {
                    text: vec![0; 64],
                    ..FinalImageMemory::default()
                },
                Handle::invalid(),
                imports.len(),
                imports.len(),
                imports.iter().map(|row| row.sites.len()).sum(),
            );
            for (index, fixture) in imports.iter().enumerate() {
                let symbol_handle = crate::test_support::insert_import_fixture(
                    &mut image,
                    "__omega_payload_roster_import_",
                    index,
                    fixture.object,
                    fixture.symbol,
                    fixture.version,
                    target,
                    "valid payload-roster locator",
                );
                crate::test_support::insert_text_call_relocations(
                    &mut image,
                    target,
                    fixture.sites.iter().map(|site| (*site, symbol_handle)),
                );
            }
            let path = match target {
                TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2".as_slice(),
                TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1".as_slice(),
                _ => unreachable!(),
            };
            let interpreter = normalize_elf_interpreter_plan(path.to_vec(), target).unwrap();
            let inputs = plan_elf_dynamic_link_inputs(image, interpreter).unwrap();
            crate::test_support::section_header_table(inputs)
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let section_headers = headers(target, &IMPORTS);
            let contents = derive_contents(&section_headers).unwrap();
            let non_authoritative_payload_roster_compatibility_fingerprint =
                non_authoritative_payload_roster_compatibility_fingerprint(
                    &section_headers,
                    &contents,
                );
            Candidate {
                section_headers,
                contents,
                non_authoritative_payload_roster_compatibility_fingerprint,
            }
        }

        #[test]
        fn both_targets_join_exact_fourteen_payloads_and_indexed_fixups() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let plan = plan_elf_indexed_section_payloads(headers(target, &IMPORTS)).unwrap();
                assert_eq!(plan.row_count(), 14);
                assert_eq!(plan.dynamic_fixup_count(), 9);
                assert!(plan.contents.rows[0].bytes.is_empty());
                assert_eq!(plan.contents.rows[7].bytes.len(), 36);
                assert_eq!(plan.contents.rows[11].bytes.len(), 0);
                assert_eq!(plan.contents.rows[12].bytes.len(), 320);
                assert_eq!(plan.contents.rows[13].bytes.len(), 122);
                for (index, row) in plan.contents.rows.iter().enumerate() {
                    assert_eq!(row.index, index as u32);
                    assert_eq!(
                        row.bytes,
                        upstream_payload(plan.section_headers(), row.kind).unwrap()
                    );
                    assert_eq!(
                        row.bytes.len() as u64,
                        plan.section_headers.roster().contents().rows[index].payload_size
                    );
                }
                validate_contents(plan.section_headers(), &plan.contents).unwrap();
                assert_ne!(
                    plan.non_authoritative_payload_roster_compatibility_fingerprint(),
                    0
                );
            }
        }

        #[test]
        fn indexed_storage_targets_constraints_and_raw_bytes_are_exact() {
            let plan =
                plan_elf_indexed_section_payloads(headers(TargetProfile::LinuxX64, &IMPORTS))
                    .unwrap();
            assert!(
                plan.contents.rows[2]
                    .bytes
                    .windows(8)
                    .any(|window| window == b"liba\xff.so")
            );
            assert!(
                plan.contents
                    .procedure_fixups
                    .iter()
                    .any(|fixup| fixup.storage == ElfIndexedProcedureFixupStorage::SourceText)
            );
            for fixup in &plan.contents.procedure_fixups {
                assert_eq!(
                    fixup.target_section_index,
                    procedure_target_section(fixup.target)
                );
                if let ElfIndexedProcedureFixupStorage::Section { index, kind } = fixup.storage {
                    assert_eq!(plan.contents.rows[index as usize].kind, kind);
                }
            }
            assert_eq!(
                plan.contents.procedure_constraints,
                procedure_templates(plan.section_headers())
                    .contents()
                    .constraints
            );
            assert_eq!(
                plan.contents
                    .dynamic_fixups
                    .iter()
                    .map(|fixup| fixup.target_section_index)
                    .collect::<Vec<_>>(),
                [9, 4, 7, 2, 3, 10, 11, 5, 6]
            );
        }

        #[test]
        fn permutation_preserves_identity_and_target_remains_bound() {
            let forward =
                plan_elf_indexed_section_payloads(headers(TargetProfile::LinuxX64, &IMPORTS))
                    .unwrap();
            let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
            let reverse = plan_elf_indexed_section_payloads(headers(
                TargetProfile::LinuxX64,
                &reverse_imports,
            ))
            .unwrap();
            let arm =
                plan_elf_indexed_section_payloads(headers(TargetProfile::LinuxArm64, &IMPORTS))
                    .unwrap();
            assert_eq!(forward.contents, reverse.contents);
            assert_eq!(
                forward.non_authoritative_payload_roster_compatibility_fingerprint(),
                reverse.non_authoritative_payload_roster_compatibility_fingerprint()
            );
            assert_ne!(
                forward.non_authoritative_payload_roster_compatibility_fingerprint(),
                arm.non_authoritative_payload_roster_compatibility_fingerprint()
            );
        }

        #[test]
        fn payload_boundaries_reject_with_header_custody() {
            let lengths = candidate(TargetProfile::LinuxX64)
                .contents
                .rows
                .iter()
                .map(|row| row.bytes.len())
                .collect::<Vec<_>>();
            for (row_index, length) in lengths.into_iter().enumerate() {
                let offsets = if length == 0 {
                    vec![0]
                } else {
                    vec![0, length - 1]
                };
                for offset in offsets {
                    let mut candidate = candidate(TargetProfile::LinuxX64);
                    let identity = candidate
                        .section_headers
                        .non_authoritative_template_compatibility_fingerprint();
                    if length == 0 {
                        candidate.contents.rows[row_index].bytes.push(1);
                    } else {
                        candidate.contents.rows[row_index].bytes[offset] ^= 1;
                    }
                    let error =
                        validate_candidate(candidate).expect_err("payload corruption must reject");
                    assert!(
                        error
                            .diagnostic
                            .to_string()
                            .contains("indexed ELF payload row drifted")
                    );
                    assert_eq!(
                        error
                            .candidate
                            .section_headers
                            .non_authoritative_template_compatibility_fingerprint(),
                        identity,
                    );
                }
            }
        }

        #[test]
        fn row_fixup_constraint_and_identity_corruption_reject_recoverably() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|c| {
                    c.contents.rows.pop();
                }),
                Box::new(|c| c.contents.rows.push(c.contents.rows[0].clone())),
                Box::new(|c| c.contents.rows.swap(1, 2)),
                Box::new(|c| c.contents.rows[1].index = u32::MAX),
                Box::new(|c| c.contents.rows[1].kind = ElfDynamicRosterSectionKind::DynamicString),
                Box::new(|c| {
                    c.contents.procedure_fixups.pop();
                }),
                Box::new(|c| c.contents.procedure_fixups[0].byte_offset = usize::MAX),
                Box::new(|c| c.contents.procedure_fixups[0].byte_width = 3),
                Box::new(|c| c.contents.procedure_fixups[0].mutable_mask ^= 1),
                Box::new(|c| c.contents.procedure_fixups[0].target_section_index ^= 1),
                Box::new(|c| {
                    c.contents.procedure_constraints.pop();
                }),
                Box::new(|c| {
                    c.contents.dynamic_fixups.pop();
                }),
                Box::new(|c| c.contents.dynamic_fixups[0].storage_section_index = 9),
                Box::new(|c| c.contents.dynamic_fixups[0].target_section_index ^= 1),
                Box::new(|c| c.non_authoritative_payload_roster_compatibility_fingerprint ^= 1),
            ];
            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxArm64);
                let identity = candidate
                    .section_headers
                    .non_authoritative_template_compatibility_fingerprint();
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("corrupt indexed payloads must reject");
                assert_eq!(
                    error
                        .candidate
                        .section_headers
                        .non_authoritative_template_compatibility_fingerprint(),
                    identity
                );
            }
        }

        #[test]
        fn malformed_bounds_widths_and_arithmetic_reject_without_panicking() {
            assert!(checked_sum(usize::MAX, 8, "sum").is_err());
            assert!(checked_u32(usize::MAX, "word").is_err());
            assert!(read_field(&[], usize::MAX, 8, "test fixup field").is_err());
            assert!(read_field(&[0; 3], 0, 4, "test fixup field").is_err());
            assert!(read_field(&[0; 8], 0, 3, "test fixup field").is_err());
        }
    }
}

pub(crate) mod section_roster {
    use super::super::checked::{self, CandidateValidationError};
    use crate::bytes::require_name;
    use crate::constants::SECTION_COUNT;
    use crate::dynamic_executable::checked::{checked_u32, require};
    use crate::dynamic_executable::import_sections::dynamic_section_descriptors::ElfDynamicSectionKind;
    use crate::dynamic_executable::procedure_linkage::dynamic_linkage_descriptors::{
        ElfProcedureLinkageSectionInfo, ElfProcedureLinkageSectionKind,
        ElfProcedureLinkageSectionLink,
    };
    use crate::dynamic_executable::section_headers::section_name_table::{
        ElfSectionNameTableSectionKind, ValidatedElfSectionNameTablePlan,
    };
    use diagnostics::Diagnostic;

    const SECTION_NAME_TABLE_INDEX: u32 = 13;
    const SHT_NULL: u32 = 0;
    const CANONICAL_KINDS: [ElfDynamicRosterSectionKind; SECTION_COUNT] = [
        ElfDynamicRosterSectionKind::Null,
        ElfDynamicRosterSectionKind::Interpreter,
        ElfDynamicRosterSectionKind::DynamicString,
        ElfDynamicRosterSectionKind::DynamicSymbol,
        ElfDynamicRosterSectionKind::SystemVHash,
        ElfDynamicRosterSectionKind::GnuSymbolVersion,
        ElfDynamicRosterSectionKind::GnuVersionRequirement,
        ElfDynamicRosterSectionKind::GnuHash,
        ElfDynamicRosterSectionKind::ProcedureLinkage,
        ElfDynamicRosterSectionKind::ProcedureGot,
        ElfDynamicRosterSectionKind::ProcedureRelocation,
        ElfDynamicRosterSectionKind::GeneralRelocation,
        ElfDynamicRosterSectionKind::DynamicTable,
        ElfDynamicRosterSectionKind::SectionNameTable,
    ];

    #[derive(Debug)]
    #[must_use = "validated ELF section roster retains the complete name-table owner"]
    pub struct ValidatedElfDynamicSectionRoster {
        section_names: ValidatedElfSectionNameTablePlan,
        contents: ElfDynamicSectionRosterContents,
        non_authoritative_roster_compatibility_fingerprint: u64,
    }

    impl ValidatedElfDynamicSectionRoster {
        pub const fn section_names(&self) -> &ValidatedElfSectionNameTablePlan {
            &self.section_names
        }

        pub fn section_count(&self) -> usize {
            self.contents.rows.len()
        }

        pub const fn section_name_table_index(&self) -> u32 {
            self.contents.section_name_table_index
        }

        pub const fn non_authoritative_roster_compatibility_fingerprint(&self) -> u64 {
            self.non_authoritative_roster_compatibility_fingerprint
        }

        pub(crate) const fn contents(&self) -> &ElfDynamicSectionRosterContents {
            &self.contents
        }

        pub(crate) fn into_parts(
            self,
        ) -> (
            ValidatedElfSectionNameTablePlan,
            ElfDynamicSectionRosterContents,
        ) {
            (self.section_names, self.contents)
        }
    }

    #[derive(Debug)]
    #[must_use = "ELF section-roster rejection retains the section-name table"]
    pub struct ElfDynamicSectionRosterPlanningError {
        section_names: ValidatedElfSectionNameTablePlan,
        diagnostic: Diagnostic,
    }

    image::rejection_carrier!(
        ElfDynamicSectionRosterPlanningError;
        section_names: ValidatedElfSectionNameTablePlan,
    );

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct ElfDynamicSectionRosterContents {
        pub(crate) rows: Vec<ElfNumericSectionDescriptor>,
        pub(crate) section_name_table_index: u32,
    }

    pub(crate) type ElfDynamicRosterSectionKind = crate::ElfPlacedDynamicSectionKind;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct ElfNumericSectionDescriptor {
        pub(crate) kind: ElfDynamicRosterSectionKind,
        pub(crate) index: u32,
        pub(crate) name_offset: u32,
        pub(crate) section_type: u32,
        pub(crate) flags: u64,
        pub(crate) payload_size: u64,
        pub(crate) alignment: u64,
        pub(crate) entry_size: u64,
        pub(crate) link: u32,
        pub(crate) info: u32,
    }

    struct Candidate {
        section_names: ValidatedElfSectionNameTablePlan,
        contents: ElfDynamicSectionRosterContents,
        non_authoritative_roster_compatibility_fingerprint: u64,
    }

    pub(crate) fn plan_elf_dynamic_section_roster(
        section_names: ValidatedElfSectionNameTablePlan,
    ) -> Result<ValidatedElfDynamicSectionRoster, Box<ElfDynamicSectionRosterPlanningError>> {
        checked::plan_stage!(
            plan: section_names,
            derive: derive_contents,
            contents: contents,
            fingerprint: non_authoritative_roster_compatibility_fingerprint,
            error: ElfDynamicSectionRosterPlanningError,
        )
    }

    fn derive_contents(
        section_names: &ValidatedElfSectionNameTablePlan,
    ) -> Result<ElfDynamicSectionRosterContents, Diagnostic> {
        let mut rows = Vec::with_capacity(SECTION_COUNT);
        rows.push(ElfNumericSectionDescriptor {
            kind: ElfDynamicRosterSectionKind::Null,
            index: 0,
            name_offset: 0,
            section_type: SHT_NULL,
            flags: 0,
            payload_size: 0,
            alignment: 0,
            entry_size: 0,
            link: 0,
            info: 0,
        });

        for source in &base_descriptor_contents(section_names).descriptors {
            let kind = roster_kind_from_dynamic(source.kind);
            rows.push(ElfNumericSectionDescriptor {
                kind,
                index: index_for_kind(kind),
                name_offset: source.name_offset,
                section_type: source.section_type,
                flags: source.flags,
                payload_size: source.payload_size,
                alignment: source.alignment,
                entry_size: source.entry_size,
                link: source.link.map(index_for_dynamic_kind).unwrap_or(0),
                info: source.info,
            });
        }

        for source in &linkage_descriptor_contents(section_names).descriptors {
            let kind = roster_kind_from_linkage(source.kind);
            rows.push(ElfNumericSectionDescriptor {
                kind,
                index: index_for_kind(kind),
                name_offset: source.name_offset,
                section_type: source.section_type,
                flags: source.flags,
                payload_size: source.payload_size,
                alignment: source.alignment,
                entry_size: source.entry_size,
                link: match source.link {
                    ElfProcedureLinkageSectionLink::None => 0,
                    ElfProcedureLinkageSectionLink::DynamicSymbol => {
                        index_for_dynamic_kind(ElfDynamicSectionKind::DynamicSymbol)
                    }
                },
                info: match source.info {
                    ElfProcedureLinkageSectionInfo::None => 0,
                    ElfProcedureLinkageSectionInfo::RelocatedSection(kind) => {
                        index_for_kind(roster_kind_from_linkage(kind))
                    }
                },
            });
        }

        let source = &section_names.dynamic_table().contents().descriptor;
        rows.push(ElfNumericSectionDescriptor {
            kind: ElfDynamicRosterSectionKind::DynamicTable,
            index: index_for_kind(ElfDynamicRosterSectionKind::DynamicTable),
            name_offset: source.name_offset,
            section_type: source.section_type,
            flags: source.flags,
            payload_size: source.payload_size,
            alignment: source.alignment,
            entry_size: source.entry_size,
            link: source.link.map(index_for_dynamic_kind).unwrap_or(0),
            info: source.info.map(index_for_dynamic_kind).unwrap_or(0),
        });

        let source = &section_names.contents().descriptor;
        rows.push(ElfNumericSectionDescriptor {
            kind: ElfDynamicRosterSectionKind::SectionNameTable,
            index: index_for_kind(ElfDynamicRosterSectionKind::SectionNameTable),
            name_offset: source.name_offset,
            section_type: source.section_type,
            flags: source.flags,
            payload_size: source.payload_size,
            alignment: source.alignment,
            entry_size: source.entry_size,
            link: source.link.map(index_for_dynamic_kind).unwrap_or(0),
            info: source.info.map(index_for_dynamic_kind).unwrap_or(0),
        });
        Ok(ElfDynamicSectionRosterContents {
            rows,
            section_name_table_index: SECTION_NAME_TABLE_INDEX,
        })
    }

    fn base_descriptor_contents(
        section_names: &ValidatedElfSectionNameTablePlan,
    ) -> &crate::dynamic_executable::import_sections::dynamic_section_descriptors::ElfDynamicSectionDescriptorContents{
        section_names
            .dynamic_table()
            .payload()
            .plan()
            .descriptors()
            .templates()
            .linkage()
            .descriptors()
            .contents()
    }

    fn linkage_descriptor_contents(
        section_names: &ValidatedElfSectionNameTablePlan,
    ) -> &crate::dynamic_executable::procedure_linkage::dynamic_linkage_descriptors::ElfProcedureLinkageSectionDescriptorContents{
        section_names
            .dynamic_table()
            .payload()
            .plan()
            .descriptors()
            .contents()
    }

    const fn roster_kind_from_dynamic(kind: ElfDynamicSectionKind) -> ElfDynamicRosterSectionKind {
        match kind {
            ElfDynamicSectionKind::Interpreter => ElfDynamicRosterSectionKind::Interpreter,
            ElfDynamicSectionKind::DynamicString => ElfDynamicRosterSectionKind::DynamicString,
            ElfDynamicSectionKind::DynamicSymbol => ElfDynamicRosterSectionKind::DynamicSymbol,
            ElfDynamicSectionKind::SystemVHash => ElfDynamicRosterSectionKind::SystemVHash,
            ElfDynamicSectionKind::GnuSymbolVersion => {
                ElfDynamicRosterSectionKind::GnuSymbolVersion
            }
            ElfDynamicSectionKind::GnuVersionRequirement => {
                ElfDynamicRosterSectionKind::GnuVersionRequirement
            }
            ElfDynamicSectionKind::GnuHash => ElfDynamicRosterSectionKind::GnuHash,
        }
    }

    const fn roster_kind_from_linkage(
        kind: ElfProcedureLinkageSectionKind,
    ) -> ElfDynamicRosterSectionKind {
        match kind {
            ElfProcedureLinkageSectionKind::ProcedureLinkage => {
                ElfDynamicRosterSectionKind::ProcedureLinkage
            }
            ElfProcedureLinkageSectionKind::ProcedureGot => {
                ElfDynamicRosterSectionKind::ProcedureGot
            }
            ElfProcedureLinkageSectionKind::ProcedureRelocation => {
                ElfDynamicRosterSectionKind::ProcedureRelocation
            }
            ElfProcedureLinkageSectionKind::GeneralRelocation => {
                ElfDynamicRosterSectionKind::GeneralRelocation
            }
        }
    }

    const fn index_for_dynamic_kind(kind: ElfDynamicSectionKind) -> u32 {
        index_for_kind(roster_kind_from_dynamic(kind))
    }

    const fn index_for_kind(kind: ElfDynamicRosterSectionKind) -> u32 {
        kind as u32
    }

    checked::seal_stage! {
        field: section_names,
        contents: contents,
        validated: ValidatedElfDynamicSectionRoster,
        validate: validate_contents,
        fingerprint: non_authoritative_roster_compatibility_fingerprint,
        message: "ELF dynamic section-roster compatibility fingerprint does not replay",
    }

    fn validate_contents(
        section_names: &ValidatedElfSectionNameTablePlan,
        contents: &ElfDynamicSectionRosterContents,
    ) -> Result<(), Diagnostic> {
        require(
            section_names.descriptor_count() == SECTION_COUNT - 1,
            "numeric ELF roster requires the exact sealed thirteen semantic descriptors",
        )?;
        require(
            section_names.byte_count() == 122,
            "numeric ELF roster requires the exact complete 122-byte name table",
        )?;
        require(
            contents.rows.len() == SECTION_COUNT,
            "numeric ELF roster must contain exactly fourteen rows",
        )?;
        require(
            contents.section_name_table_index == SECTION_NAME_TABLE_INDEX,
            "numeric ELF roster e_shstrndx selection is not exact",
        )?;

        for (ordinal, expected_kind) in CANONICAL_KINDS.iter().enumerate() {
            let row = contents
                .rows
                .get(ordinal)
                .ok_or_else(|| Diagnostic::error("numeric ELF roster row is missing"))?;
            let expected_index = checked_u32(ordinal, "numeric ELF section index")?;
            require(
                row.kind == *expected_kind
                    && row.index == expected_index
                    && row.index == index_for_kind(*expected_kind)
                    && contents
                        .rows
                        .iter()
                        .filter(|candidate| candidate.kind == *expected_kind)
                        .count()
                        == 1,
                "numeric ELF roster kinds or indexes are missing, duplicated, or reordered",
            )?;
            validate_name(section_names.contents().bytes.as_slice(), row)?;
            validate_row_against_owner(section_names, row)?;
        }
        validate_references(contents)
    }

    fn validate_name(
        section_names: &[u8],
        row: &ElfNumericSectionDescriptor,
    ) -> Result<(), Diagnostic> {
        require_name(
            section_names,
            row.name_offset,
            expected_name(row.kind),
            "numeric ELF sh_name",
        )
    }

    const fn expected_name(kind: ElfDynamicRosterSectionKind) -> &'static [u8] {
        match kind {
            ElfDynamicRosterSectionKind::Null => b"",
            ElfDynamicRosterSectionKind::Interpreter => b".interp",
            ElfDynamicRosterSectionKind::DynamicString => b".dynstr",
            ElfDynamicRosterSectionKind::DynamicSymbol => b".dynsym",
            ElfDynamicRosterSectionKind::SystemVHash => b".hash",
            ElfDynamicRosterSectionKind::GnuSymbolVersion => b".gnu.version",
            ElfDynamicRosterSectionKind::GnuVersionRequirement => b".gnu.version_r",
            ElfDynamicRosterSectionKind::GnuHash => b".gnu.hash",
            ElfDynamicRosterSectionKind::ProcedureLinkage => b".plt",
            ElfDynamicRosterSectionKind::ProcedureGot => b".got.plt",
            ElfDynamicRosterSectionKind::ProcedureRelocation => b".rela.plt",
            ElfDynamicRosterSectionKind::GeneralRelocation => b".rela.dyn",
            ElfDynamicRosterSectionKind::DynamicTable => b".dynamic",
            ElfDynamicRosterSectionKind::SectionNameTable => b".shstrtab",
        }
    }

    fn validate_row_against_owner(
        section_names: &ValidatedElfSectionNameTablePlan,
        row: &ElfNumericSectionDescriptor,
    ) -> Result<(), Diagnostic> {
        let expected = match row.kind {
            ElfDynamicRosterSectionKind::Null => ElfNumericSectionDescriptor {
                kind: ElfDynamicRosterSectionKind::Null,
                index: 0,
                name_offset: 0,
                section_type: SHT_NULL,
                flags: 0,
                payload_size: 0,
                alignment: 0,
                entry_size: 0,
                link: 0,
                info: 0,
            },
            ElfDynamicRosterSectionKind::Interpreter
            | ElfDynamicRosterSectionKind::DynamicString
            | ElfDynamicRosterSectionKind::DynamicSymbol
            | ElfDynamicRosterSectionKind::SystemVHash
            | ElfDynamicRosterSectionKind::GnuSymbolVersion
            | ElfDynamicRosterSectionKind::GnuVersionRequirement
            | ElfDynamicRosterSectionKind::GnuHash => {
                let source = base_descriptor_contents(section_names)
                    .descriptors
                    .iter()
                    .find(|source| roster_kind_from_dynamic(source.kind) == row.kind)
                    .ok_or_else(|| Diagnostic::error("base ELF descriptor owner row is missing"))?;
                ElfNumericSectionDescriptor {
                    kind: row.kind,
                    index: index_for_kind(row.kind),
                    name_offset: source.name_offset,
                    section_type: source.section_type,
                    flags: source.flags,
                    payload_size: source.payload_size,
                    alignment: source.alignment,
                    entry_size: source.entry_size,
                    link: source.link.map(index_for_dynamic_kind).unwrap_or(0),
                    info: source.info,
                }
            }
            ElfDynamicRosterSectionKind::ProcedureLinkage
            | ElfDynamicRosterSectionKind::ProcedureGot
            | ElfDynamicRosterSectionKind::ProcedureRelocation
            | ElfDynamicRosterSectionKind::GeneralRelocation => {
                let source = linkage_descriptor_contents(section_names)
                    .descriptors
                    .iter()
                    .find(|source| roster_kind_from_linkage(source.kind) == row.kind)
                    .ok_or_else(|| {
                        Diagnostic::error("linkage ELF descriptor owner row is missing")
                    })?;
                ElfNumericSectionDescriptor {
                    kind: row.kind,
                    index: index_for_kind(row.kind),
                    name_offset: source.name_offset,
                    section_type: source.section_type,
                    flags: source.flags,
                    payload_size: source.payload_size,
                    alignment: source.alignment,
                    entry_size: source.entry_size,
                    link: match source.link {
                        ElfProcedureLinkageSectionLink::None => 0,
                        ElfProcedureLinkageSectionLink::DynamicSymbol => 3,
                    },
                    info: match source.info {
                        ElfProcedureLinkageSectionInfo::None => 0,
                        ElfProcedureLinkageSectionInfo::RelocatedSection(kind) => {
                            index_for_kind(roster_kind_from_linkage(kind))
                        }
                    },
                }
            }
            ElfDynamicRosterSectionKind::DynamicTable => {
                let source = &section_names.dynamic_table().contents().descriptor;
                ElfNumericSectionDescriptor {
                    kind: row.kind,
                    index: 12,
                    name_offset: source.name_offset,
                    section_type: source.section_type,
                    flags: source.flags,
                    payload_size: source.payload_size,
                    alignment: source.alignment,
                    entry_size: source.entry_size,
                    link: source.link.map(index_for_dynamic_kind).unwrap_or(0),
                    info: source.info.map(index_for_dynamic_kind).unwrap_or(0),
                }
            }
            ElfDynamicRosterSectionKind::SectionNameTable => {
                let source = &section_names.contents().descriptor;
                require(
                    source.kind == ElfSectionNameTableSectionKind::SectionNameTable,
                    "section-name-table descriptor kind drifted before numeric roster",
                )?;
                ElfNumericSectionDescriptor {
                    kind: row.kind,
                    index: 13,
                    name_offset: source.name_offset,
                    section_type: source.section_type,
                    flags: source.flags,
                    payload_size: source.payload_size,
                    alignment: source.alignment,
                    entry_size: source.entry_size,
                    link: source.link.map(index_for_dynamic_kind).unwrap_or(0),
                    info: source.info.map(index_for_dynamic_kind).unwrap_or(0),
                }
            }
        };
        require(
            *row == expected,
            "numeric ELF section row drifted from its exact semantic owner",
        )
    }

    fn validate_references(contents: &ElfDynamicSectionRosterContents) -> Result<(), Diagnostic> {
        for row in &contents.rows {
            require(
                row.link < SECTION_COUNT as u32,
                "numeric ELF sh_link reference is outside the closed roster",
            )?;
        }
        let row = |kind| {
            contents
                .rows
                .get(index_for_kind(kind) as usize)
                .ok_or_else(|| Diagnostic::error("numeric ELF reference target is missing"))
        };
        require(
            row(ElfDynamicRosterSectionKind::DynamicSymbol)?.link == 2
                && row(ElfDynamicRosterSectionKind::DynamicSymbol)?.info == 1
                && row(ElfDynamicRosterSectionKind::SystemVHash)?.link == 3
                && row(ElfDynamicRosterSectionKind::GnuHash)?.link == 3
                && row(ElfDynamicRosterSectionKind::GnuSymbolVersion)?.link == 3
                && row(ElfDynamicRosterSectionKind::GnuVersionRequirement)?.link == 2
                && row(ElfDynamicRosterSectionKind::ProcedureRelocation)?.link == 3
                && row(ElfDynamicRosterSectionKind::ProcedureRelocation)?.info == 9
                && row(ElfDynamicRosterSectionKind::GeneralRelocation)?.link == 3
                && row(ElfDynamicRosterSectionKind::GeneralRelocation)?.info == 0
                && row(ElfDynamicRosterSectionKind::DynamicTable)?.link == 2,
            "numeric ELF link/info relationships do not resolve to the canonical roster",
        )
    }

    fn non_authoritative_roster_compatibility_fingerprint(
        section_names: &ValidatedElfSectionNameTablePlan,
        contents: &ElfDynamicSectionRosterContents,
    ) -> u64 {
        let mut hash = checked::stage_fingerprint(
            b"omega.elf-dynamic-section-roster.v2",
            section_names.non_authoritative_table_compatibility_fingerprint(),
        );
        hash.bytes(&contents.section_name_table_index.to_le_bytes());
        hash.bytes(&(contents.rows.len() as u64).to_le_bytes());
        for row in &contents.rows {
            hash.byte(row.kind as u8);
            hash.bytes(&row.index.to_le_bytes());
            hash.bytes(&row.name_offset.to_le_bytes());
            hash.bytes(&row.section_type.to_le_bytes());
            hash.bytes(&row.flags.to_le_bytes());
            hash.bytes(&row.payload_size.to_le_bytes());
            hash.bytes(&row.alignment.to_le_bytes());
            hash.bytes(&row.entry_size.to_le_bytes());
            hash.bytes(&row.link.to_le_bytes());
            hash.bytes(&row.info.to_le_bytes());
        }
        hash.finish()
    }

    #[cfg(test)]
    mod tests {
        use super::{
            CANONICAL_KINDS, Candidate, ElfDynamicRosterSectionKind, ElfNumericSectionDescriptor,
            ValidatedElfSectionNameTablePlan, checked_u32, derive_contents, index_for_kind,
            non_authoritative_roster_compatibility_fingerprint, plan_elf_dynamic_section_roster,
            validate_candidate, validate_contents, validate_name, validate_row_against_owner,
        };
        use target::TargetProfile;

        const IMPORTS: [crate::test_support::DynamicImportFixture; 2] =
            crate::test_support::PAIR_IMPORTS;

        use crate::test_support::DynamicImportFixture as ImportFixture;
        fn section_names(
            target: TargetProfile,
            imports: &[ImportFixture],
        ) -> ValidatedElfSectionNameTablePlan {
            let inputs = crate::test_support::dynamic_import_inputs(
                target,
                imports,
                "__omega_section_roster_import_",
                "valid section-roster locator",
                "valid section-roster interpreter",
            );
            crate::test_support::dynamic_section_names(inputs)
        }

        fn candidate(target: TargetProfile) -> Candidate {
            let section_names = section_names(target, &IMPORTS);
            let contents = derive_contents(&section_names).expect("derived section roster");
            let non_authoritative_roster_compatibility_fingerprint =
                non_authoritative_roster_compatibility_fingerprint(&section_names, &contents);
            Candidate {
                section_names,
                contents,
                non_authoritative_roster_compatibility_fingerprint,
            }
        }

        #[test]
        fn both_targets_close_exact_fourteen_row_roster_and_shstrndx() {
            for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
                let roster = plan_elf_dynamic_section_roster(section_names(target, &IMPORTS))
                    .expect("validated section roster");
                assert_eq!(roster.section_count(), 14);
                assert_eq!(roster.section_name_table_index(), 13);
                assert_eq!(roster.contents.rows.len(), 14);
                assert_eq!(
                    roster
                        .contents
                        .rows
                        .iter()
                        .map(|row| row.kind)
                        .collect::<Vec<_>>(),
                    CANONICAL_KINDS,
                );
                for (index, row) in roster.contents.rows.iter().enumerate() {
                    assert_eq!(row.index, index as u32);
                    validate_name(roster.section_names.contents().bytes.as_slice(), row).unwrap();
                    validate_row_against_owner(roster.section_names(), row).unwrap();
                }
                assert_ne!(
                    roster.non_authoritative_roster_compatibility_fingerprint(),
                    0
                );
                validate_contents(roster.section_names(), &roster.contents).unwrap();
            }
        }

        #[test]
        fn import_permutation_preserves_roster_identity_and_target_remains_bound() {
            let forward =
                plan_elf_dynamic_section_roster(section_names(TargetProfile::LinuxX64, &IMPORTS))
                    .unwrap();
            let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
            let reverse = plan_elf_dynamic_section_roster(section_names(
                TargetProfile::LinuxX64,
                &reverse_imports,
            ))
            .unwrap();
            let arm =
                plan_elf_dynamic_section_roster(section_names(TargetProfile::LinuxArm64, &IMPORTS))
                    .unwrap();
            assert_eq!(forward.contents, reverse.contents);
            assert_eq!(
                forward.non_authoritative_roster_compatibility_fingerprint(),
                reverse.non_authoritative_roster_compatibility_fingerprint()
            );
            assert_ne!(
                forward.non_authoritative_roster_compatibility_fingerprint(),
                arm.non_authoritative_roster_compatibility_fingerprint()
            );
        }

        #[test]
        fn independent_replay_rejects_missing_duplicate_reordered_and_every_field_corruption() {
            let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
                Box::new(|candidate| {
                    candidate.contents.rows.pop();
                }),
                Box::new(|candidate| {
                    candidate.contents.rows.push(candidate.contents.rows[0]);
                }),
                Box::new(|candidate| candidate.contents.rows.swap(1, 2)),
                Box::new(|candidate| {
                    candidate.contents.rows[1].kind = ElfDynamicRosterSectionKind::Null
                }),
                Box::new(|candidate| candidate.contents.rows[1].index = u32::MAX),
                Box::new(|candidate| candidate.contents.rows[1].name_offset = u32::MAX),
                Box::new(|candidate| candidate.contents.rows[1].section_type ^= 1),
                Box::new(|candidate| candidate.contents.rows[1].flags ^= 1),
                Box::new(|candidate| candidate.contents.rows[1].payload_size += 1),
                Box::new(|candidate| candidate.contents.rows[1].alignment += 1),
                Box::new(|candidate| candidate.contents.rows[1].entry_size = 1),
                Box::new(|candidate| candidate.contents.rows[1].link = 2),
                Box::new(|candidate| candidate.contents.rows[1].info = 1),
                Box::new(|candidate| candidate.contents.section_name_table_index = 10),
                Box::new(|candidate| {
                    candidate.non_authoritative_roster_compatibility_fingerprint ^= 1
                }),
            ];
            for corrupt in corruptions {
                let mut candidate = candidate(TargetProfile::LinuxArm64);
                let expected_identity = candidate
                    .section_names
                    .non_authoritative_table_compatibility_fingerprint();
                corrupt(&mut candidate);
                let error = validate_candidate(candidate)
                    .expect_err("corrupt numeric roster candidate must reject");
                assert_eq!(
                    error
                        .candidate
                        .section_names
                        .non_authoritative_table_compatibility_fingerprint(),
                    expected_identity,
                );
            }
        }

        #[test]
        fn each_resolved_reference_and_literal_info_rejects_drift_with_custody() {
            for (kind, field) in [
                (ElfDynamicRosterSectionKind::DynamicSymbol, true),
                (ElfDynamicRosterSectionKind::SystemVHash, true),
                (ElfDynamicRosterSectionKind::GnuHash, true),
                (ElfDynamicRosterSectionKind::GnuSymbolVersion, true),
                (ElfDynamicRosterSectionKind::GnuVersionRequirement, true),
                (ElfDynamicRosterSectionKind::ProcedureRelocation, true),
                (ElfDynamicRosterSectionKind::ProcedureRelocation, false),
                (ElfDynamicRosterSectionKind::DynamicTable, true),
                (ElfDynamicRosterSectionKind::DynamicSymbol, false),
                (ElfDynamicRosterSectionKind::GnuVersionRequirement, false),
            ] {
                let mut candidate = candidate(TargetProfile::LinuxX64);
                let expected_identity = candidate
                    .section_names
                    .non_authoritative_table_compatibility_fingerprint();
                let row = &mut candidate.contents.rows[index_for_kind(kind) as usize];
                if field {
                    row.link = row.link.saturating_add(1);
                } else {
                    row.info = row.info.saturating_add(1);
                }
                let error = validate_candidate(candidate).expect_err("reference drift must reject");
                assert_eq!(
                    error
                        .candidate
                        .section_names
                        .non_authoritative_table_compatibility_fingerprint(),
                    expected_identity,
                );
            }
        }

        #[test]
        fn malformed_indexes_names_and_references_reject_without_panicking() {
            assert!(checked_u32(usize::MAX, "index").is_err());
            let names = b"\0.interp\0";
            let mut descriptor = ElfNumericSectionDescriptor {
                kind: ElfDynamicRosterSectionKind::Interpreter,
                index: 1,
                name_offset: u32::MAX,
                section_type: 1,
                flags: 2,
                payload_size: 1,
                alignment: 1,
                entry_size: 0,
                link: 0,
                info: 0,
            };
            assert!(validate_name(names, &descriptor).is_err());
            descriptor.name_offset = 1;
            assert!(validate_name(b"\0.interp", &descriptor).is_err());
            descriptor.name_offset = 2;
            assert!(validate_name(names, &descriptor).is_err());

            let mut candidate = candidate(TargetProfile::LinuxX64);
            candidate.contents.rows[3].link = u32::MAX;
            assert!(validate_candidate(candidate).is_err());
        }
    }
}
