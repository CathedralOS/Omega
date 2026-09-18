//! Address-free ELF section descriptors for validated dynamic payloads.
//!
//! These descriptors follow the System V ABI [section header], [section type],
//! [section flag], and [`sh_link`/`sh_info`] rules plus the LSB [GNU section
//! types]. Links remain semantic section kinds rather than premature final
//! section indexes. The [original GNU implementation] defines the GNU-hash
//! section type and its dynamic-tag relationship.
//!
//! [section header]: https://gabi.xinuos.com/elf/03-sheader.html#section-header
//! [section type]: https://gabi.xinuos.com/elf/03-sheader.html#section-types
//! [section flag]: https://gabi.xinuos.com/elf/03-sheader.html#section-attributes
//! [`sh_link`/`sh_info`]: https://gabi.xinuos.com/elf/03-sheader.html#the-sh-link-and-sh-info-fields
//! [GNU section types]: https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/sections.html
//! [original GNU implementation]: https://sourceware.org/pipermail/binutils/2006-July/048074.html

use crate::dynamic_executable::checked::require;
use crate::dynamic_executable::import_sections::dynamic_section_bytes::ValidatedElfDynamicSectionPayloads;
use diagnostics::Diagnostic;

const SHT_PROGBITS: u32 = 1;
const SHT_STRTAB: u32 = 3;
const SHT_HASH: u32 = 5;
const SHT_DYNSYM: u32 = 11;
const SHT_GNU_HASH: u32 = 0x6fff_fff6;
const SHT_GNU_VERNEED: u32 = 0x6fff_fffe;
const SHT_GNU_VERSYM: u32 = 0x6fff_ffff;
const SHF_ALLOC: u64 = 0x2;

const INTERPRETER_NAME_OFFSET: u32 = 1;
const DYNAMIC_STRING_NAME_OFFSET: u32 = 9;
const DYNAMIC_SYMBOL_NAME_OFFSET: u32 = 17;
const SYSTEM_V_HASH_NAME_OFFSET: u32 = 25;
const GNU_SYMBOL_VERSION_NAME_OFFSET: u32 = 31;
const GNU_VERSION_REQUIREMENT_NAME_OFFSET: u32 = 44;
const SECTION_NAME_TABLE_NAME_OFFSET: u32 = 59;
const GNU_HASH_NAME_OFFSET: u32 = 69;

const SECTION_NAME_TABLE_SEED: &[u8] =
    b"\0.interp\0.dynstr\0.dynsym\0.hash\0.gnu.version\0.gnu.version_r\0.shstrtab\0.gnu.hash\0";
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

const DYNAMIC_SECTION_KINDS: [ElfDynamicSectionKind; 7] = [
    ElfDynamicSectionKind::Interpreter,
    ElfDynamicSectionKind::DynamicString,
    ElfDynamicSectionKind::DynamicSymbol,
    ElfDynamicSectionKind::SystemVHash,
    ElfDynamicSectionKind::GnuSymbolVersion,
    ElfDynamicSectionKind::GnuVersionRequirement,
    ElfDynamicSectionKind::GnuHash,
];

/// Independently validated address-free metadata for the seven serialized
/// dynamic payloads.
///
/// The append-only name seed stabilizes current `sh_name` offsets but is not a
/// completed `.shstrtab`; no descriptor for that future final table is
/// fabricated. This non-clone plan grants no section index, placement,
/// program-header, image-mutation, publication, or runnable-image authority.
#[derive(Debug)]
#[must_use = "validated ELF section descriptors retain the exact payload carrier"]
pub struct ValidatedElfDynamicSectionDescriptorPlan {
    payloads: ValidatedElfDynamicSectionPayloads,
    contents: ElfDynamicSectionDescriptorContents,
    non_authoritative_descriptor_compatibility_fingerprint: u64,
}

impl ValidatedElfDynamicSectionDescriptorPlan {
    pub const fn payloads(&self) -> &ValidatedElfDynamicSectionPayloads {
        &self.payloads
    }

    pub fn descriptor_count(&self) -> usize {
        self.contents.descriptors.len()
    }

    pub fn section_name_seed_byte_count(&self) -> usize {
        self.contents.section_name_table_seed.len()
    }

    pub(crate) fn section_name_table_seed(&self) -> &[u8] {
        &self.contents.section_name_table_seed
    }

