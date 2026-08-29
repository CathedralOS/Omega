//! Address-free semantic planning for the ELF `.dynamic` array.
//!
//! The generic [System V ABI dynamic section] defines the required tag/value
//! relationships and the significant relative order of `DT_NEEDED` rows. The
//! [LSB symbol-version ABI] defines `DT_VERSYM`, `DT_VERNEED`, and
//! `DT_VERNEEDNUM`. This module retains those meanings as typed rows and seven
//! semantic address obligations; it emits no `Elf64_Dyn` bytes or addresses.
//!
//! [System V ABI dynamic section]: https://gabi.xinuos.com/elf/08-dynamic.html
//! [LSB symbol-version ABI]: https://refspecs.linuxfoundation.org/LSB_5.0.0/LSB-Core-generic/LSB-Core-generic/symversion.html

use crate::dynamic_linkage_descriptors::{
    ElfProcedureLinkageSectionKind, ValidatedElfProcedureLinkageSectionDescriptorPlan,
};
use crate::dynamic_linkage_templates::{
    ElfProcedureLinkageFixupKind, ElfProcedureLinkageFixupStorage,
    ElfProcedureLinkageSemanticTarget,
};
use omega_target::TargetProfile;
use psi_diagnostics::Diagnostic;

const ELF64_DYNAMIC_SYMBOL_SIZE: u64 = 24;
const ELF64_RELA_SIZE: usize = 24;
const ELF64_DYN_VALUE_SIZE: u8 = 8;
const FIXED_NON_NEEDED_ROW_COUNT: usize = 13;
const ADDRESS_OBLIGATION_COUNT: usize = 7;
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Independently validated semantic `.dynamic` rows and address obligations.
///
/// The exact nine-section descriptor carrier remains owned by this non-clone
/// plan. No row contains a placed pointer, final section index, serialized
/// `Elf64_Dyn`, program header, image mutation, or runnable-image authority.
#[derive(Debug)]
#[must_use = "validated ELF dynamic tags retain the exact descriptor carrier"]
pub struct ValidatedElfDynamicTagPlan {
    descriptors: ValidatedElfProcedureLinkageSectionDescriptorPlan,
    contents: ElfDynamicTagContents,
    non_authoritative_tag_compatibility_fingerprint: u64,
}

impl ValidatedElfDynamicTagPlan {
    pub const fn descriptors(&self) -> &ValidatedElfProcedureLinkageSectionDescriptorPlan {
        &self.descriptors
    }

    pub fn row_count(&self) -> usize {
        self.contents.rows.len()
    }

    pub fn needed_row_count(&self) -> usize {
        self.descriptors
            .templates()
            .linkage()
            .descriptors()
            .payloads()
            .plan()
            .needed_object_count()
    }

    pub fn address_obligation_count(&self) -> usize {
        self.contents.address_obligations.len()
    }

    /// Compatibility fingerprint of the owning nine-section descriptor
    /// identity, exact typed row sequence, and seven semantic address
    /// obligations. This is not final-byte or loader identity.
    pub const fn non_authoritative_tag_compatibility_fingerprint(&self) -> u64 {
        self.non_authoritative_tag_compatibility_fingerprint
    }

    pub(crate) const fn contents(&self) -> &ElfDynamicTagContents {
        &self.contents
    }

    #[allow(dead_code)]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ValidatedElfProcedureLinkageSectionDescriptorPlan,
        ElfDynamicTagContents,
    ) {
        (self.descriptors, self.contents)
    }
}

/// Rejected semantic dynamic-tag planning with exact descriptor custody.
#[derive(Debug)]
#[must_use = "ELF dynamic-tag rejection retains the nine-section carrier"]
pub struct ElfDynamicTagPlanningError {
    descriptors: ValidatedElfProcedureLinkageSectionDescriptorPlan,
    diagnostic: Diagnostic,
}

impl ElfDynamicTagPlanningError {
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        ValidatedElfProcedureLinkageSectionDescriptorPlan,
        Diagnostic,
    ) {
        (self.descriptors, self.diagnostic)
    }
}

