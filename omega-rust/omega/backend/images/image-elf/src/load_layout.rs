//! Absolute file and virtual placement for the closed dynamic ELF roster.
//!
//! This layer consumes relative permission-domain packing, binds it to the
//! exact retained Linux target, and closes the current five-program-header
//! geometry (`PT_INTERP`, R/RX/RW `PT_LOAD`, and `PT_DYNAMIC`). It also
//! resolves all twenty-three section-header placement obligations as retained
//! values. It does not write those values into the section-header template,
//! resolve payload-internal fixups, mutate image bytes, serialize program
//! headers, or claim runnable-ELF authority. The section-header template gets
//! a file-only coordinate, but its bytes remain unchanged.

use crate::constants::{ELF_HEADER_SIZE, IMAGE_BASE, PROGRAM_HEADER_SIZE};
use crate::dynamic_linkage_templates::ElfProcedureLinkagePlacementConstraintKind;
use crate::relative_section_layout::{
    ElfRelativeSectionPayloadRegion, ValidatedElfRelativeSectionPayloadLayout,
};
use crate::section_header_bytes::ElfSectionHeaderPlacementFixupKind;
use crate::section_payload_roster::ElfIndexedProcedureFixupStorage;
use crate::section_roster::ElfDynamicRosterSectionKind;
use diagnostics::Diagnostic;
use target::TargetProfile;

const SECTION_COUNT: usize = 13;
const PLACEMENT_FIXUP_COUNT: usize = 23;
const DYNAMIC_PROGRAM_HEADER_COUNT: u64 = 5;
const DYNAMIC_MAX_PAGE_SIZE: u64 = 0x1_0000;
const DYNAMIC_LOAD_POLICY_TAG: u8 = 1;
const AARCH64_RELOCATION_PAGE_SIZE: u64 = 0x1000;
const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Ordered program-header role closed by the dynamic load-layout carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ElfLoadProgramHeaderKind {
    Interpreter = 1,
    LoadReadOnly = 2,
    LoadReadExecute = 3,
    LoadReadWrite = 4,
    Dynamic = 5,
}

/// Public observation vocabulary for the closed placed section roster.
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
    DynamicTable = 11,
    SectionNameTable = 12,
}

/// Absolute geometry for one future `Elf64_Phdr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElfLoadProgramHeader {
    kind: ElfLoadProgramHeaderKind,
    flags: u32,
    file_offset: u64,
    virtual_address: u64,
    physical_address: u64,
    file_size: u64,
    memory_size: u64,
    alignment: u64,
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

/// Absolute placement of one row in the closed thirteen-section roster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElfPlacedDynamicSection {
    index: u32,
    kind: ElfPlacedDynamicSectionKind,
    region: Option<ElfRelativeSectionPayloadRegion>,
    file_offset: u64,
    virtual_address: Option<u64>,
    byte_size: u64,
    alignment: u64,
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

    pub const fn alignment(&self) -> u64 {
        self.alignment
    }
}

/// Meaning of a retained section-header placement resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ElfSectionPlacementResolutionKind {
    VirtualAddress = 1,
    FileOffset = 2,
}

/// One resolved value for an existing typed section-header fixup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElfResolvedSectionHeaderPlacement {
    row_index: u32,
    section_kind: ElfPlacedDynamicSectionKind,
    byte_offset: usize,
    byte_width: u8,
    kind: ElfSectionPlacementResolutionKind,
    value: u64,
}

/// Placement of the retained non-section `FinalImage` memory domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElfLoadImageMemoryPlacement {
    text_file_offset: u64,
    text_virtual_address: u64,
    text_size: u64,
    data_file_offset: u64,
    data_virtual_address: u64,
    data_size: u64,
    bss_virtual_address: u64,
    bss_size: u64,
    bss_alignment: u64,
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

