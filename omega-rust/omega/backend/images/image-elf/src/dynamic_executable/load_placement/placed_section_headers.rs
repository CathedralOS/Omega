//! Applied ELF64-LSB section-header placement fields.
//!
//! This layer consumes the independently validated absolute dynamic-load
//! geometry and copies its retained thirteen-row `Elf64_Shdr` template. It
//! applies only the twenty-three already-resolved `sh_addr` and `sh_offset`
//! fields, then decodes the complete table and replays every field against the
//! retained numeric roster and placement ledger.
//!
//! It does not choose or serialize `e_shoff`, an ELF header, or program
//! headers; resolve payload-internal dynamic/procedure/source relocations;
//! mutate the retained `FinalImage`; or claim a runnable image.

use crate::dynamic_executable::checked::{checked_product, checked_sum, read_u64, require};
use crate::dynamic_executable::load_placement::load_layout::{
    ElfPlacedDynamicSectionKind, ElfSectionPlacementResolutionKind, ValidatedElfDynamicLoadLayout,
};
use crate::dynamic_executable::section_headers::section_header_bytes::ElfSectionHeaderPlacementFixupKind;
use crate::dynamic_executable::section_headers::section_roster::{
    ElfDynamicRosterSectionKind, ElfNumericSectionDescriptor,
};
use diagnostics::Diagnostic;

const ELF64_SECTION_HEADER_SIZE: usize = 64;
const ELF64_SECTION_ADDRESS_OFFSET: usize = 16;
const ELF64_SECTION_FILE_OFFSET: usize = 24;
const ELF64_PLACEMENT_FIELD_SIZE: u8 = 8;
const SECTION_COUNT: usize = 13;
const FILE_OFFSET_PLACEMENT_COUNT: usize = 12;
const VIRTUAL_ADDRESS_PLACEMENT_COUNT: usize = 11;
const PLACEMENT_COUNT: usize = 23;
const SECTION_NAME_TABLE_INDEX: usize = 12;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// One placement application retained beside the resulting bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElfAppliedSectionHeaderPlacement {
    row_index: u32,
    section_kind: ElfPlacedDynamicSectionKind,
    byte_offset: usize,
    byte_width: u8,
    kind: ElfSectionPlacementResolutionKind,
    value: u64,
}

impl ElfAppliedSectionHeaderPlacement {
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

/// Independently decoded section-header bytes retaining exact load-layout
/// custody.
///
/// This non-clone carrier proves only that the retained 832-byte template has
/// received its exact twenty-three load-layout placements. It carries no final
/// file placement, mutation, publication, or runnable-image authority.
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

    pub fn applied_placements(&self) -> &[ElfAppliedSectionHeaderPlacement] {
        &self.contents.applications
    }

    pub const fn non_authoritative_placed_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_placed_compatibility_fingerprint
    }

    #[allow(dead_code)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedElfDynamicLoadLayout,
        ElfPlacedSectionHeaderContents,
    ) {
        (self.load_layout, self.contents)
    }
}

/// Rejected application retaining exact absolute-load-layout custody.
#[derive(Debug)]
#[must_use = "section-header placement rejection retains the absolute load layout"]
pub struct ElfSectionHeaderPlacementApplicationError {
    load_layout: ValidatedElfDynamicLoadLayout,
    diagnostic: Diagnostic,
}

impl ElfSectionHeaderPlacementApplicationError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfDynamicLoadLayout, Diagnostic) {
        (self.load_layout, self.diagnostic)
    }
}

impl std::fmt::Display for ElfSectionHeaderPlacementApplicationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfSectionHeaderPlacementApplicationError {}

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

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DecodedElf64SectionHeader {
    name_offset: u32,
    section_type: u32,
    flags: u64,
    address: u64,
    file_offset: u64,
    payload_size: u64,
    link: u32,
    info: u32,
    alignment: u64,
    entry_size: u64,
}