impl std::fmt::Display for ElfDynamicTagPlanningError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ElfDynamicTagPlanningError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ElfDynamicTagContents {
    pub(crate) rows: Vec<ElfDynamicSemanticRow>,
    pub(crate) address_obligations: Vec<ElfDynamicAddressObligation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub(crate) enum ElfDynamicTag {
    Null = 0,
    Needed = 1,
    ProcedureRelocationSize = 2,
    ProcedureGot = 3,
    SystemVHash = 4,
    DynamicString = 5,
    DynamicSymbol = 6,
    Rela = 7,
    DynamicStringSize = 10,
    DynamicSymbolEntrySize = 11,
    ProcedureRelocationKind = 20,
    ProcedureRelocation = 23,
    GnuSymbolVersion = 0x6fff_fff0,
    GnuVersionRequirement = 0x6fff_fffe,
    GnuVersionRequirementCount = 0x6fff_ffff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ElfDynamicValue {
    NeededStringOffset(u32),
    ProcedureRelocationByteCount(u64),
    AddressPlaceholder,
    DynamicStringByteCount(u64),
    DynamicSymbolEntryByteCount(u64),
    RelocationTag(ElfDynamicTag),
    VersionRequirementRecordCount(u64),
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfDynamicSemanticRow {
    pub(crate) tag: ElfDynamicTag,
    pub(crate) value: ElfDynamicValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ElfDynamicAddressTarget {
    ProcedureGot = 1,
    SystemVHash = 2,
    DynamicString = 3,
    DynamicSymbol = 4,
    ProcedureRelocation = 5,
    GnuSymbolVersion = 6,
    GnuVersionRequirement = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ElfDynamicAddressObligation {
    pub(crate) row_ordinal: u32,
    pub(crate) byte_width: u8,
    pub(crate) target: ElfDynamicAddressTarget,
}

struct Candidate {
    descriptors: ValidatedElfProcedureLinkageSectionDescriptorPlan,
    contents: ElfDynamicTagContents,
    non_authoritative_tag_compatibility_fingerprint: u64,
}

struct CandidateValidationError {
    candidate: Candidate,
    diagnostic: Diagnostic,
}

/// Consume the exact nine-section address-free carrier into a complete
/// semantic `.dynamic` tag sequence and typed future-address obligations.
///
/// The planner deliberately stops before `Elf64_Dyn` serialization, `.dynamic`
/// section descriptors, final section indexes, placement, or byte mutation.
pub fn plan_elf_dynamic_tags(
    descriptors: ValidatedElfProcedureLinkageSectionDescriptorPlan,
) -> Result<ValidatedElfDynamicTagPlan, Box<ElfDynamicTagPlanningError>> {
    let contents = match derive_contents(&descriptors) {
        Ok(contents) => contents,
        Err(diagnostic) => {
            return Err(Box::new(ElfDynamicTagPlanningError {
                descriptors,
                diagnostic,
            }));
        }
    };
    let non_authoritative_tag_compatibility_fingerprint =
        non_authoritative_tag_compatibility_fingerprint(&descriptors, &contents);
    let candidate = Candidate {
        descriptors,
        contents,
        non_authoritative_tag_compatibility_fingerprint,
    };
    match validate_candidate(candidate) {
        Ok(validated) => Ok(validated),
        Err(error) => Err(Box::new(ElfDynamicTagPlanningError {
            descriptors: error.candidate.descriptors,
            diagnostic: error.diagnostic,
        })),
    }
}

fn derive_contents(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
) -> Result<ElfDynamicTagContents, Diagnostic> {
    let structural = structural_contents(descriptors);
    let row_capacity = checked_sum(
        structural.needed.len(),
        FIXED_NON_NEEDED_ROW_COUNT,
        "ELF dynamic row count",
    )?;
    let mut rows = Vec::with_capacity(row_capacity);
    let mut address_obligations = Vec::with_capacity(ADDRESS_OBLIGATION_COUNT);
    for needed in &structural.needed {
        rows.push(ElfDynamicSemanticRow {
            tag: ElfDynamicTag::Needed,
            value: ElfDynamicValue::NeededStringOffset(*needed),
        });
    }
    rows.push(ElfDynamicSemanticRow {
        tag: ElfDynamicTag::ProcedureRelocationSize,
        value: ElfDynamicValue::ProcedureRelocationByteCount(checked_u64(
            descriptors.templates().procedure_relocation_byte_count(),
            "procedure relocation byte count",
        )?),
    });
    push_address_row(
        &mut rows,
        &mut address_obligations,
        ElfDynamicTag::ProcedureGot,
        ElfDynamicAddressTarget::ProcedureGot,
    )?;
    push_address_row(
        &mut rows,
        &mut address_obligations,
        ElfDynamicTag::SystemVHash,
        ElfDynamicAddressTarget::SystemVHash,
    )?;
    push_address_row(
        &mut rows,
        &mut address_obligations,
        ElfDynamicTag::DynamicString,
        ElfDynamicAddressTarget::DynamicString,
    )?;
    push_address_row(
        &mut rows,
        &mut address_obligations,
        ElfDynamicTag::DynamicSymbol,
        ElfDynamicAddressTarget::DynamicSymbol,
    )?;
    rows.push(ElfDynamicSemanticRow {
        tag: ElfDynamicTag::DynamicStringSize,
        value: ElfDynamicValue::DynamicStringByteCount(checked_u64(
            dynamic_payloads(descriptors).dynamic_string_byte_count(),
            "dynamic string byte count",
        )?),
    });
    rows.push(ElfDynamicSemanticRow {
        tag: ElfDynamicTag::DynamicSymbolEntrySize,
        value: ElfDynamicValue::DynamicSymbolEntryByteCount(ELF64_DYNAMIC_SYMBOL_SIZE),
    });
    rows.push(ElfDynamicSemanticRow {
        tag: ElfDynamicTag::ProcedureRelocationKind,
        value: ElfDynamicValue::RelocationTag(ElfDynamicTag::Rela),
    });
    push_address_row(
        &mut rows,
        &mut address_obligations,
        ElfDynamicTag::ProcedureRelocation,
        ElfDynamicAddressTarget::ProcedureRelocation,
    )?;
    push_address_row(
        &mut rows,
        &mut address_obligations,
        ElfDynamicTag::GnuSymbolVersion,
        ElfDynamicAddressTarget::GnuSymbolVersion,
    )?;
    push_address_row(
        &mut rows,
        &mut address_obligations,
        ElfDynamicTag::GnuVersionRequirement,
        ElfDynamicAddressTarget::GnuVersionRequirement,
    )?;
    rows.push(ElfDynamicSemanticRow {
        tag: ElfDynamicTag::GnuVersionRequirementCount,
        value: ElfDynamicValue::VersionRequirementRecordCount(checked_u64(
            structural.verneed.len(),
            "GNU version-requirement record count",
        )?),
    });
    rows.push(ElfDynamicSemanticRow {
        tag: ElfDynamicTag::Null,
        value: ElfDynamicValue::Null,
    });
    Ok(ElfDynamicTagContents {
        rows,
        address_obligations,
    })
}

fn push_address_row(
    rows: &mut Vec<ElfDynamicSemanticRow>,
    obligations: &mut Vec<ElfDynamicAddressObligation>,
    tag: ElfDynamicTag,
    target: ElfDynamicAddressTarget,
) -> Result<(), Diagnostic> {
    let row_ordinal = checked_u32(rows.len(), "ELF dynamic address-row ordinal")?;
    rows.push(ElfDynamicSemanticRow {
        tag,
        value: ElfDynamicValue::AddressPlaceholder,
    });
    obligations.push(ElfDynamicAddressObligation {
        row_ordinal,
        byte_width: ELF64_DYN_VALUE_SIZE,
        target,
    });
    Ok(())
}

fn dynamic_payloads(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
) -> &crate::dynamic_section_bytes::ValidatedElfDynamicSectionPayloads {
    descriptors.templates().linkage().descriptors().payloads()
}

fn structural_contents(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
) -> &crate::dynamic_sections::ElfDynamicSectionContents {
    dynamic_payloads(descriptors).plan().contents()
}

fn target(descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan) -> TargetProfile {
    dynamic_payloads(descriptors)
        .plan()
        .inputs()
        .interpreter()
        .target()
}

fn validate_candidate(
    candidate: Candidate,
) -> Result<ValidatedElfDynamicTagPlan, CandidateValidationError> {
    if let Err(diagnostic) = validate_contents(&candidate.descriptors, &candidate.contents) {
        return Err(CandidateValidationError {
            candidate,
            diagnostic,
        });
    }
    if candidate.non_authoritative_tag_compatibility_fingerprint
        != non_authoritative_tag_compatibility_fingerprint(
            &candidate.descriptors,
            &candidate.contents,
        )
    {
        return Err(CandidateValidationError {
            candidate,
            diagnostic: Diagnostic::error(
                "ELF dynamic-tag compatibility fingerprint does not replay",
            ),
        });
    }
    Ok(ValidatedElfDynamicTagPlan {
        descriptors: candidate.descriptors,
        contents: candidate.contents,
        non_authoritative_tag_compatibility_fingerprint: candidate
            .non_authoritative_tag_compatibility_fingerprint,
    })
}

fn validate_contents(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
    contents: &ElfDynamicTagContents,
) -> Result<(), Diagnostic> {
    require(
        descriptors.descriptor_count() == 9,
        "ELF dynamic tags require the exact nine-section descriptor carrier",
    )?;
    require(
        matches!(
            target(descriptors),
            TargetProfile::LinuxX64 | TargetProfile::LinuxArm64
        ),
        "ELF dynamic tags require an exact supported Linux profile",
    )?;
    let structural = structural_contents(descriptors);
    require(
        !structural.needed.is_empty(),
        "ELF dynamic tags require at least one owned DT_NEEDED row",
    )?;
    let expected_row_count = checked_sum(
        structural.needed.len(),
        FIXED_NON_NEEDED_ROW_COUNT,
        "validated ELF dynamic row count",
    )?;
    require(
        contents.rows.len() == expected_row_count,
        "ELF dynamic tag row count is not exact",
    )?;
    validate_needed_rows(descriptors, contents, structural.needed.len())?;
    validate_fixed_rows(descriptors, contents, structural.needed.len())?;
    validate_descriptor_targets(descriptors)?;
    validate_address_obligations(contents)?;
    validate_relocation_closure(descriptors)?;
    Ok(())
}

fn validate_descriptor_targets(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
) -> Result<(), Diagnostic> {
    let appended = &descriptors.contents().descriptors;
    for kind in [
        ElfProcedureLinkageSectionKind::ProcedureGot,
        ElfProcedureLinkageSectionKind::ProcedureRelocation,
    ] {
        require(
            appended.iter().filter(|row| row.kind == kind).count() == 1,
            "ELF dynamic address target is not uniquely owned by the linkage descriptors",
        )?;
    }
    Ok(())
}

fn validate_needed_rows(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
    contents: &ElfDynamicTagContents,
    needed_count: usize,
) -> Result<(), Diagnostic> {
    let structural = structural_contents(descriptors);
    let dynstr = &dynamic_payloads(descriptors).payloads().dynstr;
    let mut prior = None;
    for (index, expected_offset) in structural.needed.iter().enumerate() {
        let row = contents
            .rows
            .get(index)
            .ok_or_else(|| Diagnostic::error("missing DT_NEEDED row"))?;
        require(
            *row == (ElfDynamicSemanticRow {
                tag: ElfDynamicTag::Needed,
                value: ElfDynamicValue::NeededStringOffset(*expected_offset),
            }),
            "DT_NEEDED row drifted from the exact significant roster",
        )?;
        let object = dynamic_string(dynstr, *expected_offset)?;
        require(
            !object.is_empty(),
            "DT_NEEDED references an empty object name",
        )?;
        if let Some(prior_object) = prior {
            require(
                prior_object < object,
                "DT_NEEDED rows are duplicated or not in canonical significant order",
            )?;
        }
        prior = Some(object);
    }
    require(
        contents
            .rows
            .get(needed_count)
            .is_some_and(|row| row.tag != ElfDynamicTag::Needed),
        "DT_NEEDED rows are not one contiguous significant prefix",
    )
}

fn validate_fixed_rows(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
    contents: &ElfDynamicTagContents,
    base: usize,
) -> Result<(), Diagnostic> {
    let structural = structural_contents(descriptors);
    let fixed = [
        ElfDynamicSemanticRow {
            tag: ElfDynamicTag::ProcedureRelocationSize,
            value: ElfDynamicValue::ProcedureRelocationByteCount(checked_u64(
                descriptors.templates().procedure_relocation_byte_count(),
                "validated procedure relocation size",
            )?),
        },
        address_row(ElfDynamicTag::ProcedureGot),
        address_row(ElfDynamicTag::SystemVHash),
        address_row(ElfDynamicTag::DynamicString),
        address_row(ElfDynamicTag::DynamicSymbol),
        ElfDynamicSemanticRow {
            tag: ElfDynamicTag::DynamicStringSize,
            value: ElfDynamicValue::DynamicStringByteCount(checked_u64(
                dynamic_payloads(descriptors).dynamic_string_byte_count(),
                "validated dynamic string size",
            )?),
        },
        ElfDynamicSemanticRow {
            tag: ElfDynamicTag::DynamicSymbolEntrySize,
            value: ElfDynamicValue::DynamicSymbolEntryByteCount(ELF64_DYNAMIC_SYMBOL_SIZE),
        },
        ElfDynamicSemanticRow {
            tag: ElfDynamicTag::ProcedureRelocationKind,
            value: ElfDynamicValue::RelocationTag(ElfDynamicTag::Rela),
        },
        address_row(ElfDynamicTag::ProcedureRelocation),
        address_row(ElfDynamicTag::GnuSymbolVersion),
        address_row(ElfDynamicTag::GnuVersionRequirement),
        ElfDynamicSemanticRow {
            tag: ElfDynamicTag::GnuVersionRequirementCount,
            value: ElfDynamicValue::VersionRequirementRecordCount(checked_u64(
                structural.verneed.len(),
                "validated version-requirement count",
            )?),
        },
        ElfDynamicSemanticRow {
            tag: ElfDynamicTag::Null,
            value: ElfDynamicValue::Null,
        },
    ];
    require(
        contents.rows.get(base..) == Some(&fixed),
        "fixed ELF dynamic tags are missing, duplicated, reordered, or malformed",
    )?;
    let relocation_bytes = descriptors.templates().procedure_relocation_byte_count();
    let relocation_count = descriptors
        .templates()
        .linkage()
        .procedure_relocation_count();
    require(
        relocation_bytes > 0
            && relocation_bytes % ELF64_RELA_SIZE == 0
            && relocation_bytes
                == checked_product(
                    relocation_count,
                    ELF64_RELA_SIZE,
                    "validated PLT relocation byte count",
                )?,
        "DT_PLTRELSZ does not match exact Elf64_Rela JUMP_SLOT rows",
    )?;
    require(
        structural.verneed.len() == structural.needed.len(),
        "DT_VERNEEDNUM does not cover the exact needed-object roster",
    )
}

const fn address_row(tag: ElfDynamicTag) -> ElfDynamicSemanticRow {
    ElfDynamicSemanticRow {
        tag,
        value: ElfDynamicValue::AddressPlaceholder,
    }
}

fn validate_address_obligations(contents: &ElfDynamicTagContents) -> Result<(), Diagnostic> {
    let expected = [
        (
            ElfDynamicTag::ProcedureGot,
            ElfDynamicAddressTarget::ProcedureGot,
        ),
        (
            ElfDynamicTag::SystemVHash,
            ElfDynamicAddressTarget::SystemVHash,
        ),
        (
            ElfDynamicTag::DynamicString,
            ElfDynamicAddressTarget::DynamicString,
        ),
        (
            ElfDynamicTag::DynamicSymbol,
            ElfDynamicAddressTarget::DynamicSymbol,
        ),
        (
            ElfDynamicTag::ProcedureRelocation,
            ElfDynamicAddressTarget::ProcedureRelocation,
        ),
        (
            ElfDynamicTag::GnuSymbolVersion,
            ElfDynamicAddressTarget::GnuSymbolVersion,
        ),
        (
            ElfDynamicTag::GnuVersionRequirement,
            ElfDynamicAddressTarget::GnuVersionRequirement,
        ),
    ];
    require(
        contents.address_obligations.len() == ADDRESS_OBLIGATION_COUNT,
        "ELF dynamic address-obligation count is not exact",
    )?;
    for (obligation, (tag, target)) in contents.address_obligations.iter().zip(expected) {
        let row_index = usize::try_from(obligation.row_ordinal)
            .map_err(|_| Diagnostic::error("ELF dynamic row ordinal exceeds usize"))?;
        require(
            obligation.byte_width == ELF64_DYN_VALUE_SIZE
                && obligation.target == target
                && contents.rows.get(row_index) == Some(&address_row(tag)),
            "ELF dynamic address obligation drifted from its exact semantic row",
        )?;
    }
    for (index, row) in contents.rows.iter().enumerate() {
        let obligation_count = contents
            .address_obligations
            .iter()
            .filter(|obligation| obligation.row_ordinal as usize == index)
            .count();
        require(
            obligation_count
                == usize::from(matches!(row.value, ElfDynamicValue::AddressPlaceholder)),
            "ELF dynamic row has a missing, duplicate, or orphan address obligation",
        )?;
    }
    Ok(())
}

fn validate_relocation_closure(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
) -> Result<(), Diagnostic> {
    let linkage = descriptors.templates().linkage();
    require(
        linkage.general_dynamic_relocation_count() == 0,
        "semantic dynamic tags cannot omit a required general relocation table",
    )?;
    let future_dynamic = descriptors
        .templates()
        .contents()
        .fixups
        .iter()
        .filter(|fixup| {
            fixup.storage == ElfProcedureLinkageFixupStorage::GotPlt
                && fixup.kind == ElfProcedureLinkageFixupKind::Absolute64
                && fixup.target == ElfProcedureLinkageSemanticTarget::FutureDynamicSection
        })
        .count();
    require(
        future_dynamic
            == match target(descriptors) {
                TargetProfile::LinuxX64 => 1,
                TargetProfile::LinuxArm64 => 0,
                _ => return Err(Diagnostic::error("unsupported relocation-closure target")),
            },
        "target GOT policy drifted from the future .dynamic section obligation",
    )
}

fn dynamic_string(bytes: &[u8], offset: u32) -> Result<&[u8], Diagnostic> {
    let offset = usize::try_from(offset)
        .map_err(|_| Diagnostic::error("DT_NEEDED string offset exceeds usize"))?;
    let tail = bytes
        .get(offset..)
        .ok_or_else(|| Diagnostic::error("DT_NEEDED string offset is outside .dynstr"))?;
    let terminator = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| Diagnostic::error("DT_NEEDED object name is not NUL-terminated"))?;
    Ok(&tail[..terminator])
}

fn checked_sum(left: usize, right: usize, context: &'static str) -> Result<usize, Diagnostic> {
    left.checked_add(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

fn checked_product(left: usize, right: usize, context: &'static str) -> Result<usize, Diagnostic> {
    left.checked_mul(right)
        .ok_or_else(|| Diagnostic::error(format!("{context} overflows usize")))
}

fn checked_u32(value: usize, context: &'static str) -> Result<u32, Diagnostic> {
    u32::try_from(value).map_err(|_| Diagnostic::error(format!("{context} exceeds Elf64_Word")))
}

fn checked_u64(value: usize, context: &'static str) -> Result<u64, Diagnostic> {
    u64::try_from(value).map_err(|_| Diagnostic::error(format!("{context} exceeds Elf64_Xword")))
}

fn require(condition: bool, message: &'static str) -> Result<(), Diagnostic> {
    condition
        .then_some(())
        .ok_or_else(|| Diagnostic::error(message))
}

fn non_authoritative_tag_compatibility_fingerprint(
    descriptors: &ValidatedElfProcedureLinkageSectionDescriptorPlan,
    contents: &ElfDynamicTagContents,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.elf-dynamic-tags.v1");
    hash.bytes(
        &descriptors
            .non_authoritative_descriptor_compatibility_fingerprint()
            .to_le_bytes(),
    );
    hash.bytes(&(contents.rows.len() as u64).to_le_bytes());
    for row in &contents.rows {
        hash.bytes(&(row.tag as i64).to_le_bytes());
        match row.value {
            ElfDynamicValue::NeededStringOffset(offset) => {
                hash.byte(1);
                hash.bytes(&offset.to_le_bytes());
            }
            ElfDynamicValue::ProcedureRelocationByteCount(count) => {
                hash.byte(2);
                hash.bytes(&count.to_le_bytes());
            }
            ElfDynamicValue::AddressPlaceholder => hash.byte(3),
            ElfDynamicValue::DynamicStringByteCount(count) => {
                hash.byte(4);
                hash.bytes(&count.to_le_bytes());
            }
            ElfDynamicValue::DynamicSymbolEntryByteCount(count) => {
                hash.byte(5);
                hash.bytes(&count.to_le_bytes());
            }
            ElfDynamicValue::RelocationTag(tag) => {
                hash.byte(6);
                hash.bytes(&(tag as i64).to_le_bytes());
            }
            ElfDynamicValue::VersionRequirementRecordCount(count) => {
                hash.byte(7);
                hash.bytes(&count.to_le_bytes());
            }
            ElfDynamicValue::Null => hash.byte(8),
        }
    }
    hash.bytes(&(contents.address_obligations.len() as u64).to_le_bytes());
    for obligation in &contents.address_obligations {
        hash.bytes(&obligation.row_ordinal.to_le_bytes());
        hash.byte(obligation.byte_width);
        hash.byte(obligation.target as u8);
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
        plan_elf_dynamic_sections, plan_elf_procedure_linkage_relocations,
        plan_elf_procedure_linkage_section_descriptors, plan_elf_procedure_linkage_templates,
        serialize_elf_dynamic_sections,
    };
    use omega_image::{
        FinalImage, FinalImageImport, FinalImageImportPlan, FinalImageMemory, FinalImageRelocation,
        FinalImageSection, FinalImageSymbol,
    };
    use omega_object_file::{RelocationKind, SymbolKind};
    use omega_target::{
        ForeignLocatorCandidate, normalize_elf_interpreter_plan, normalize_foreign_locator,
    };
    use psi_arena::Handle;

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
            _ => unreachable!("dynamic-tag fixture uses a Linux target"),
        }
    }

    fn descriptors(
        target: TargetProfile,
        imports: &[ImportFixture],
    ) -> ValidatedElfProcedureLinkageSectionDescriptorPlan {
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
                name: format!("__omega_dynamic_tag_import_{index}"),
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
                    .expect("valid dynamic-tag locator"),
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
            .expect("valid dynamic-tag interpreter");
        let inputs =
            plan_elf_dynamic_link_inputs(image, interpreter).expect("valid dynamic-link inputs");
        let sections = plan_elf_dynamic_sections(inputs).expect("valid dynamic sections");
        let payloads = serialize_elf_dynamic_sections(sections).expect("valid dynamic payloads");
        let base = plan_elf_dynamic_section_descriptors(payloads).expect("valid base descriptors");
        let linkage =
            plan_elf_procedure_linkage_relocations(base).expect("valid procedure linkage");
        let templates =
            plan_elf_procedure_linkage_templates(linkage).expect("valid linkage templates");
        plan_elf_procedure_linkage_section_descriptors(templates)
            .expect("valid linkage descriptors")
    }

    fn candidate(target: TargetProfile) -> Candidate {
        let descriptors = descriptors(target, &IMPORTS);
        let contents = derive_contents(&descriptors).expect("derived dynamic tags");
        let non_authoritative_tag_compatibility_fingerprint =
            non_authoritative_tag_compatibility_fingerprint(&descriptors, &contents);
        Candidate {
            descriptors,
            contents,
            non_authoritative_tag_compatibility_fingerprint,
        }
    }

    #[test]
    fn both_targets_plan_exact_needed_prefix_fixed_tags_and_address_obligations() {
        for target in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            let plan = plan_elf_dynamic_tags(descriptors(target, &IMPORTS))
                .expect("validated semantic dynamic tags");
            assert_eq!(plan.descriptors().descriptor_count(), 9);
            assert_eq!(plan.needed_row_count(), 2);
            assert_eq!(plan.row_count(), 15);
            assert_eq!(plan.address_obligation_count(), 7);
            assert_ne!(plan.non_authoritative_tag_compatibility_fingerprint(), 0);

            let structural = structural_contents(&plan.descriptors);
            assert_eq!(
                &plan.contents.rows[..2],
                &[
                    ElfDynamicSemanticRow {
                        tag: ElfDynamicTag::Needed,
                        value: ElfDynamicValue::NeededStringOffset(structural.needed[0]),
                    },
                    ElfDynamicSemanticRow {
                        tag: ElfDynamicTag::Needed,
                        value: ElfDynamicValue::NeededStringOffset(structural.needed[1]),
                    },
                ],
            );
            assert_eq!(
                plan.contents
                    .rows
                    .iter()
                    .map(|row| row.tag)
                    .collect::<Vec<_>>(),
                [
                    ElfDynamicTag::Needed,
                    ElfDynamicTag::Needed,
                    ElfDynamicTag::ProcedureRelocationSize,
                    ElfDynamicTag::ProcedureGot,
                    ElfDynamicTag::SystemVHash,
                    ElfDynamicTag::DynamicString,
                    ElfDynamicTag::DynamicSymbol,
                    ElfDynamicTag::DynamicStringSize,
                    ElfDynamicTag::DynamicSymbolEntrySize,
                    ElfDynamicTag::ProcedureRelocationKind,
                    ElfDynamicTag::ProcedureRelocation,
                    ElfDynamicTag::GnuSymbolVersion,
                    ElfDynamicTag::GnuVersionRequirement,
                    ElfDynamicTag::GnuVersionRequirementCount,
                    ElfDynamicTag::Null,
                ],
            );
            assert_eq!(
                plan.contents.address_obligations,
                [
                    ElfDynamicAddressObligation {
                        row_ordinal: 3,
                        byte_width: 8,
                        target: ElfDynamicAddressTarget::ProcedureGot,
                    },
                    ElfDynamicAddressObligation {
                        row_ordinal: 4,
                        byte_width: 8,
                        target: ElfDynamicAddressTarget::SystemVHash,
                    },
                    ElfDynamicAddressObligation {
                        row_ordinal: 5,
                        byte_width: 8,
                        target: ElfDynamicAddressTarget::DynamicString,
                    },
                    ElfDynamicAddressObligation {
                        row_ordinal: 6,
                        byte_width: 8,
                        target: ElfDynamicAddressTarget::DynamicSymbol,
                    },
                    ElfDynamicAddressObligation {
                        row_ordinal: 10,
                        byte_width: 8,
                        target: ElfDynamicAddressTarget::ProcedureRelocation,
                    },
                    ElfDynamicAddressObligation {
                        row_ordinal: 11,
                        byte_width: 8,
                        target: ElfDynamicAddressTarget::GnuSymbolVersion,
                    },
                    ElfDynamicAddressObligation {
                        row_ordinal: 12,
                        byte_width: 8,
                        target: ElfDynamicAddressTarget::GnuVersionRequirement,
                    },
                ],
            );
            assert_eq!(
                plan.contents.rows[2].value,
                ElfDynamicValue::ProcedureRelocationByteCount(48),
            );
            assert_eq!(
                plan.contents.rows[8].value,
                ElfDynamicValue::DynamicSymbolEntryByteCount(24),
            );
            assert_eq!(
                plan.contents.rows[9].value,
                ElfDynamicValue::RelocationTag(ElfDynamicTag::Rela),
            );
            assert_eq!(
                plan.contents.rows[13].value,
                ElfDynamicValue::VersionRequirementRecordCount(2),
            );
            assert_eq!(plan.contents.rows[14].value, ElfDynamicValue::Null);
            validate_contents(plan.descriptors(), &plan.contents)
                .expect("independent dynamic-tag replay");
        }
    }

    #[test]
    fn needed_rows_preserve_raw_non_utf8_names_and_significant_order() {
        let plan = plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
        let dynstr = &dynamic_payloads(plan.descriptors()).payloads().dynstr;
        let names = structural_contents(plan.descriptors())
            .needed
            .iter()
            .map(|offset| dynamic_string(dynstr, *offset).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(names, [b"liba\xff.so".as_slice(), b"libb.so".as_slice()]);
        assert!(names[0].iter().any(|byte| *byte == 0xff));
    }

    #[test]
    fn import_permutation_preserves_rows_and_identity_while_target_stays_bound() {
        let forward =
            plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
        let reverse_imports = IMPORTS.iter().rev().copied().collect::<Vec<_>>();
        let reverse =
            plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxX64, &reverse_imports)).unwrap();
        let arm = plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxArm64, &IMPORTS)).unwrap();
        assert_eq!(forward.contents, reverse.contents);
        assert_eq!(
            forward.non_authoritative_tag_compatibility_fingerprint(),
            reverse.non_authoritative_tag_compatibility_fingerprint()
        );
        assert_ne!(
            forward.non_authoritative_tag_compatibility_fingerprint(),
            arm.non_authoritative_tag_compatibility_fingerprint()
        );
    }