    /// Compatibility fingerprint of the exact payload identity, append-only
    /// name seed, semantic descriptor kinds/links, and every ABI metadata
    /// field. This is a content compatibility coordinate, not layout or image authority.
    pub const fn non_authoritative_descriptor_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_descriptor_compatibility_fingerprint
    }

    #[allow(dead_code)]
    pub(crate) const fn contents(&self) -> &ElfDynamicSectionDescriptorContents {
        &self.contents
    }

    #[allow(dead_code)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedElfDynamicSectionPayloads,
        ElfDynamicSectionDescriptorContents,
    ) {
        (self.payloads, self.contents)
    }
}

/// Rejected dynamic-section descriptor planning with exact payload custody.
#[derive(Debug)]
#[must_use = "ELF descriptor rejection retains the validated payload carrier"]
pub struct ElfDynamicSectionDescriptorPlanningError {
    payloads: ValidatedElfDynamicSectionPayloads,
    diagnostic: Diagnostic,
}

impl ElfDynamicSectionDescriptorPlanningError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfDynamicSectionPayloads, Diagnostic) {
        (self.payloads, self.diagnostic)
    }
}

impl std::fmt::Display for ElfDynamicSectionDescriptorPlanningError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfDynamicSectionDescriptorPlanningError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfDynamicSectionDescriptorContents {
    pub(crate) section_name_table_seed: Vec<u8>,
    pub(crate) descriptors: Vec<ElfAddressFreeSectionDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ElfDynamicSectionKind {
    Interpreter = 1,
    DynamicString = 2,
    DynamicSymbol = 3,
    SystemVHash = 4,
    GnuSymbolVersion = 5,
    GnuVersionRequirement = 6,
    GnuHash = 7,
}

impl ElfDynamicSectionKind {
    const fn name(self) -> &'static [u8] {
        match self {
            Self::Interpreter => b".interp",
            Self::DynamicString => b".dynstr",
            Self::DynamicSymbol => b".dynsym",
            Self::SystemVHash => b".hash",
            Self::GnuSymbolVersion => b".gnu.version",
            Self::GnuVersionRequirement => b".gnu.version_r",
            Self::GnuHash => b".gnu.hash",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfAddressFreeSectionDescriptor {
    pub(crate) kind: ElfDynamicSectionKind,
    pub(crate) name_offset: u32,
    pub(crate) section_type: u32,
    pub(crate) flags: u64,
    pub(crate) payload_size: u64,
    pub(crate) alignment: u64,
    pub(crate) entry_size: u64,
    pub(crate) link: Option<ElfDynamicSectionKind>,
    pub(crate) info: u32,
}

struct Candidate {
    payloads: ValidatedElfDynamicSectionPayloads,
    contents: ElfDynamicSectionDescriptorContents,
    non_authoritative_descriptor_compatibility_fingerprint: u64,
}

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Consume exact serialized payloads into canonical address-free ELF section
/// descriptors and independently replay all names, metadata, semantic links,
/// and cross-payload counts before sealing success.
///
/// This deliberately does not choose final numeric section indexes, complete
/// `.shstrtab`, serialize section headers, place bytes, or plan `.dynamic`,
/// GOT/PLT, relocation, or program-header contents.
pub fn plan_elf_dynamic_section_descriptors(
    payloads: ValidatedElfDynamicSectionPayloads,
) -> Result<ValidatedElfDynamicSectionDescriptorPlan, Box<ElfDynamicSectionDescriptorPlanningError>>
{
    let contents = match derive_contents(&payloads) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfDynamicSectionDescriptorPlanningError {
                payloads,
                diagnostic,
            }));
        }
    };
    let non_authoritative_descriptor_compatibility_fingerprint =
        non_authoritative_descriptor_compatibility_fingerprint(&payloads, &contents);
    let candidate = Candidate {
        payloads,
        contents,
        non_authoritative_descriptor_compatibility_fingerprint,
    };
    match validate_candidate(candidate) {
        Ok(validated) => Ok(validated),
        Err(error) => Err(Box::new(ElfDynamicSectionDescriptorPlanningError {
            payloads: error.candidate.payloads,
            diagnostic: error.diagnostic,
        })),
    }
}

