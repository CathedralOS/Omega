//! Final numeric ELF dynamic-section roster without placement or headers.
//!
//! The generic System V ABI defines the null section, numeric `sh_name`,
//! `sh_link`, and `sh_info` fields in the [section header] and identifies the
//! section-name table through [`e_shstrndx`]. This layer closes only the exact
//! thirteen-row section order and resolves retained semantic references to
//! numeric indexes. It assigns no address, file offset, placement, program
//! header, or serialized section-header bytes.
//!
//! [section header]: https://gabi.xinuos.com/elf/03-sheader.html#section-header
//! [`e_shstrndx`]: https://gabi.xinuos.com/elf/02-eheader.html#elf-header

use crate::dynamic_linkage_descriptors::{
    ElfProcedureLinkageSectionInfo, ElfProcedureLinkageSectionKind, ElfProcedureLinkageSectionLink,
};
use crate::dynamic_section_descriptors::ElfDynamicSectionKind;
use crate::section_name_table::{ElfSectionNameTableSectionKind, ValidatedElfSectionNameTablePlan};
use diagnostics::Diagnostic;

const SECTION_COUNT: usize = 13;
const SECTION_NAME_TABLE_INDEX: u32 = 12;
const SHT_NULL: u32 = 0;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

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
    ElfDynamicRosterSectionKind::DynamicTable,
    ElfDynamicRosterSectionKind::SectionNameTable,
];

/// Independently replayed closed numeric roster for the current ELF dynamic
/// image inputs.
///
/// The exact section-name table and all prior payload/descriptor owners remain
/// retained by this non-clone carrier. Numeric rows grant no address, file
/// offset, section-header bytes, payload placement, fixup application, image
/// mutation, publication, or runnable-image authority.
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

    /// Compatibility fingerprint of the exact name-table owner and every
    /// numeric section-row field. This is a roster compatibility coordinate, not layout or final
    /// image identity.
    pub const fn non_authoritative_roster_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_roster_compatibility_fingerprint
    }

    pub(crate) const fn contents(&self) -> &ElfDynamicSectionRosterContents {
        &self.contents
    }

    #[allow(dead_code)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedElfSectionNameTablePlan,
        ElfDynamicSectionRosterContents,
    ) {
        (self.section_names, self.contents)
    }
}

/// Rejected numeric-roster planning with exact section-name-table custody.
#[derive(Debug)]
#[must_use = "ELF section-roster rejection retains the section-name table"]
pub struct ElfDynamicSectionRosterPlanningError {
    section_names: ValidatedElfSectionNameTablePlan,
    diagnostic: Diagnostic,
}

impl ElfDynamicSectionRosterPlanningError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfSectionNameTablePlan, Diagnostic) {
        (self.section_names, self.diagnostic)
    }
}

impl std::fmt::Display for ElfDynamicSectionRosterPlanningError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfDynamicSectionRosterPlanningError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfDynamicSectionRosterContents {
    pub(crate) rows: Vec<ElfNumericSectionDescriptor>,
    pub(crate) section_name_table_index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ElfDynamicRosterSectionKind {
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

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Consume the completed section-name-table carrier and close the current ELF
/// dynamic-section roster with exact numeric indexes and resolved `sh_link` /
/// `sh_info` fields.
///
/// This does not assign addresses or file offsets, serialize section/program
/// headers, place payloads, resolve address fixups, mutate an image, or mint a
/// runnable dynamic image.
pub fn plan_elf_dynamic_section_roster(
    section_names: ValidatedElfSectionNameTablePlan,
) -> Result<ValidatedElfDynamicSectionRoster, Box<ElfDynamicSectionRosterPlanningError>> {
    let contents = match derive_contents(&section_names) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfDynamicSectionRosterPlanningError {
                section_names,
                diagnostic,
            }));
        }
    };
    let non_authoritative_roster_compatibility_fingerprint =
        non_authoritative_roster_compatibility_fingerprint(&section_names, &contents);
    let candidate = Candidate {
        section_names,
        contents,
        non_authoritative_roster_compatibility_fingerprint,
    };
    match validate_candidate(candidate) {
        Ok(validated) => Ok(validated),
        Err(error) => Err(Box::new(ElfDynamicSectionRosterPlanningError {
            section_names: error.candidate.section_names,
            diagnostic: error.diagnostic,
        })),
    }
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
) -> &crate::dynamic_section_descriptors::ElfDynamicSectionDescriptorContents {
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
) -> &crate::dynamic_linkage_descriptors::ElfProcedureLinkageSectionDescriptorContents {
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
        ElfDynamicSectionKind::GnuSymbolVersion => ElfDynamicRosterSectionKind::GnuSymbolVersion,
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
        ElfProcedureLinkageSectionKind::ProcedureGot => ElfDynamicRosterSectionKind::ProcedureGot,
        ElfProcedureLinkageSectionKind::ProcedureRelocation => {
            ElfDynamicRosterSectionKind::ProcedureRelocation
        }
    }
}