    #[test]
    fn exact_tag_set_omits_unowned_general_relocation_and_optional_policies() {
        let plan = plan_elf_dynamic_tags(descriptors(TargetProfile::LinuxX64, &IMPORTS)).unwrap();
        assert!(
            !plan
                .contents
                .rows
                .iter()
                .any(|row| row.tag == ElfDynamicTag::Rela)
        );
        assert_eq!(
            plan.descriptors
                .templates()
                .linkage()
                .general_dynamic_relocation_count(),
            0,
        );
        assert_eq!(
            plan.contents
                .rows
                .iter()
                .filter(|row| row.tag == ElfDynamicTag::Null)
                .count(),
            1,
        );
        assert_eq!(plan.contents.rows.last().unwrap().tag, ElfDynamicTag::Null);
    }

    #[test]
    fn independent_replay_rejects_every_value_family_order_and_identity_corruption() {
        let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
            Box::new(|candidate| candidate.contents.rows.swap(0, 1)),
            Box::new(|candidate| candidate.contents.rows.swap(2, 3)),
            Box::new(|candidate| {
                candidate.contents.rows.pop();
            }),
            Box::new(|candidate| candidate.contents.rows.push(candidate.contents.rows[14])),
            Box::new(|candidate| candidate.contents.rows[0].tag = ElfDynamicTag::Null),
            Box::new(|candidate| {
                candidate.contents.rows[0].value = ElfDynamicValue::NeededStringOffset(0)
            }),
            Box::new(|candidate| {
                candidate.contents.rows[2].value = ElfDynamicValue::ProcedureRelocationByteCount(47)
            }),
            Box::new(|candidate| candidate.contents.rows[3].value = ElfDynamicValue::Null),
            Box::new(|candidate| {
                candidate.contents.rows[7].value = ElfDynamicValue::DynamicStringByteCount(0)
            }),
            Box::new(|candidate| {
                candidate.contents.rows[8].value = ElfDynamicValue::DynamicSymbolEntryByteCount(16)
            }),
            Box::new(|candidate| {
                candidate.contents.rows[9].value =
                    ElfDynamicValue::RelocationTag(ElfDynamicTag::Null)
            }),
            Box::new(|candidate| {
                candidate.contents.rows[13].value =
                    ElfDynamicValue::VersionRequirementRecordCount(1)
            }),
            Box::new(|candidate| {
                candidate.contents.rows[14].value = ElfDynamicValue::AddressPlaceholder
            }),
            Box::new(|candidate| candidate.non_authoritative_tag_compatibility_fingerprint ^= 1),
        ];
        for corrupt in corruptions {
            let mut candidate = candidate(TargetProfile::LinuxX64);
            let expected_identity = candidate
                .descriptors
                .non_authoritative_descriptor_compatibility_fingerprint();
            corrupt(&mut candidate);
            let error = validate_candidate(candidate)
                .expect_err("corrupt semantic dynamic tags must reject");
            assert_eq!(
                error
                    .candidate
                    .descriptors
                    .non_authoritative_descriptor_compatibility_fingerprint(),
                expected_identity,
                "dynamic-tag rejection retains exact descriptor custody",
            );
        }
    }

    #[test]
    fn independent_replay_rejects_missing_duplicate_or_misdirected_address_obligations() {
        let corruptions: Vec<Box<dyn Fn(&mut Candidate)>> = vec![
            Box::new(|candidate| {
                candidate.contents.address_obligations.pop();
            }),
            Box::new(|candidate| {
                candidate
                    .contents
                    .address_obligations
                    .push(candidate.contents.address_obligations[0])
            }),
            Box::new(|candidate| candidate.contents.address_obligations.swap(0, 1)),
            Box::new(|candidate| candidate.contents.address_obligations[0].row_ordinal = u32::MAX),
            Box::new(|candidate| candidate.contents.address_obligations[0].byte_width = 4),
            Box::new(|candidate| {
                candidate.contents.address_obligations[0].target =
                    ElfDynamicAddressTarget::DynamicString
            }),
        ];
        for corrupt in corruptions {
            let mut candidate = candidate(TargetProfile::LinuxArm64);
            let expected_identity = candidate
                .descriptors
                .non_authoritative_descriptor_compatibility_fingerprint();
            corrupt(&mut candidate);
            let error = validate_candidate(candidate)
                .expect_err("corrupt dynamic address obligations must reject");
            assert_eq!(
                error
                    .candidate
                    .descriptors
                    .non_authoritative_descriptor_compatibility_fingerprint(),
                expected_identity,
            );
        }
    }

    #[test]
    fn malformed_offsets_counts_and_arithmetic_reject_without_panicking() {
        assert!(checked_sum(usize::MAX, 1, "sum").is_err());
        assert!(checked_product(usize::MAX, 2, "product").is_err());
        assert!(checked_u32(usize::MAX, "word").is_err());
        assert!(dynamic_string(&[], 0).is_err());
        assert!(dynamic_string(b"unterminated", 0).is_err());
        assert!(dynamic_string(b"\0", u32::MAX).is_err());

        let mut candidate = candidate(TargetProfile::LinuxX64);
        candidate.contents.address_obligations[0].row_ordinal = u32::MAX;
        assert!(validate_candidate(candidate).is_err());
    }
}