fn derive_contents(
    payloads: &ValidatedElfDynamicSectionPayloads,
) -> Result<ElfDynamicSectionDescriptorContents, Diagnostic> {
    let bytes = payloads.payloads();
    let needed_object_count = u32::try_from(payloads.plan().needed_object_count())
        .map_err(|_| Diagnostic::error("ELF version-need section info exceeds Elf64_Word"))?;
    let descriptors = vec![
        descriptor(
            ElfDynamicSectionKind::Interpreter,
            INTERPRETER_NAME_OFFSET,
            SHT_PROGBITS,
            SHF_ALLOC,
            bytes.interpreter.len(),
            1,
            0,
            None,
            0,
        )?,
        descriptor(
            ElfDynamicSectionKind::DynamicString,
            DYNAMIC_STRING_NAME_OFFSET,
            SHT_STRTAB,
            SHF_ALLOC,
            bytes.dynstr.len(),
            1,
            0,
            None,
            0,
        )?,
        descriptor(
            ElfDynamicSectionKind::DynamicSymbol,
            DYNAMIC_SYMBOL_NAME_OFFSET,
            SHT_DYNSYM,
            SHF_ALLOC,
            bytes.dynsym.len(),
            8,
            24,
            Some(ElfDynamicSectionKind::DynamicString),
            1,
        )?,
        descriptor(
            ElfDynamicSectionKind::SystemVHash,
            SYSTEM_V_HASH_NAME_OFFSET,
            SHT_HASH,
            SHF_ALLOC,
            bytes.sysv_hash.len(),
            4,
            4,
            Some(ElfDynamicSectionKind::DynamicSymbol),
            0,
        )?,
        descriptor(
            ElfDynamicSectionKind::GnuSymbolVersion,
            GNU_SYMBOL_VERSION_NAME_OFFSET,
            SHT_GNU_VERSYM,
            SHF_ALLOC,
            bytes.versym.len(),
            2,
            2,
            Some(ElfDynamicSectionKind::DynamicSymbol),
            0,
        )?,
        descriptor(
            ElfDynamicSectionKind::GnuVersionRequirement,
            GNU_VERSION_REQUIREMENT_NAME_OFFSET,
            SHT_GNU_VERNEED,
            SHF_ALLOC,
            bytes.verneed.len(),
            4,
            0,
            Some(ElfDynamicSectionKind::DynamicString),
            needed_object_count,
        )?,
        descriptor(
            ElfDynamicSectionKind::GnuHash,
            GNU_HASH_NAME_OFFSET,
            SHT_GNU_HASH,
            SHF_ALLOC,
            bytes.gnu_hash.len(),
            8,
            0,
            Some(ElfDynamicSectionKind::DynamicSymbol),
            0,
        )?,
    ];
    Ok(ElfDynamicSectionDescriptorContents {
        section_name_table_seed: SECTION_NAME_TABLE_SEED.to_vec(),
        descriptors,
    })
}

#[allow(clippy::too_many_arguments)]
fn descriptor(
    kind: ElfDynamicSectionKind,
    name_offset: u32,
    section_type: u32,
    flags: u64,
    payload_size: usize,
    alignment: u64,
    entry_size: u64,
    link: Option<ElfDynamicSectionKind>,
    info: u32,
) -> Result<ElfAddressFreeSectionDescriptor, Diagnostic> {
    Ok(ElfAddressFreeSectionDescriptor {
        kind,
        name_offset,
        section_type,
        flags,
        payload_size: u64::try_from(payload_size)
            .map_err(|_| Diagnostic::error("ELF section payload size exceeds Elf64_Xword"))?,
        alignment,
        entry_size,
        link,
        info,
    })
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfDynamicSectionDescriptorPlan, CandidateValidationError> {
    if let Err(diagnostic) = validate_contents(&candidate.payloads, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    if candidate.non_authoritative_descriptor_compatibility_fingerprint
        != non_authoritative_descriptor_compatibility_fingerprint(
            &candidate.payloads,
            &candidate.contents,
        )
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(
                "ELF dynamic descriptor compatibility fingerprint does not replay",
            ),
        });
    }
    Ok(ValidatedElfDynamicSectionDescriptorPlan {
        payloads: candidate.payloads,
        contents: candidate.contents,
        non_authoritative_descriptor_compatibility_fingerprint: candidate
            .non_authoritative_descriptor_compatibility_fingerprint,
    })
}

