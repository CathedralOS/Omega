//! Relative file packing for the closed ELF dynamic-section payload roster.
//!
//! This layer assigns each non-null payload an exact offset relative to its
//! future permission-homogeneous file region. Keeping read-only, executable,
//! writable, and non-allocated rows in distinct domains avoids pre-choosing an
//! unsafe load-segment policy. It deliberately does not select any region's
//! absolute file offset or virtual address, resolve a section-header fixup,
//! emit a program header, or mutate an image.

use target::compatibility_fingerprint::Fnv1a;

use super::super::checked::{self, CandidateValidationError};
use crate::dynamic_executable::checked::require;
use crate::dynamic_executable::dynamic_table::dynamic_table_descriptor::ValidatedElfDynamicTableSectionDescriptorPlan;
use crate::dynamic_executable::section_headers::section_payload_roster::ValidatedElfIndexedSectionPayloadPlan;
use crate::dynamic_executable::section_headers::section_roster::ElfDynamicRosterSectionKind;
use diagnostics::Diagnostic;

use crate::constants::{SECTION_COUNT, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE};
/// Closed relative-packing domain derived from exact ELF section flags.
///
/// This classifies candidate section payloads; it does not grant a load
/// segment, page permission, or absolute placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ElfRelativeSectionPayloadRegion {
    ReadOnly = 1,
    ReadExecute = 2,
    ReadWrite = 3,
    FileOnly = 4,
}

/// Independently replayed relative file placement of the exact indexed ELF
/// payload roster.
///
/// Each region's retained offsets start at zero and therefore are not ELF
/// `sh_offset` values. The carrier owns no absolute placement, virtual address,
/// segment, section-header-fixup, byte-emission, publication, or runnable-image
/// authority.
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

    /// The dynamic-table descriptor plan reachable through this layout's
    /// section-name table — the ABI-relevant dynamic region every consumer
    /// walks the same six accessor hops to reach.
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

    /// Exact byte span occupied inside one relative region, including
    /// intra-region alignment gaps but excluding future page/base alignment.
    pub const fn region_span(&self, region: ElfRelativeSectionPayloadRegion) -> u64 {
        self.contents.region_spans.get(region)
    }

    /// Compatibility fingerprint over the indexed-payload identity and every
    /// relative placement row. This is not final ELF layout identity.
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

/// Rejected relative layout with exact indexed-payload custody.
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

/// Pack the closed indexed payload roster relative to a future dynamic-section
/// file region.
///
/// Rows remain in numeric section order. Every non-null row begins at the
/// first checked offset in its flag-derived region satisfying retained
/// `sh_addralign`; the null row remains the unique zero-sized, regionless row
/// at offset zero. This function does not choose future region bases or resolve
/// any serialized section-header field.
pub(crate) fn plan_elf_relative_section_payload_layout(
    payloads: ValidatedElfIndexedSectionPayloadPlan,
) -> Result<ValidatedElfRelativeSectionPayloadLayout, Box<ElfRelativeSectionPayloadLayoutError>> {
    checked::plan_candidate(
        payloads,
        derive_contents,
        |payloads, contents| {
            non_authoritative_layout_compatibility_fingerprint(&payloads, &contents)
        },
        |payloads, contents, fingerprint| Candidate {
            payloads: payloads,
            contents: contents,
            non_authoritative_layout_compatibility_fingerprint: fingerprint,
        },
        validate_candidate,
        |candidate| candidate.payloads,
        |payloads, diagnostic| {
            Box::new(ElfRelativeSectionPayloadLayoutError {
                payloads: payloads,
                diagnostic,
            })
        },
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
    for (ordinal, (payload, descriptor)) in payload_rows.iter().zip(roster_rows).enumerate() {
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
            *cursor = crate::dynamic_executable::checked::checked_sum_u64(
                offset,
                byte_size,
                "relative ELF payload end",
                "Elf64_Off",
            )?;
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
    Ok(ElfRelativeSectionPayloadLayoutContents { rows, region_spans })
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfRelativeSectionPayloadLayout, CandidateValidationError<Candidate>> {
    checked::validate_candidate(
        candidate,
        |candidate| validate_contents(&candidate.payloads, &candidate.contents),
        |candidate| {
            candidate.non_authoritative_layout_compatibility_fingerprint
                == non_authoritative_layout_compatibility_fingerprint(
                    &candidate.payloads,
                    &candidate.contents,
                )
        },
        "relative ELF payload-layout compatibility fingerprint does not replay",
        |candidate| ValidatedElfRelativeSectionPayloadLayout {
            payloads: candidate.payloads,
            contents: candidate.contents,
            non_authoritative_layout_compatibility_fingerprint: candidate
                .non_authoritative_layout_compatibility_fingerprint,
        },
    )
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
    for (ordinal, ((placement, payload), descriptor)) in contents
        .rows
        .iter()
        .zip(payload_rows)
        .zip(roster_rows)
        .enumerate()
    {
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
            *cursor = crate::dynamic_executable::checked::checked_sum_u64(
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
        crate::dynamic_executable::checked::checked_sum_u64(
            value,
            alignment - remainder,
            context,
            "Elf64_Off",
        )
    }
}

fn non_authoritative_layout_compatibility_fingerprint(
    payloads: &ValidatedElfIndexedSectionPayloadPlan,
    contents: &ElfRelativeSectionPayloadLayoutContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-relative-section-payload-layout.v1");
    hash.bytes(
        &payloads
            .non_authoritative_payload_roster_compatibility_fingerprint()
            .to_le_bytes(),
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
        ElfRelativeSectionPayloadRegionSpans, SECTION_COUNT, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE,
        ValidatedElfIndexedSectionPayloadPlan, checked_align, classify_region, derive_contents,
        non_authoritative_layout_compatibility_fingerprint,
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
                layout.contents.rows[1..=7]
                    .iter()
                    .all(|row| { row.region == Some(ElfRelativeSectionPayloadRegion::ReadOnly) })
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
            for row in &layout.contents.rows[1..] {
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
        let first =
            plan_elf_relative_section_payload_layout(payloads(TargetProfile::LinuxX64)).unwrap();
        let second =
            plan_elf_relative_section_payload_layout(payloads(TargetProfile::LinuxX64)).unwrap();
        let arm =
            plan_elf_relative_section_payload_layout(payloads(TargetProfile::LinuxArm64)).unwrap();
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
        assert!(
            crate::dynamic_executable::checked::checked_sum_u64(u64::MAX, 1, "sum", "Elf64_Off")
                .is_err()
        );
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
