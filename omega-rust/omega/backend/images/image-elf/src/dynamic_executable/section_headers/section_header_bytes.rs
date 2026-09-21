//! Canonical ELF64-LSB section-header table templates.
//!
//! The primary System V ABI defines the exact 64-byte [`Elf64_Shdr`] field
//! order and the generic ELF [data encoding] requires least-significant-byte
//! first serialization. This layer copies the closed numeric roster into exact
//! bytes while retaining every address and file offset as a typed zero-valued
//! placement fixup.
//!
//! [`Elf64_Shdr`]: https://gabi.xinuos.com/elf/03-sheader.html#section-header
//! [data encoding]: https://gabi.xinuos.com/elf/02-eheader.html#data-encoding

use crate::bytes::{write_u32, write_u64};
use crate::dynamic_executable::checked::{checked_product, checked_sum, require};
use crate::dynamic_executable::section_headers::section_roster::{
    ElfDynamicRosterSectionKind, ElfNumericSectionDescriptor, ValidatedElfDynamicSectionRoster,
};
use diagnostics::Diagnostic;

const ELF64_SECTION_HEADER_SIZE: usize = 64;
const ELF64_SECTION_ADDRESS_OFFSET: usize = 16;
const ELF64_SECTION_FILE_OFFSET: usize = 24;
const ELF64_PLACEMENT_FIELD_SIZE: u8 = 8;
const SECTION_COUNT: usize = 14;
const FILE_OFFSET_FIXUP_COUNT: usize = 13;
const VIRTUAL_ADDRESS_FIXUP_COUNT: usize = 12;
const PLACEMENT_FIXUP_COUNT: usize = FILE_OFFSET_FIXUP_COUNT + VIRTUAL_ADDRESS_FIXUP_COUNT;
const SHF_ALLOC: u64 = 0x2;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Independently decoded and replayed ELF64-LSB section-header table
/// templates.
///
/// The exact numeric roster remains owned by this non-clone carrier. Every
/// `sh_addr` and `sh_offset` that needs placement remains zero and is named by
/// one typed fixup. These bytes grant no placement, `e_shoff`, program header,
/// image mutation, publication, or runnable-image authority.
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

    pub fn placement_fixup_count(&self) -> usize {
        self.contents.placement_fixups.len()
    }

    /// Compatibility fingerprint of the exact roster identity, ELF64-LSB
    /// bytes, and typed placement-fixup coordinates. This is not placed-header
    /// or final-image identity.
    pub const fn non_authoritative_template_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_template_compatibility_fingerprint
    }

    #[allow(dead_code)]
    pub(crate) const fn contents(&self) -> &ElfSectionHeaderTableTemplateContents {
        &self.contents
    }

    #[allow(dead_code)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedElfDynamicSectionRoster,
        ElfSectionHeaderTableTemplateContents,
    ) {
        (self.roster, self.contents)
    }
}

/// Rejected section-header serialization with exact numeric-roster custody.
#[derive(Debug)]
#[must_use = "ELF section-header serialization rejection retains the numeric roster"]
pub struct ElfSectionHeaderTableSerializationError {
    roster: ValidatedElfDynamicSectionRoster,
    diagnostic: Diagnostic,
}

impl ElfSectionHeaderTableSerializationError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfDynamicSectionRoster, Diagnostic) {
        (self.roster, self.diagnostic)
    }
}

impl std::fmt::Display for ElfSectionHeaderTableSerializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfSectionHeaderTableSerializationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfSectionHeaderTableTemplateContents {
    pub(crate) bytes: Vec<u8>,
    pub(crate) placement_fixups: Vec<ElfSectionHeaderPlacementFixup>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ElfSectionHeaderPlacementFixupKind {
    VirtualAddress = 1,
    FileOffset = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfSectionHeaderPlacementFixup {
    pub(crate) row_index: u32,
    pub(crate) section_kind: ElfDynamicRosterSectionKind,
    pub(crate) byte_offset: usize,
    pub(crate) byte_width: u8,
    pub(crate) kind: ElfSectionHeaderPlacementFixupKind,
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

struct Candidate {
    roster: ValidatedElfDynamicSectionRoster,
    contents: ElfSectionHeaderTableTemplateContents,
    non_authoritative_template_compatibility_fingerprint: u64,
}

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Serialize the exact numeric roster as thirteen 64-byte ELF64-LSB section
/// headers with zero placement placeholders and typed fixup coordinates.
///
/// This does not resolve a fixup, choose `sh_addr`, `sh_offset`, or `e_shoff`,
/// place the table or payloads, emit program headers, or mutate an image.
pub fn serialize_elf_section_header_table(
    roster: ValidatedElfDynamicSectionRoster,
) -> Result<ValidatedElfSectionHeaderTableTemplate, Box<ElfSectionHeaderTableSerializationError>> {
    let contents = match encode_contents(&roster) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfSectionHeaderTableSerializationError {
                roster,
                diagnostic,
            }));
        }
    };
    let non_authoritative_template_compatibility_fingerprint =
        non_authoritative_template_compatibility_fingerprint(&roster, &contents);
    let candidate = Candidate {
        roster,
        contents,
        non_authoritative_template_compatibility_fingerprint,
    };
    match validate_candidate(candidate) {
        Ok(validated) => Ok(validated),
        Err(error) => Err(Box::new(ElfSectionHeaderTableSerializationError {
            roster: error.candidate.roster,
            diagnostic: error.diagnostic,
        })),
    }
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

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfSectionHeaderTableTemplate, CandidateValidationError> {
    if let Err(diagnostic) = validate_contents(&candidate.roster, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    if candidate.non_authoritative_template_compatibility_fingerprint
        != non_authoritative_template_compatibility_fingerprint(
            &candidate.roster,
            &candidate.contents,
        )
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(
                "ELF section-header template compatibility fingerprint does not replay",
            ),
        });
    }
    Ok(ValidatedElfSectionHeaderTableTemplate {
        roster: candidate.roster,
        contents: candidate.contents,
        non_authoritative_template_compatibility_fingerprint: candidate
            .non_authoritative_template_compatibility_fingerprint,
    })
}