fn validate_contents(
    payloads: &ValidatedElfDynamicSectionPayloads,
    contents: &ElfDynamicSectionDescriptorContents,
) -> Result<(), Diagnostic> {
    require(
        contents.section_name_table_seed == SECTION_NAME_TABLE_SEED
            && contents.section_name_table_seed.first() == Some(&0)
            && contents.section_name_table_seed.last() == Some(&0),
        "ELF section-name seed is not the exact append-only canonical prefix",
    )?;
    require(
        name_at(
            &contents.section_name_table_seed,
            SECTION_NAME_TABLE_NAME_OFFSET,
        )? == b".shstrtab"
            && contents
                .descriptors
                .iter()
                .all(|descriptor| descriptor.name_offset != SECTION_NAME_TABLE_NAME_OFFSET),
        "ELF section-name seed does not retain an unclaimed future .shstrtab name",
    )?;
    require(
        contents.descriptors.len() == DYNAMIC_SECTION_KINDS.len(),
        "ELF dynamic descriptor plan does not contain exactly seven rows",
    )?;

    for (index, expected_kind) in DYNAMIC_SECTION_KINDS.iter().enumerate() {
        let row = contents.descriptors.get(index).ok_or_else(|| {
            Diagnostic::error("ELF dynamic descriptor row is missing from canonical order")
        })?;
        require(
            row.kind == *expected_kind
                && contents
                    .descriptors
                    .iter()
                    .filter(|candidate| candidate.kind == *expected_kind)
                    .count()
                    == 1,
            "ELF dynamic descriptor kinds are missing, duplicated, or reordered",
        )?;
        validate_name(&contents.section_name_table_seed, row)?;
        require(
            row.alignment.is_power_of_two(),
            "ELF dynamic descriptor alignment is not a positive power of two",
        )?;
        if row.entry_size != 0 {
            require(
                row.payload_size % row.entry_size == 0,
                "ELF dynamic descriptor payload is not divisible by its entry size",
            )?;
        }
        if let Some(link) = row.link {
            require(
                contents
                    .descriptors
                    .iter()
                    .filter(|candidate| candidate.kind == link)
                    .count()
                    == 1,
                "ELF dynamic descriptor semantic link does not resolve exactly once",
            )?;
        }
        validate_row(payloads, row)?;
    }
    validate_cross_payloads(payloads, contents)
}

fn validate_name(
    section_names: &[u8],
    row: &ElfAddressFreeSectionDescriptor,
) -> Result<(), Diagnostic> {
    require(
        name_at(section_names, row.name_offset)? == row.kind.name(),
        "ELF descriptor name offset does not identify its semantic kind",
    )
}

fn name_at(section_names: &[u8], offset: u32) -> Result<&[u8], Diagnostic> {
    let suffix = section_names
        .get(offset as usize..)
        .ok_or_else(|| Diagnostic::error("ELF descriptor name offset exceeds the name seed"))?;
    let end = suffix.iter().position(|byte| *byte == 0).ok_or_else(|| {
        Diagnostic::error("ELF descriptor name is not terminated in the name seed")
    })?;
    Ok(&suffix[..end])
}