const fn index_for_dynamic_kind(kind: ElfDynamicSectionKind) -> u32 {
    index_for_kind(roster_kind_from_dynamic(kind))
}

const fn index_for_kind(kind: ElfDynamicRosterSectionKind) -> u32 {
    kind as u32
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfDynamicSectionRoster, CandidateValidationError> {
    if let Err(diagnostic) = validate_contents(&candidate.section_names, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    if candidate.non_authoritative_roster_compatibility_fingerprint
        != non_authoritative_roster_compatibility_fingerprint(
            &candidate.section_names,
            &candidate.contents,
        )
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(
                "ELF dynamic section-roster compatibility fingerprint does not replay",
            ),
        });
    }
    Ok(ValidatedElfDynamicSectionRoster {
        section_names: candidate.section_names,
        contents: candidate.contents,
        non_authoritative_roster_compatibility_fingerprint: candidate
            .non_authoritative_roster_compatibility_fingerprint,
    })
}

fn validate_contents(
    section_names: &ValidatedElfSectionNameTablePlan,
    contents: &ElfDynamicSectionRosterContents,
) -> Result<(), Diagnostic> {
    require(
        section_names.descriptor_count() == SECTION_COUNT - 1,
        "numeric ELF roster requires the exact sealed twelve semantic descriptors",
    )?;
    require(
        section_names.byte_count() == 112,
        "numeric ELF roster requires the exact complete 112-byte name table",
    )?;
    require(
        contents.rows.len() == SECTION_COUNT,
        "numeric ELF roster must contain exactly thirteen rows",
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
    let offset = usize::try_from(row.name_offset)
        .map_err(|_| Diagnostic::error("numeric ELF sh_name exceeds usize"))?;
    let tail = section_names
        .get(offset..)
        .ok_or_else(|| Diagnostic::error("numeric ELF sh_name is outside .shstrtab"))?;
    let terminator = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| Diagnostic::error("numeric ELF section name is not NUL-terminated"))?;
    require(
        tail.get(..terminator) == Some(expected_name(row.kind)),
        "numeric ELF sh_name does not select its exact canonical name",
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
        | ElfDynamicRosterSectionKind::ProcedureRelocation => {
            let source = linkage_descriptor_contents(section_names)
                .descriptors
                .iter()
                .find(|source| roster_kind_from_linkage(source.kind) == row.kind)
                .ok_or_else(|| Diagnostic::error("linkage ELF descriptor owner row is missing"))?;
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
                index: 11,
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
            && row(ElfDynamicRosterSectionKind::DynamicTable)?.link == 2,
        "numeric ELF link/info relationships do not resolve to the canonical roster",
    )
}

fn checked_u32(value: usize, context: &'static str) -> Result<u32, Diagnostic> {
    u32::try_from(value).map_err(|_| Diagnostic::error(format!("{context} exceeds Elf64_Word")))
}

fn require(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| Diagnostic::error(message))
}