/// Copy the retained section-header template and apply its exact resolved
/// address and file-offset values in little-endian form.
pub fn apply_elf_section_header_placements(
    load_layout: ValidatedElfDynamicLoadLayout,
) -> Result<ValidatedElfPlacedSectionHeaderTable, Box<ElfSectionHeaderPlacementApplicationError>> {
    let contents = match derive_contents(&load_layout) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfSectionHeaderPlacementApplicationError {
                load_layout,
                diagnostic,
            }));
        }
    };
    let non_authoritative_placed_compatibility_fingerprint =
        non_authoritative_placed_compatibility_fingerprint(&load_layout, &contents);
    let candidate = Candidate {
        load_layout,
        contents,
        non_authoritative_placed_compatibility_fingerprint,
    };
    validate_candidate(candidate).map_err(|error| {
        Box::new(ElfSectionHeaderPlacementApplicationError {
            load_layout: error.candidate.load_layout,
            diagnostic: error.diagnostic,
        })
    })
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
        "placed ELF section-header input does not have exactly twenty-three fixups and resolutions",
    )?;

    let mut bytes = template.bytes.clone();
    let mut applications = Vec::with_capacity(PLACEMENT_COUNT);
    for (fixup, resolution) in template
        .placement_fixups
        .iter()
        .zip(load_layout.section_header_resolutions())
    {
        let expected_kind = match fixup.kind {
            ElfSectionHeaderPlacementFixupKind::VirtualAddress => {
                ElfSectionPlacementResolutionKind::VirtualAddress
            }
            ElfSectionHeaderPlacementFixupKind::FileOffset => {
                ElfSectionPlacementResolutionKind::FileOffset
            }
        };
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
        let field = field_mut(&mut bytes, fixup.byte_offset, fixup.byte_width)?;
        require(
            field.iter().all(|byte| *byte == 0),
            "ELF section-header placement input is not an exact zero placeholder",
        )?;
        field.copy_from_slice(&resolution.value().to_le_bytes());
        applications.push(ElfAppliedSectionHeaderPlacement {
            row_index: resolution.row_index(),
            section_kind: resolution.section_kind(),
            byte_offset: resolution.byte_offset(),
            byte_width: resolution.byte_width(),
            kind: resolution.kind(),
            value: resolution.value(),
        });
    }
    Ok(ElfPlacedSectionHeaderContents {
        bytes,
        applications,
    })
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfPlacedSectionHeaderTable, CandidateValidationError> {
    if let Err(diagnostic) = validate_contents(&candidate.load_layout, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    let expected_identity = non_authoritative_placed_compatibility_fingerprint(
        &candidate.load_layout,
        &candidate.contents,
    );
    if candidate.non_authoritative_placed_compatibility_fingerprint == 0
        || candidate.non_authoritative_placed_compatibility_fingerprint != expected_identity
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(
                "placed ELF section-header compatibility fingerprint does not replay",
            ),
        });
    }
    Ok(ValidatedElfPlacedSectionHeaderTable {
        load_layout: candidate.load_layout,
        contents: candidate.contents,
        non_authoritative_placed_compatibility_fingerprint: candidate
            .non_authoritative_placed_compatibility_fingerprint,
    })
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
        "placed ELF section-header table does not retain exactly thirteen complete rows",
    )?;
    require(
        template.placement_fixups.len() == PLACEMENT_COUNT
            && resolutions.len() == PLACEMENT_COUNT
            && contents.applications.len() == PLACEMENT_COUNT,
        "placed ELF section-header ledger does not cover exactly twenty-three fields",
    )?;

    for ((fixup, resolution), application) in template
        .placement_fixups
        .iter()
        .zip(resolutions)
        .zip(&contents.applications)
    {
        let expected_kind = match fixup.kind {
            ElfSectionHeaderPlacementFixupKind::VirtualAddress => {
                ElfSectionPlacementResolutionKind::VirtualAddress
            }
            ElfSectionHeaderPlacementFixupKind::FileOffset => {
                ElfSectionPlacementResolutionKind::FileOffset
            }
        };
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
            .filter(|application| application.kind == ElfSectionPlacementResolutionKind::FileOffset)
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

    let decoded = decode_rows(&contents.bytes)?;
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

fn decode_rows(bytes: &[u8]) -> Result<Vec<DecodedElf64SectionHeader>, Diagnostic> {
    require(
        bytes.len() == SECTION_COUNT * ELF64_SECTION_HEADER_SIZE,
        "placed ELF section-header table has a truncated row or trailing bytes",
    )?;
    let mut rows = Vec::with_capacity(SECTION_COUNT);
    for index in 0..SECTION_COUNT {
        let offset = checked_product(index, ELF64_SECTION_HEADER_SIZE, "decoded section row")?;
        rows.push(DecodedElf64SectionHeader {
            name_offset: read_u32(bytes, offset, "Elf64_Shdr.sh_name")?,
            section_type: read_u32(bytes, offset + 4, "Elf64_Shdr.sh_type")?,
            flags: read_u64(bytes, offset + 8, "Elf64_Shdr.sh_flags")?,
            address: read_u64(bytes, offset + 16, "Elf64_Shdr.sh_addr")?,
            file_offset: read_u64(bytes, offset + 24, "Elf64_Shdr.sh_offset")?,
            payload_size: read_u64(bytes, offset + 32, "Elf64_Shdr.sh_size")?,
            link: read_u32(bytes, offset + 40, "Elf64_Shdr.sh_link")?,
            info: read_u32(bytes, offset + 44, "Elf64_Shdr.sh_info")?,
            alignment: read_u64(bytes, offset + 48, "Elf64_Shdr.sh_addralign")?,
            entry_size: read_u64(bytes, offset + 56, "Elf64_Shdr.sh_entsize")?,
        });
    }
    Ok(rows)
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

fn field_mut(bytes: &mut [u8], offset: usize, width: u8) -> Result<&mut [u8], Diagnostic> {
    let end = checked_sum(offset, usize::from(width), "section-header placement end")?;
    bytes
        .get_mut(offset..end)
        .ok_or_else(|| Diagnostic::error("section-header placement exceeds the template"))
}

fn read_u32(bytes: &[u8], offset: usize, context: &'static str) -> Result<u32, Diagnostic> {
    let end = checked_sum(offset, 4, context)?;
    let value = bytes
        .get(offset..end)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))?;
    Ok(u32::from_le_bytes(value))
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

fn non_authoritative_placed_compatibility_fingerprint(
    load_layout: &ValidatedElfDynamicLoadLayout,
    contents: &ElfPlacedSectionHeaderContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf.placed-section-header-table.v1");
    hash.bytes(
        &load_layout
            .non_authoritative_layout_compatibility_fingerprint()
            .to_le_bytes(),
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
mod tests;