fn validate_row(
    payloads: &ValidatedElfDynamicSectionPayloads,
    row: &ElfAddressFreeSectionDescriptor,
) -> Result<(), Diagnostic> {
    let bytes = payloads.payloads();
    let needed_object_count = u32::try_from(payloads.plan().needed_object_count())
        .map_err(|_| Diagnostic::error("validated needed-object count exceeds Elf64_Word"))?;
    let expected = match row.kind {
        ElfDynamicSectionKind::Interpreter => (
            INTERPRETER_NAME_OFFSET,
            SHT_PROGBITS,
            bytes.interpreter.len(),
            1,
            0,
            None,
            0,
        ),
        ElfDynamicSectionKind::DynamicString => (
            DYNAMIC_STRING_NAME_OFFSET,
            SHT_STRTAB,
            bytes.dynstr.len(),
            1,
            0,
            None,
            0,
        ),
        ElfDynamicSectionKind::DynamicSymbol => (
            DYNAMIC_SYMBOL_NAME_OFFSET,
            SHT_DYNSYM,
            bytes.dynsym.len(),
            8,
            24,
            Some(ElfDynamicSectionKind::DynamicString),
            1,
        ),
        ElfDynamicSectionKind::SystemVHash => (
            SYSTEM_V_HASH_NAME_OFFSET,
            SHT_HASH,
            bytes.sysv_hash.len(),
            4,
            4,
            Some(ElfDynamicSectionKind::DynamicSymbol),
            0,
        ),
        ElfDynamicSectionKind::GnuSymbolVersion => (
            GNU_SYMBOL_VERSION_NAME_OFFSET,
            SHT_GNU_VERSYM,
            bytes.versym.len(),
            2,
            2,
            Some(ElfDynamicSectionKind::DynamicSymbol),
            0,
        ),
        ElfDynamicSectionKind::GnuVersionRequirement => (
            GNU_VERSION_REQUIREMENT_NAME_OFFSET,
            SHT_GNU_VERNEED,
            bytes.verneed.len(),
            4,
            0,
            Some(ElfDynamicSectionKind::DynamicString),
            needed_object_count,
        ),
        ElfDynamicSectionKind::GnuHash => (
            GNU_HASH_NAME_OFFSET,
            SHT_GNU_HASH,
            bytes.gnu_hash.len(),
            8,
            0,
            Some(ElfDynamicSectionKind::DynamicSymbol),
            0,
        ),
    };
    require(
        row.name_offset == expected.0
            && row.section_type == expected.1
            && row.flags == SHF_ALLOC
            && row.payload_size == expected.2 as u64
            && row.alignment == expected.3
            && row.entry_size == expected.4
            && row.link == expected.5
            && row.info == expected.6,
        "ELF dynamic descriptor metadata drifted from its exact semantic kind",
    )
}

fn validate_cross_payloads(
    payloads: &ValidatedElfDynamicSectionPayloads,
    contents: &ElfDynamicSectionDescriptorContents,
) -> Result<(), Diagnostic> {
    let bytes = payloads.payloads();
    require(
        bytes.dynsym.len().is_multiple_of(24)
            && bytes.versym.len().is_multiple_of(2)
            && bytes.dynsym.len() / 24 == bytes.versym.len() / 2,
        "ELF dynamic symbol and GNU symbol-version descriptor counts diverge",
    )?;
    let chain_count = read_u32(&bytes.sysv_hash, 4, "System V nchain")? as usize;
    require(
        chain_count == bytes.dynsym.len() / 24,
        "ELF System V hash nchain does not match descriptor dynamic symbols",
    )?;
    let version_requirement = contents
        .descriptors
        .iter()
        .find(|descriptor| descriptor.kind == ElfDynamicSectionKind::GnuVersionRequirement)
        .ok_or_else(|| Diagnostic::error("ELF GNU version requirement descriptor is absent"))?;
    require(
        version_requirement.info as usize == payloads.plan().needed_object_count(),
        "ELF GNU version requirement sh_info does not match the Verneed count",
    )
}

fn read_u32(bytes: &[u8], offset: usize, context: &'static str) -> Result<u32, Diagnostic> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| Diagnostic::error(format!("{context} offset overflow")))?;
    let value = bytes
        .get(offset..end)
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(|| Diagnostic::error(format!("truncated {context}")))?;
    Ok(u32::from_le_bytes(value))
}

fn non_authoritative_descriptor_compatibility_fingerprint(
    payloads: &ValidatedElfDynamicSectionPayloads,
    contents: &ElfDynamicSectionDescriptorContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-dynamic-section-descriptors.v2");
    hash.bytes(
        &payloads
            .non_authoritative_payload_compatibility_fingerprint()
            .to_le_bytes(),
    );
    hash.bytes(&contents.section_name_table_seed);
    for row in &contents.descriptors {
        hash.byte(row.kind as u8);
        hash.bytes(&row.name_offset.to_le_bytes());
        hash.bytes(&row.section_type.to_le_bytes());
        hash.bytes(&row.flags.to_le_bytes());
        hash.bytes(&row.payload_size.to_le_bytes());
        hash.bytes(&row.alignment.to_le_bytes());
        hash.bytes(&row.entry_size.to_le_bytes());
        hash.byte(row.link.map_or(0, |kind| kind as u8));
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
mod tests;