fn non_authoritative_roster_compatibility_fingerprint(
    section_names: &ValidatedElfSectionNameTablePlan,
    contents: &ElfDynamicSectionRosterContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-dynamic-section-roster.v2");
    hash.bytes(
        &section_names
            .non_authoritative_table_compatibility_fingerprint()
            .to_le_bytes(),
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
        plan_elf_dynamic_sections, plan_elf_dynamic_table_section_descriptor,
        plan_elf_dynamic_tags, plan_elf_procedure_linkage_relocations,
        plan_elf_procedure_linkage_section_descriptors, plan_elf_procedure_linkage_templates,
        plan_elf_section_name_table, serialize_elf_dynamic_sections, serialize_elf_dynamic_table,
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

    #[derive(Clone, Copy)]
    struct ImportFixture {
        object: &'static [u8],
        symbol: &'static [u8],
        version: &'static [u8],
        instruction_offsets: &'static [usize],
    }

    const FIRST_SITES: &[usize] = &[0, 32];
    const SECOND_SITES: &[usize] = &[16];
    const IMPORTS: [ImportFixture; 2] = [
        ImportFixture {
            object: b"liba\xff.so",
            symbol: b"alpha\xfe",
            version: b"V1\xfd",
            instruction_offsets: FIRST_SITES,
        },
        ImportFixture {
            object: b"libb.so",
            symbol: b"beta",
            version: b"V2",
            instruction_offsets: SECOND_SITES,
        },
    ];

    fn interpreter_path(target: TargetProfile) -> &'static [u8] {
        match target {
            TargetProfile::LinuxX64 => b"/lib64/ld-linux-\xfc-x86-64.so.2",
            TargetProfile::LinuxArm64 => b"/lib/ld-linux-\xfb-aarch64.so.1",
            _ => unreachable!("section-roster fixture uses a Linux target"),
        }
    }

    fn section_names(
        target: TargetProfile,
        imports: &[ImportFixture],
    ) -> ValidatedElfSectionNameTablePlan {
        let relocation_count = imports
            .iter()
            .map(|fixture| fixture.instruction_offsets.len())
            .sum();
        let mut image = FinalImage::with_capacity(
            target.native_target(),
            FinalImageMemory {
                text: vec![0; 64],
                ..FinalImageMemory::default()
            },
            Handle::invalid(),
            imports.len(),
            imports.len(),
            relocation_count,
        );
        for (index, fixture) in imports.iter().enumerate() {
            let symbol_handle = image.symbol_table.symbols.insert(FinalImageSymbol {
                name: format!("__omega_section_roster_import_{index}"),
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
                            object: fixture.object.to_vec(),
                            symbol: fixture.symbol.to_vec(),
                            version: fixture.version.to_vec(),
                        },
                        target,
                    )
                    .expect("valid section-roster locator"),
                ),
            });
            for instruction_offset in fixture.instruction_offsets {
                let (relocation_offset, kind) = match target {
                    TargetProfile::LinuxX64 => {
                        image.memory.text[*instruction_offset] = 0xe8;
                        (instruction_offset + 1, RelocationKind::X86_64Relative32)
                    }
                    TargetProfile::LinuxArm64 => {
                        image.memory.text[*instruction_offset..*instruction_offset + 4]
                            .copy_from_slice(&[0, 0, 0, 0x94]);
                        (*instruction_offset, RelocationKind::Aarch64Branch26)
                    }
                    _ => unreachable!(),
                };
                image
                    .relocation_table
                    .relocations
                    .insert(FinalImageRelocation {
                        section: FinalImageSection::Text,
                        offset: relocation_offset,
                        byte_width: 4,
                        symbol_handle,
                        addend: 0,
                        kind,
                    });
            }
        }
        let interpreter = normalize_elf_interpreter_plan(interpreter_path(target).to_vec(), target)
            .expect("valid section-roster interpreter");
        let inputs = plan_elf_dynamic_link_inputs(image, interpreter).expect("valid link inputs");
        let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
        let payloads = serialize_elf_dynamic_sections(sections).expect("valid payloads");
        let descriptors =
            plan_elf_dynamic_section_descriptors(payloads).expect("valid base descriptors");
        let linkage =
            plan_elf_procedure_linkage_relocations(descriptors).expect("valid linkage plan");
        let templates =
            plan_elf_procedure_linkage_templates(linkage).expect("valid linkage templates");
        let descriptors = plan_elf_procedure_linkage_section_descriptors(templates)
            .expect("valid linkage descriptors");
        let tags = plan_elf_dynamic_tags(descriptors).expect("valid dynamic tags");
        let payload = serialize_elf_dynamic_table(tags).expect("valid dynamic payload");
        let descriptor =
            plan_elf_dynamic_table_section_descriptor(payload).expect("valid dynamic descriptor");
        plan_elf_section_name_table(descriptor).expect("valid section-name table")
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

    fn row(
        contents: &ElfDynamicSectionRosterContents,
        kind: ElfDynamicRosterSectionKind,
    ) -> &ElfNumericSectionDescriptor {
        contents
            .rows
            .get(index_for_kind(kind) as usize)
            .expect("numeric roster row")
    }

    #[test]
    fn both_targets_close_exact_thirteen_row_roster_and_shstrndx() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let roster = plan_elf_dynamic_section_roster(section_names(target, &IMPORTS))
                .expect("validated section roster");
            assert_eq!(roster.section_count(), 13);
            assert_eq!(roster.section_name_table_index(), 12);
            assert_eq!(roster.contents.rows.len(), 13);
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
    fn exact_numeric_links_and_literal_info_are_preserved() {
        let roster =
            plan_elf_dynamic_section_roster(section_names(TargetProfile::LinuxX64, &IMPORTS))
                .unwrap();
        assert_eq!(
            *row(&roster.contents, ElfDynamicRosterSectionKind::DynamicSymbol),
            ElfNumericSectionDescriptor {
                kind: ElfDynamicRosterSectionKind::DynamicSymbol,
                index: 3,
                name_offset: 17,
                section_type: 11,
                flags: 2,
                payload_size: 72,
                alignment: 8,
                entry_size: 24,
                link: 2,
                info: 1,
            },
        );
        let verneed = row(
            &roster.contents,
            ElfDynamicRosterSectionKind::GnuVersionRequirement,
        );
        assert_eq!((verneed.link, verneed.info), (2, 2));
        let rela = row(
            &roster.contents,
            ElfDynamicRosterSectionKind::ProcedureRelocation,
        );
        assert_eq!((rela.link, rela.info), (3, 9));
        let dynamic = row(&roster.contents, ElfDynamicRosterSectionKind::DynamicTable);
        assert_eq!(
            (dynamic.name_offset, dynamic.link, dynamic.info),
            (103, 2, 0)
        );
        let shstrtab = row(
            &roster.contents,
            ElfDynamicRosterSectionKind::SectionNameTable,
        );
        assert_eq!(
            (
                shstrtab.name_offset,
                shstrtab.section_type,
                shstrtab.payload_size,
                shstrtab.link,
                shstrtab.info,
            ),
            (59, 3, 112, 0, 0),
        );
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
            Box::new(|candidate| candidate.non_authoritative_roster_compatibility_fingerprint ^= 1),
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