/// Replayed absolute geometry retaining the exact relative-layout owner.
#[derive(Debug)]
#[must_use = "validated dynamic ELF load layout retains all upstream payload custody"]
pub struct ValidatedElfDynamicLoadLayout {
    relative: ValidatedElfRelativeSectionPayloadLayout,
    target: TargetProfile,
    image_base: u64,
    max_page_alignment: u64,
    program_headers: Vec<ElfLoadProgramHeader>,
    image_memory: ElfLoadImageMemoryPlacement,
    sections: Vec<ElfPlacedDynamicSection>,
    section_header_table_file_offset: u64,
    section_header_resolutions: Vec<ElfResolvedSectionHeaderPlacement>,
    non_authoritative_layout_compatibility_fingerprint: u64,
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

    /// Address observation required by later source relocation replay.
    pub const fn final_image_layout(&self) -> image::FinalImageLayout {
        image::FinalImageLayout {
            text_address: self.image_memory.text_virtual_address,
            data_address: self.image_memory.data_virtual_address,
            bss_address: self.image_memory.bss_virtual_address,
        }
    }

    /// Crate-private custody access for later exact ELF serialization rungs.
    /// Public consumers must not bypass the validated layout carrier to recover
    /// the mutable source image.
    pub(crate) fn retained_image(&self) -> &image::FinalImage {
        retained_image(&self.relative)
    }

    pub fn sections(&self) -> &[ElfPlacedDynamicSection] {
        &self.sections
    }

    pub fn section_header_resolutions(&self) -> &[ElfResolvedSectionHeaderPlacement] {
        &self.section_header_resolutions
    }

    pub const fn section_header_table_file_offset(&self) -> u64 {
        self.section_header_table_file_offset
    }

    pub fn section_header_table_byte_size(&self) -> usize {
        self.relative.payloads().section_headers().byte_count()
    }

    pub fn dynamic_section(&self) -> &ElfPlacedDynamicSection {
        &self.sections[ElfPlacedDynamicSectionKind::DynamicTable as usize]
    }

    /// Compatibility fingerprint only; this is not a runnable-image identity.
    pub const fn non_authoritative_layout_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_layout_compatibility_fingerprint
    }

    pub(crate) fn into_relative(self) -> ValidatedElfRelativeSectionPayloadLayout {
        self.relative
    }
}

/// Rejected absolute placement with exact relative-layout custody.
#[derive(Debug)]
#[must_use = "dynamic ELF load-layout rejection retains the relative layout"]
pub struct ElfDynamicLoadLayoutError {
    relative: ValidatedElfRelativeSectionPayloadLayout,
    diagnostic: Diagnostic,
}

impl ElfDynamicLoadLayoutError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfRelativeSectionPayloadLayout, Diagnostic) {
        (self.relative, self.diagnostic)
    }
}

impl std::fmt::Display for ElfDynamicLoadLayoutError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfDynamicLoadLayoutError {}

struct Candidate {
    relative: ValidatedElfRelativeSectionPayloadLayout,
    target: TargetProfile,
    image_base: u64,
    max_page_alignment: u64,
    program_headers: Vec<ElfLoadProgramHeader>,
    image_memory: ElfLoadImageMemoryPlacement,
    sections: Vec<ElfPlacedDynamicSection>,
    section_header_table_file_offset: u64,
    section_header_resolutions: Vec<ElfResolvedSectionHeaderPlacement>,
    non_authoritative_layout_compatibility_fingerprint: u64,
}

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Close absolute load and section geometry for the retained Linux target.
///
/// Both admitted profiles use a target-derived 64 KiB maximum-page policy.
/// This is compatible with the AArch64 ELF ABI recommendation and with x86-64
/// Linux loaders, while preserving `p_offset == p_vaddr (mod p_align)`.
pub fn plan_elf_dynamic_load_layout(
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

type DerivedContents = (
    Vec<ElfLoadProgramHeader>,
    ElfLoadImageMemoryPlacement,
    Vec<ElfPlacedDynamicSection>,
    u64,
    Vec<ElfResolvedSectionHeaderPlacement>,
);

fn derive_contents(
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
        "dynamic ELF load placement requires the exact thirteen-row relative roster",
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
        "dynamic ELF load placement requires exactly twenty-three section-header fixups",
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

fn validate_candidate(
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

fn validate_abi(candidate: &Candidate) -> Result<(), Diagnostic> {
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
            checked_sum(pair[0].file_offset, pair[0].file_size, "PT_LOAD extent")?
                <= pair[1].file_offset,
            "dynamic ELF PT_LOAD file extents overlap",
        )?;
        require(
            checked_sum(
                pair[0].virtual_address,
                pair[0].memory_size,
                "PT_LOAD memory extent",
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
            placed_address == checked_sum(candidate.image_base, placed.file_offset, "section map")?
                && placed.file_offset >= load.file_offset
                && checked_sum(placed.file_offset, placed.byte_size, "section file extent")?
                    <= checked_sum(load.file_offset, load.file_size, "load file extent")?
                && placed_address >= load.virtual_address
                && checked_sum(placed_address, placed.byte_size, "section memory extent")?
                    <= checked_sum(load.virtual_address, load.memory_size, "load memory extent")?,
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
    let text_file_end = checked_sum(
        candidate.image_memory.text_file_offset,
        candidate.image_memory.text_size,
        "source text extent",
    )?;
    let text_memory_end = checked_sum(
        candidate.image_memory.text_virtual_address,
        candidate.image_memory.text_size,
        "source text memory extent",
    )?;
    require(
        candidate.image_memory.text_virtual_address
            == checked_sum(
                candidate.image_base,
                candidate.image_memory.text_file_offset,
                "source text mapping",
            )?
            && candidate.image_memory.text_file_offset >= rx.file_offset
            && text_file_end <= checked_sum(rx.file_offset, rx.file_size, "RX file extent")?
            && candidate.image_memory.text_virtual_address >= rx.virtual_address
            && text_memory_end
                <= checked_sum(rx.virtual_address, rx.memory_size, "RX memory extent")?,
        "source text is not mapped exactly inside the executable load",
    )?;
    let rw_file_end = checked_sum(rw.file_offset, rw.file_size, "RW file extent")?;
    let rw_memory_end = checked_sum(rw.virtual_address, rw.memory_size, "RW memory extent")?;
    let data_file_end = checked_sum(
        candidate.image_memory.data_file_offset,
        candidate.image_memory.data_size,
        "source data extent",
    )?;
    let data_memory_end = checked_sum(
        candidate.image_memory.data_virtual_address,
        candidate.image_memory.data_size,
        "source data memory extent",
    )?;
    require(
        candidate.image_memory.data_file_offset >= rw.file_offset
            && data_file_end <= rw_file_end
            && candidate.image_memory.data_virtual_address
                == checked_sum(
                    candidate.image_base,
                    candidate.image_memory.data_file_offset,
                    "source data mapping",
                )?
            && candidate.image_memory.data_virtual_address >= rw.virtual_address
            && data_memory_end <= rw_memory_end
            && candidate.image_memory.bss_virtual_address >= rw.virtual_address
            && candidate.image_memory.bss_virtual_address >= data_memory_end
            && checked_sum(
                candidate.image_memory.bss_virtual_address,
                candidate.image_memory.bss_size,
                "source BSS extent",
            )? <= rw_memory_end,
        "source data or BSS is not contained by the writable load",
    )?;
    let target_alignment = retained_image(&candidate.relative).target.pointer_alignment as u64;
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
        .filter(|section| section.region == Some(ElfRelativeSectionPayloadRegion::ReadExecute))
        .map(|section| section.file_offset)
        .min()
        .ok_or_else(|| Diagnostic::error("executable section roster is empty"))?;
    let last_rw_section_end = candidate
        .sections
        .iter()
        .filter(|section| section.region == Some(ElfRelativeSectionPayloadRegion::ReadWrite))
        .map(|section| checked_sum(section.file_offset, section.byte_size, "RW section extent"))
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
                >= checked_sum(
                    shstrtab.file_offset,
                    shstrtab.byte_size,
                    "`.shstrtab` extent",
                )?,
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
        && checked_sum(inner_start, inner_size, context)?
            <= checked_sum(outer_start, outer_size, context)?)
}

fn validate_deferred_constraint_envelope(candidate: &Candidate) -> Result<(), Diagnostic> {
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
        let source = checked_sum(
            source_base,
            fixup.byte_offset as u64,
            "procedure fixup site",
        )?;
        let target = candidate
            .sections
            .get(fixup.target_section_index as usize)
            .filter(|section| section.index == fixup.target_section_index)
            .ok_or_else(|| Diagnostic::error("procedure fixup target section is absent"))?;
        let target_start = target
            .virtual_address
            .ok_or_else(|| Diagnostic::error("procedure fixup target section is not allocated"))?;
        require(
            target.byte_size != 0,
            "procedure fixup target section has no bytes",
        )?;
        let target_last = checked_sum(
            target_start,
            target.byte_size - 1,
            "procedure target last byte",
        )?;
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
            ElfProcedureLinkagePlacementConstraintKind::Aarch64Load64Low12Aligned => require(
                target.alignment >= 8 && target_start % 8 == 0,
                "AArch64 low-12 load target is not eight-byte aligned",
            )?,
        }
    }
    Ok(())
}

fn aarch64_page_delta_covers_extent(
    source: u64,
    target_start: u64,
    target_size: u64,
) -> Result<bool, Diagnostic> {
    if target_size == 0 {
        return Ok(false);
    }
    let target_last = checked_sum(
        target_start,
        target_size - 1,
        "AArch64 ADRP target last byte",
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

fn retained_target(relative: &ValidatedElfRelativeSectionPayloadLayout) -> TargetProfile {
    relative
        .payloads()
        .section_headers()
        .roster()
        .section_names()
        .dynamic_table()
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

fn retained_image(relative: &ValidatedElfRelativeSectionPayloadLayout) -> &image::FinalImage {
    relative
        .payloads()
        .section_headers()
        .roster()
        .section_names()
        .dynamic_table()
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

fn section(
    sections: &[ElfPlacedDynamicSection],
    kind: ElfPlacedDynamicSectionKind,
) -> Result<&ElfPlacedDynamicSection, Diagnostic> {
    sections
        .get(kind as usize)
        .filter(|section| section.kind == kind && section.index as usize == kind as usize)
        .ok_or_else(|| Diagnostic::error("dynamic ELF placed-section roster drifted"))
}

const fn public_section_kind(kind: ElfDynamicRosterSectionKind) -> ElfPlacedDynamicSectionKind {
    match kind {
        ElfDynamicRosterSectionKind::Null => ElfPlacedDynamicSectionKind::Null,
        ElfDynamicRosterSectionKind::Interpreter => ElfPlacedDynamicSectionKind::Interpreter,
        ElfDynamicRosterSectionKind::DynamicString => ElfPlacedDynamicSectionKind::DynamicString,
        ElfDynamicRosterSectionKind::DynamicSymbol => ElfPlacedDynamicSectionKind::DynamicSymbol,
        ElfDynamicRosterSectionKind::SystemVHash => ElfPlacedDynamicSectionKind::SystemVHash,
        ElfDynamicRosterSectionKind::GnuSymbolVersion => {
            ElfPlacedDynamicSectionKind::GnuSymbolVersion
        }
        ElfDynamicRosterSectionKind::GnuVersionRequirement => {
            ElfPlacedDynamicSectionKind::GnuVersionRequirement
        }
        ElfDynamicRosterSectionKind::GnuHash => ElfPlacedDynamicSectionKind::GnuHash,
        ElfDynamicRosterSectionKind::ProcedureLinkage => {
            ElfPlacedDynamicSectionKind::ProcedureLinkage
        }
        ElfDynamicRosterSectionKind::ProcedureGot => ElfPlacedDynamicSectionKind::ProcedureGot,
        ElfDynamicRosterSectionKind::ProcedureRelocation => {
            ElfPlacedDynamicSectionKind::ProcedureRelocation
        }
        ElfDynamicRosterSectionKind::DynamicTable => ElfPlacedDynamicSectionKind::DynamicTable,
        ElfDynamicRosterSectionKind::SectionNameTable => {
            ElfPlacedDynamicSectionKind::SectionNameTable
        }
    }
}

fn align_for_region(
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

fn placed_coordinates(
    base: u64,
    relative_offset: u64,
    allocated: bool,
) -> Result<(u64, Option<u64>), Diagnostic> {
    let file_offset = checked_sum(base, relative_offset, "absolute ELF section offset")?;
    let virtual_address = allocated
        .then(|| checked_sum(IMAGE_BASE, file_offset, "absolute ELF section address"))
        .transpose()?;
    Ok((file_offset, virtual_address))
}

fn non_authoritative_layout_compatibility_fingerprint(
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
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf.dynamic-load-layout.v1");
    hash.bytes(
        &relative
            .non_authoritative_layout_compatibility_fingerprint()
            .to_le_bytes(),
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
    }
}

fn checked_sum(left: u64, right: u64, context: &str) -> Result<u64, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows Elf64_Off")))
}

fn checked_product(left: u64, right: u64, context: &str) -> Result<u64, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows Elf64_Off")))
}

fn checked_align(value: u64, alignment: u64, context: &str) -> Result<u64, Diagnostic> {
    if alignment <= 1 {
        return Ok(value);
    }
    require(
        alignment.is_power_of_two(),
        "ELF placement alignment is not a power of two",
    )?;
    let mask = alignment - 1;
    checked_sum(value, mask, context).map(|sum| sum & !mask)
}

fn require(condition: bool, message: &str) -> Result<(), Diagnostic> {
    if condition {
        Ok(())
    } else {
        Err(Diagnostic::error(message))
    }
}

struct Fnv1a(u64);

impl Fnv1a {
    const fn new() -> Self {
        Self(FNV_OFFSET_BASIS)
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(FNV_PRIME);
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            self.byte(byte);
        }
    }

    const fn finish(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        plan_elf_dynamic_link_inputs, plan_elf_dynamic_section_descriptors,
        plan_elf_dynamic_section_roster, plan_elf_dynamic_sections,
        plan_elf_dynamic_table_section_descriptor, plan_elf_dynamic_tags,
        plan_elf_indexed_section_payloads, plan_elf_procedure_linkage_relocations,
        plan_elf_procedure_linkage_section_descriptors, plan_elf_procedure_linkage_templates,
        plan_elf_relative_section_payload_layout, plan_elf_section_name_table,
        serialize_elf_dynamic_sections, serialize_elf_dynamic_table,
        serialize_elf_section_header_table,
    };
    use arena::Handle;
    use image::{
        FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
        FinalImageSection, FinalImageSymbol,
    };
    use object_file::{RelocationKind, SymbolKind};
    use target::{
        ForeignLocatorCandidate, TargetProfile, normalize_elf_interpreter_plan,
        normalize_foreign_locator,
    };

    fn relative(target: TargetProfile) -> ValidatedElfRelativeSectionPayloadLayout {
        relative_with_bss_alignment(target, 16)
    }

    fn relative_with_bss_alignment(
        target: TargetProfile,
        bss_alignment: usize,
    ) -> ValidatedElfRelativeSectionPayloadLayout {
        let mut image = FinalImage::with_capacity(
            target.native_target(),
            FinalImageMemory {
                text: vec![0; 32],
                data: vec![0x5a; 13],
                bss_size: 23,
                bss_alignment,
            },
            Handle::invalid(),
            1,
            1,
            1,
        );
        let symbol_handle = image.symbol_table.symbols.insert(FinalImageSymbol {
            name: "__omega_load_layout_import".to_owned(),
            section: FinalImageSection::None,
            offset: 0,
            size: 0,
            kind: SymbolKind::Import,
        });
        image.symbol_table.imports.insert(FinalImageImport {
            symbol_handle,
            import: FinalImageImportPlan::Normalized(
                normalize_foreign_locator(
                    ForeignLocatorCandidate::ElfVersioned {
                        object: b"libload-layout.so".to_vec(),
                        symbol: b"load_layout_call".to_vec(),
                        version: b"LOAD_LAYOUT_1".to_vec(),
                    },
                    target,
                )
                .unwrap(),
            ),
        });
        let (offset, kind) = match target {
            TargetProfile::LinuxX64 => {
                image.memory.text[0] = 0xe8;
                (1, RelocationKind::X86_64Relative32)
            }
            TargetProfile::LinuxArm64 => {
                image.memory.text[0..4].copy_from_slice(&[0, 0, 0, 0x94]);
                (0, RelocationKind::Aarch64Branch26)
            }
            _ => unreachable!(),
        };
        image
            .relocation_table
            .relocations
            .insert(FinalImageRelocation {
                section: FinalImageSection::Text,
                offset,
                byte_width: 4,
                symbol_handle,
                addend: 0,
                kind,
            });
        let path = match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-x86-64.so.2".as_slice(),
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-aarch64.so.1".as_slice(),
            _ => unreachable!(),
        };
        let interpreter = normalize_elf_interpreter_plan(path.to_vec(), target).unwrap();
        let inputs = plan_elf_dynamic_link_inputs(image, interpreter).unwrap();
        let sections = plan_elf_dynamic_sections(inputs).unwrap();
        let payloads = serialize_elf_dynamic_sections(sections).unwrap();
        let descriptors = plan_elf_dynamic_section_descriptors(payloads).unwrap();
        let linkage = plan_elf_procedure_linkage_relocations(descriptors).unwrap();
        let templates = plan_elf_procedure_linkage_templates(linkage).unwrap();
        let descriptors = plan_elf_procedure_linkage_section_descriptors(templates).unwrap();
        let tags = plan_elf_dynamic_tags(descriptors).unwrap();
        let dynamic = serialize_elf_dynamic_table(tags).unwrap();
        let descriptor = plan_elf_dynamic_table_section_descriptor(dynamic).unwrap();
        let names = plan_elf_section_name_table(descriptor).unwrap();
        let roster = plan_elf_dynamic_section_roster(names).unwrap();
        let headers = serialize_elf_section_header_table(roster).unwrap();
        let payloads = plan_elf_indexed_section_payloads(headers).unwrap();
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
        let shstrtab = &layout.sections()[ElfPlacedDynamicSectionKind::SectionNameTable as usize];
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
        assert_eq!(layout.section_header_table_byte_size(), 13 * 64);
    }

    #[test]
    fn exact_twenty_three_fixups_are_resolved_without_mutating_template_bytes() {
        let layout = plan_elf_dynamic_load_layout(relative(TargetProfile::LinuxArm64)).unwrap();
        let template = layout.relative().payloads().section_headers().contents();
        assert_eq!(layout.section_header_resolutions().len(), 23);
        assert_eq!(template.placement_fixups.len(), 23);
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
            Box::new(|candidate| candidate.non_authoritative_layout_compatibility_fingerprint ^= 1),
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
        assert!(aarch64_page_delta_covers_extent(source, maximum_positive_page, 0x1000).unwrap());
        assert!(
            !aarch64_page_delta_covers_extent(source, maximum_positive_page, 0x1001).unwrap(),
            "the last byte crossing into the first out-of-range page must reject"
        );
        let maximum_negative_page = source - ((1_u64 << 20) * 0x1000);
        assert!(aarch64_page_delta_covers_extent(source, maximum_negative_page, 1).unwrap());
        assert!(
            !aarch64_page_delta_covers_extent(source, maximum_negative_page - 0x1000, 1).unwrap()
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
            constraint_kinds
                .contains(&ElfProcedureLinkagePlacementConstraintKind::Aarch64Load64Low12Aligned)
        );
        assert!(
            constraint_kinds.contains(&ElfProcedureLinkagePlacementConstraintKind::Aarch64Branch26)
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
        assert!(checked_sum(u64::MAX, 1, "sum").is_err());
        assert!(checked_product(u64::MAX, 2, "product").is_err());
        assert!(checked_align(u64::MAX, 8, "alignment").is_err());
        assert!(checked_align(7, 3, "alignment").is_err());
    }
}
