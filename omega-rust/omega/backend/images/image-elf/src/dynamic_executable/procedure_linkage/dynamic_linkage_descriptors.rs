//! Address-free ELF section descriptors for validated procedure linkage.
//!
//! This layer appends `.plt`, `.got.plt`, and `.rela.plt` to the exact
//! upstream section-name seed and binds their fixed template byte counts to
//! semantic ELF metadata. The [generic System V ABI] defines section flags,
//! relocation-section links, and `sh_info`; the [AArch64 ELF ABI] additionally
//! defines pure-code sections and eight-byte GOT entries, while the target
//! procedure-linkage layouts come from the [x86-64 psABI] and the [AArch64
//! System V ABI]. Numeric indexes, addresses, placement, and mutation remain
//! deliberately absent.
//!
//! [generic System V ABI]: https://gabi.xinuos.com/elf/03-sheader.html
//! [AArch64 ELF ABI]: https://github.com/ARM-software/abi-aa/blob/main/aaelf64/aaelf64.rst
//! [x86-64 psABI]: https://gitlab.com/x86-psABIs/x86-64-ABI/-/blob/master/x86-64-ABI/dl.tex
//! [AArch64 System V ABI]: https://github.com/ARM-software/abi-aa/blob/main/sysvabi64/sysvabi64.rst#procedure-linkage-table

use crate::dynamic_executable::checked::{checked_sum, checked_u32, require};
use crate::dynamic_executable::procedure_linkage::dynamic_linkage_templates::ValidatedElfProcedureLinkageTemplatePlan;
use diagnostics::Diagnostic;
use target::TargetProfile;

const SHT_PROGBITS: u32 = 1;
const SHT_RELA: u32 = 4;
const SHF_WRITE: u64 = 0x1;
const SHF_ALLOC: u64 = 0x2;
const SHF_EXECINSTR: u64 = 0x4;
const SHF_INFO_LINK: u64 = 0x40;
const SHF_AARCH64_PURECODE: u64 = 0x2000_0000;
const ELF64_RELA_SIZE: u64 = 24;
const PROCEDURE_LINKAGE_ALIGNMENT: u64 = 16;
const PROCEDURE_GOT_ALIGNMENT: u64 = 8;
const PROCEDURE_RELOCATION_ALIGNMENT: u64 = 8;
const UPSTREAM_DESCRIPTOR_COUNT: usize = 7;
const APPENDED_DESCRIPTOR_COUNT: usize = 3;
const PROCEDURE_LINKAGE_NAME_SUFFIX: &[u8] = b".plt\0.got.plt\0.rela.plt\0";
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Independently validated address-free descriptors for the three procedure-
/// linkage sections, retaining the exact target templates and prior seven-row
/// descriptor carrier.
///
/// The extended name seed is append-only and still is not a completed
/// `.shstrtab`. This plan grants no final section index, address, placement,
/// fixup application, image mutation, publication, or runnable-image authority.
#[derive(Debug)]
#[must_use = "validated linkage descriptors retain the exact template carrier"]
pub struct ValidatedElfProcedureLinkageSectionDescriptorPlan {
    templates: ValidatedElfProcedureLinkageTemplatePlan,
    contents: ElfProcedureLinkageSectionDescriptorContents,
    non_authoritative_descriptor_compatibility_fingerprint: u64,
}

impl ValidatedElfProcedureLinkageSectionDescriptorPlan {
    pub const fn templates(&self) -> &ValidatedElfProcedureLinkageTemplatePlan {
        &self.templates
    }

    pub fn descriptor_count(&self) -> usize {
        UPSTREAM_DESCRIPTOR_COUNT + self.contents.descriptors.len()
    }

    pub fn appended_descriptor_count(&self) -> usize {
        self.contents.descriptors.len()
    }

    pub fn section_name_seed_byte_count(&self) -> usize {
        self.contents.section_name_table_seed.len()
    }