fn validate_contents(
    roster: &ValidatedElfDynamicSectionRoster,
    contents: &ElfSectionHeaderTableTemplateContents,
) -> Result<(), Diagnostic> {
    require(
        roster.section_count() == SECTION_COUNT,
        "ELF section-header serialization requires the exact thirteen-row roster",
    )?;
    let decoded = decode_rows(&contents.bytes, roster.section_count())?;
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

fn decode_rows(
    bytes: &[u8],
    expected_count: usize,
) -> Result<Vec<DecodedElf64SectionHeader>, Diagnostic> {
    let expected_size = checked_product(
        expected_count,
        ELF64_SECTION_HEADER_SIZE,
        "decoded ELF64 section-header table size",
    )?;
    require(
        bytes.len() == expected_size,
        "ELF64 section-header table has a truncated row or trailing bytes",
    )?;
    let mut rows = Vec::with_capacity(expected_count);
    for index in 0..expected_count {
        let offset = checked_product(
            index,
            ELF64_SECTION_HEADER_SIZE,
            "decoded ELF64 section-header row",
        )?;
        rows.push(DecodedElf64SectionHeader {
            name_offset: read_u32(bytes, offset, "Elf64_Shdr.sh_name")?,
            section_type: read_u32(
                bytes,
                checked_sum(offset, 4, "Elf64_Shdr.sh_type offset")?,
                "Elf64_Shdr.sh_type",
            )?,
            flags: read_u64(
                bytes,
                checked_sum(offset, 8, "Elf64_Shdr.sh_flags offset")?,
                "Elf64_Shdr.sh_flags",
            )?,
            address: read_u64(
                bytes,
                checked_sum(offset, 16, "Elf64_Shdr.sh_addr offset")?,
                "Elf64_Shdr.sh_addr",
            )?,
            file_offset: read_u64(
                bytes,
                checked_sum(offset, 24, "Elf64_Shdr.sh_offset offset")?,
                "Elf64_Shdr.sh_offset",
            )?,
            payload_size: read_u64(
                bytes,
                checked_sum(offset, 32, "Elf64_Shdr.sh_size offset")?,
                "Elf64_Shdr.sh_size",
            )?,
            link: read_u32(
                bytes,
                checked_sum(offset, 40, "Elf64_Shdr.sh_link offset")?,
                "Elf64_Shdr.sh_link",
            )?,
            info: read_u32(
                bytes,
                checked_sum(offset, 44, "Elf64_Shdr.sh_info offset")?,
                "Elf64_Shdr.sh_info",
            )?,
            alignment: read_u64(
                bytes,
                checked_sum(offset, 48, "Elf64_Shdr.sh_addralign offset")?,
                "Elf64_Shdr.sh_addralign",
            )?,
            entry_size: read_u64(
                bytes,
                checked_sum(offset, 56, "Elf64_Shdr.sh_entsize offset")?,
                "Elf64_Shdr.sh_entsize",
            )?,
        });
    }
    Ok(rows)
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
                ElfSectionHeaderPlacementFixupKind::VirtualAddress => ELF64_SECTION_ADDRESS_OFFSET,
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

fn read_u32(bytes: &[u8], offset: usize, context: &'static str) -> Result<u32, Diagnostic> {
    let end = checked_sum(offset, 4, context)?;
    let value = bytes
        .get(offset..end)
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))?;
    Ok(u32::from_le_bytes(value))
}

fn read_u64(bytes: &[u8], offset: usize, context: &'static str) -> Result<u64, Diagnostic> {
    let end = checked_sum(offset, 8, context)?;
    let value = bytes
        .get(offset..end)
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))?;
    Ok(u64::from_le_bytes(value))
}

fn non_authoritative_template_compatibility_fingerprint(
    roster: &ValidatedElfDynamicSectionRoster,
    contents: &ElfSectionHeaderTableTemplateContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-section-header-table-template.v1");
    hash.bytes(
        &roster
            .non_authoritative_roster_compatibility_fingerprint()
            .to_le_bytes(),
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