    /// Compatibility fingerprint of the exact target-template identity,
    /// append-only name seed, typed links/info, and every ABI metadata field.
    /// This is a content compatibility coordinate, not layout or image authority.
    pub const fn non_authoritative_descriptor_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_descriptor_compatibility_fingerprint
    }

    pub(crate) const fn contents(&self) -> &ElfProcedureLinkageSectionDescriptorContents {
        &self.contents
    }

    #[allow(dead_code)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedElfProcedureLinkageTemplatePlan,
        ElfProcedureLinkageSectionDescriptorContents,
    ) {
        (self.templates, self.contents)
    }
}

/// Rejected linkage-section descriptor planning with exact template custody.
#[derive(Debug)]
#[must_use = "ELF linkage-descriptor rejection retains the validated templates"]
pub struct ElfProcedureLinkageSectionDescriptorPlanningError {
    templates: ValidatedElfProcedureLinkageTemplatePlan,
    diagnostic: Diagnostic,
}

impl ElfProcedureLinkageSectionDescriptorPlanningError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(self) -> (ValidatedElfProcedureLinkageTemplatePlan, Diagnostic) {
        (self.templates, self.diagnostic)
    }
}

impl std::fmt::Display for ElfProcedureLinkageSectionDescriptorPlanningError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfProcedureLinkageSectionDescriptorPlanningError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfProcedureLinkageSectionDescriptorContents {
    pub(crate) section_name_table_seed: Vec<u8>,
    pub(crate) descriptors: Vec<ElfProcedureLinkageSectionDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(
    clippy::enum_variant_names,
    reason = "the repeated Procedure prefix preserves exact ELF section-domain terminology"
)]
pub(crate) enum ElfProcedureLinkageSectionKind {
    ProcedureLinkage = 1,
    ProcedureGot = 2,
    ProcedureRelocation = 3,
}

impl ElfProcedureLinkageSectionKind {
    const fn name(self) -> &'static [u8] {
        match self {
            Self::ProcedureLinkage => b".plt",
            Self::ProcedureGot => b".got.plt",
            Self::ProcedureRelocation => b".rela.plt",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ElfProcedureLinkageSectionLink {
    None = 0,
    DynamicSymbol = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ElfProcedureLinkageSectionInfo {
    None,
    RelocatedSection(ElfProcedureLinkageSectionKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfProcedureLinkageSectionDescriptor {
    pub(crate) kind: ElfProcedureLinkageSectionKind,
    pub(crate) name_offset: u32,
    pub(crate) section_type: u32,
    pub(crate) flags: u64,
    pub(crate) payload_size: u64,
    pub(crate) alignment: u64,
    pub(crate) entry_size: u64,
    pub(crate) link: ElfProcedureLinkageSectionLink,
    pub(crate) info: ElfProcedureLinkageSectionInfo,
}

struct Candidate {
    templates: ValidatedElfProcedureLinkageTemplatePlan,
    contents: ElfProcedureLinkageSectionDescriptorContents,
    non_authoritative_descriptor_compatibility_fingerprint: u64,
}

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Consume validated target templates into three address-free semantic section
/// descriptors and an append-only extension of the owning name seed.
///
/// This does not complete `.shstrtab`, assign numeric section indexes, place or
/// fix up bytes, plan `.dynamic`, serialize headers, or mutate the image.
pub fn plan_elf_procedure_linkage_section_descriptors(
    templates: ValidatedElfProcedureLinkageTemplatePlan,
) -> Result<
    ValidatedElfProcedureLinkageSectionDescriptorPlan,
    Box<ElfProcedureLinkageSectionDescriptorPlanningError>,
> {
    let contents = match derive_contents(&templates) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(
                ElfProcedureLinkageSectionDescriptorPlanningError {
                    templates,
                    diagnostic,
                },
            ));
        }
    };
    let non_authoritative_descriptor_compatibility_fingerprint =
        non_authoritative_descriptor_compatibility_fingerprint(&templates, &contents);
    let candidate = Candidate {
        templates,
        contents,
        non_authoritative_descriptor_compatibility_fingerprint,
    };
    match validate_candidate(candidate) {
        Ok(validated) => Ok(validated),
        Err(error) => Err(Box::new(
            ElfProcedureLinkageSectionDescriptorPlanningError {
                templates: error.candidate.templates,
                diagnostic: error.diagnostic,
            },
        )),
    }
}

fn derive_contents(
    templates: &ValidatedElfProcedureLinkageTemplatePlan,
) -> Result<ElfProcedureLinkageSectionDescriptorContents, Diagnostic> {
    let base_seed = templates.linkage().descriptors().section_name_table_seed();
    let plt_name_offset = checked_u32(base_seed.len(), "procedure-linkage name offset")?;
    let got_name_offset = checked_u32(
        checked_sum(
            base_seed.len(),
            ElfProcedureLinkageSectionKind::ProcedureLinkage
                .name()
                .len()
                + 1,
            "procedure GOT name offset",
        )?,
        "procedure GOT name offset",
    )?;
    let rela_name_offset = checked_u32(
        checked_sum(
            usize::try_from(got_name_offset)
                .map_err(|_| Diagnostic::error("procedure GOT name offset exceeds usize"))?,
            ElfProcedureLinkageSectionKind::ProcedureGot.name().len() + 1,
            "procedure relocation name offset",
        )?,
        "procedure relocation name offset",
    )?;
    let mut section_name_table_seed = Vec::with_capacity(checked_sum(
        base_seed.len(),
        PROCEDURE_LINKAGE_NAME_SUFFIX.len(),
        "extended ELF section-name seed size",
    )?);
    section_name_table_seed.extend_from_slice(base_seed);
    section_name_table_seed.extend_from_slice(PROCEDURE_LINKAGE_NAME_SUFFIX);

    let target = target(templates);
    let plt_flags = match target {
        TargetProfile::LinuxX64 => SHF_ALLOC | SHF_EXECINSTR,
        TargetProfile::LinuxArm64 => SHF_ALLOC | SHF_EXECINSTR | SHF_AARCH64_PURECODE,
        _ => {
            return Err(Diagnostic::error(
                "ELF linkage descriptors require an exact Linux x86-64 or AArch64 profile",
            ));
        }
    };
    let bytes = &templates.contents().bytes;
    let descriptors = vec![
        descriptor(
            ElfProcedureLinkageSectionKind::ProcedureLinkage,
            plt_name_offset,
            SHT_PROGBITS,
            plt_flags,
            bytes.plt.len(),
            PROCEDURE_LINKAGE_ALIGNMENT,
            0,
            ElfProcedureLinkageSectionLink::None,
            ElfProcedureLinkageSectionInfo::None,
        )?,
        descriptor(
            ElfProcedureLinkageSectionKind::ProcedureGot,
            got_name_offset,
            SHT_PROGBITS,
            SHF_ALLOC | SHF_WRITE,
            bytes.got_plt.len(),
            PROCEDURE_GOT_ALIGNMENT,
            0,
            ElfProcedureLinkageSectionLink::None,
            ElfProcedureLinkageSectionInfo::None,
        )?,
        descriptor(
            ElfProcedureLinkageSectionKind::ProcedureRelocation,
            rela_name_offset,
            SHT_RELA,
            SHF_ALLOC | SHF_INFO_LINK,
            bytes.rela_plt.len(),
            PROCEDURE_RELOCATION_ALIGNMENT,
            ELF64_RELA_SIZE,
            ElfProcedureLinkageSectionLink::DynamicSymbol,
            ElfProcedureLinkageSectionInfo::RelocatedSection(
                ElfProcedureLinkageSectionKind::ProcedureGot,
            ),
        )?,
    ];
    Ok(ElfProcedureLinkageSectionDescriptorContents {
        section_name_table_seed,
        descriptors,
    })
}

#[allow(clippy::too_many_arguments)]
fn descriptor(
    kind: ElfProcedureLinkageSectionKind,
    name_offset: u32,
    section_type: u32,
    flags: u64,
    payload_size: usize,
    alignment: u64,
    entry_size: u64,
    link: ElfProcedureLinkageSectionLink,
    info: ElfProcedureLinkageSectionInfo,
) -> Result<ElfProcedureLinkageSectionDescriptor, Diagnostic> {
    Ok(ElfProcedureLinkageSectionDescriptor {
        kind,
        name_offset,
        section_type,
        flags,
        payload_size: u64::try_from(payload_size)
            .map_err(|_| Diagnostic::error("ELF linkage payload size exceeds Elf64_Xword"))?,
        alignment,
        entry_size,
        link,
        info,
    })
}

fn target(templates: &ValidatedElfProcedureLinkageTemplatePlan) -> TargetProfile {
    templates
        .linkage()
        .descriptors()
        .payloads()
        .plan()
        .inputs()
        .interpreter()
        .target()
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfProcedureLinkageSectionDescriptorPlan, CandidateValidationError> {
    if let Err(diagnostic) = validate_contents(&candidate.templates, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    if candidate.non_authoritative_descriptor_compatibility_fingerprint
        != non_authoritative_descriptor_compatibility_fingerprint(
            &candidate.templates,
            &candidate.contents,
        )
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(
                "ELF linkage-descriptor compatibility fingerprint does not replay",
            ),
        });
    }
    Ok(ValidatedElfProcedureLinkageSectionDescriptorPlan {
        templates: candidate.templates,
        contents: candidate.contents,
        non_authoritative_descriptor_compatibility_fingerprint: candidate
            .non_authoritative_descriptor_compatibility_fingerprint,
    })
}

fn validate_contents(
    templates: &ValidatedElfProcedureLinkageTemplatePlan,
    contents: &ElfProcedureLinkageSectionDescriptorContents,
) -> Result<(), Diagnostic> {
    let base_descriptors = templates.linkage().descriptors();
    require(
        base_descriptors.descriptor_count() == UPSTREAM_DESCRIPTOR_COUNT,
        "ELF linkage descriptors require the exact sealed seven-row base",
    )?;
    let base_seed = base_descriptors.section_name_table_seed();
    let expected_seed_len = checked_sum(
        base_seed.len(),
        PROCEDURE_LINKAGE_NAME_SUFFIX.len(),
        "validated linkage name-seed size",
    )?;
    require(
        contents.section_name_table_seed.len() == expected_seed_len
            && contents.section_name_table_seed.get(..base_seed.len()) == Some(base_seed)
            && contents.section_name_table_seed.get(base_seed.len()..)
                == Some(PROCEDURE_LINKAGE_NAME_SUFFIX),
        "ELF linkage section-name seed is not an exact append-only extension",
    )?;
    require(
        contents.descriptors.len() == APPENDED_DESCRIPTOR_COUNT,
        "ELF linkage descriptor row count is not exact",
    )?;

    let bytes = &templates.contents().bytes;
    let target = target(templates);
    for (index, row) in contents.descriptors.iter().enumerate() {
        let expected_kind = match index {
            0 => ElfProcedureLinkageSectionKind::ProcedureLinkage,
            1 => ElfProcedureLinkageSectionKind::ProcedureGot,
            2 => ElfProcedureLinkageSectionKind::ProcedureRelocation,
            _ => unreachable!("descriptor count checked above"),
        };
        require(
            row.kind == expected_kind,
            "ELF linkage descriptors are missing, duplicated, or reordered",
        )?;
        validate_name(&contents.section_name_table_seed, row)?;
        let (section_type, flags, payload_size, alignment, entry_size, link, info) = match row.kind
        {
            ElfProcedureLinkageSectionKind::ProcedureLinkage => (
                SHT_PROGBITS,
                match target {
                    TargetProfile::LinuxX64 => SHF_ALLOC | SHF_EXECINSTR,
                    TargetProfile::LinuxArm64 => SHF_ALLOC | SHF_EXECINSTR | SHF_AARCH64_PURECODE,
                    _ => return Err(Diagnostic::error("unsupported linkage descriptor target")),
                },
                bytes.plt.len(),
                PROCEDURE_LINKAGE_ALIGNMENT,
                0,
                ElfProcedureLinkageSectionLink::None,
                ElfProcedureLinkageSectionInfo::None,
            ),
            ElfProcedureLinkageSectionKind::ProcedureGot => (
                SHT_PROGBITS,
                SHF_ALLOC | SHF_WRITE,
                bytes.got_plt.len(),
                PROCEDURE_GOT_ALIGNMENT,
                0,
                ElfProcedureLinkageSectionLink::None,
                ElfProcedureLinkageSectionInfo::None,
            ),
            ElfProcedureLinkageSectionKind::ProcedureRelocation => (
                SHT_RELA,
                SHF_ALLOC | SHF_INFO_LINK,
                bytes.rela_plt.len(),
                PROCEDURE_RELOCATION_ALIGNMENT,
                ELF64_RELA_SIZE,
                ElfProcedureLinkageSectionLink::DynamicSymbol,
                ElfProcedureLinkageSectionInfo::RelocatedSection(
                    ElfProcedureLinkageSectionKind::ProcedureGot,
                ),
            ),
        };
        require(
            row.section_type == section_type
                && row.flags == flags
                && row.payload_size
                    == u64::try_from(payload_size).map_err(|_| {
                        Diagnostic::error("validated linkage payload size exceeds Elf64_Xword")
                    })?
                && row.alignment == alignment
                && row.entry_size == entry_size
                && row.link == link
                && row.info == info,
            "ELF linkage descriptor metadata drifted from its target template",
        )?;
    }
    Ok(())
}

fn validate_name(
    seed: &[u8],
    row: &ElfProcedureLinkageSectionDescriptor,
) -> Result<(), Diagnostic> {
    let offset = usize::try_from(row.name_offset)
        .map_err(|_| Diagnostic::error("ELF linkage sh_name exceeds usize"))?;
    let tail = seed
        .get(offset..)
        .ok_or_else(|| Diagnostic::error("ELF linkage sh_name is outside the name seed"))?;
    let terminator = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| Diagnostic::error("ELF linkage section name is not NUL-terminated"))?;
    require(
        tail.get(..terminator) == Some(row.kind.name()),
        "ELF linkage sh_name does not select its exact semantic name",
    )
}

fn non_authoritative_descriptor_compatibility_fingerprint(
    templates: &ValidatedElfProcedureLinkageTemplatePlan,
    contents: &ElfProcedureLinkageSectionDescriptorContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-procedure-linkage-section-descriptors.v1");
    hash.bytes(
        &templates
            .non_authoritative_template_compatibility_fingerprint()
            .to_le_bytes(),
    );
    hash.bytes(&contents.section_name_table_seed);
    hash.bytes(&(contents.descriptors.len() as u64).to_le_bytes());
    for row in &contents.descriptors {
        hash.byte(row.kind as u8);
        hash.bytes(&row.name_offset.to_le_bytes());
        hash.bytes(&row.section_type.to_le_bytes());
        hash.bytes(&row.flags.to_le_bytes());
        hash.bytes(&row.payload_size.to_le_bytes());
        hash.bytes(&row.alignment.to_le_bytes());
        hash.bytes(&row.entry_size.to_le_bytes());
        hash.byte(row.link as u8);
        match row.info {
            ElfProcedureLinkageSectionInfo::None => hash.byte(0),
            ElfProcedureLinkageSectionInfo::RelocatedSection(kind) => {
                hash.byte(1);
                hash.byte(kind as u8);
            }
        }
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
